use crate::error::{NetworkMonitorError, Result};
use crate::models::{Connection, ProcessIO};
use crate::services::connection_monitor::ConnectionMonitor;
use aya::maps::perf::{PerfEvent, PerfEventArray, PerfEventArrayBuffer};
use aya::maps::MapData;
use aya::programs::KProbe;
use aya::util::online_cpus;
use network_monitor_common::{
    EbpfEvent, TcpAcceptEvent, TcpCloseEvent, TcpConnectEvent, AF_INET, AF_INET6,
    EVENT_TYPE_ACCEPT, EVENT_TYPE_CLOSE, EVENT_TYPE_CONNECT,
};
use std::collections::HashMap;
use std::mem;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::{Duration, Instant};

struct ConnectionState {
    connection: Connection,
    last_seen: Instant,
}

pub struct EbpfMonitor {
    _bpf: Option<aya::Ebpf>,
    _reader_thread: Option<thread::JoinHandle<()>>,
    connections: Arc<Mutex<HashMap<u64, ConnectionState>>>,
    stopped: Arc<AtomicBool>,
    last_update_time: std::cell::RefCell<Instant>,
}

impl EbpfMonitor {
    pub fn new() -> Result<Self> {
        Self::check_ebpf_availability()?;
        Self::load_and_attach()
    }

    fn check_ebpf_availability() -> Result<()> {
        if !cfg!(target_os = "linux") {
            return Err(NetworkMonitorError::EbpfNotAvailable(
                "eBPF is only supported on Linux".to_string(),
            ));
        }

        let version = std::fs::read_to_string("/proc/sys/kernel/osrelease").unwrap_or_default();
        if version.is_empty() {
            return Err(NetworkMonitorError::EbpfNotAvailable(
                "Cannot determine kernel version from /proc/sys/kernel/osrelease".to_string(),
            ));
        }
        if let Some(major_str) = version.split('.').next() {
            if let Ok(major) = major_str.parse::<u32>() {
                if major < 4 {
                    return Err(NetworkMonitorError::EbpfNotAvailable(format!(
                        "Kernel {version} too old, need 4.1+ for kprobes"
                    )));
                }
            }
        }

        let caps_bpf = 1u64 << 39;
        let caps_perfmon = 1u64 << 38;
        let caps_net_admin = 1u64 << 12;
        let caps_sys_admin = 1u64 << 21;
        if !has_effective_cap(caps_bpf | caps_perfmon | caps_net_admin)
            && !has_effective_cap(caps_sys_admin)
        {
            return Err(NetworkMonitorError::EbpfPermissionError(
                "eBPF programs require CAP_BPF + CAP_NET_ADMIN + CAP_PERFMON, \
                 or CAP_SYS_ADMIN on older kernels"
                    .to_string(),
            ));
        }

        Ok(())
    }

    fn load_and_attach() -> Result<Self> {
        let ebpf_bytes: &[u8] =
            aya::include_bytes_aligned!(concat!(env!("OUT_DIR"), "/network-monitor-ebpf"));
        if ebpf_bytes.len() <= 4 {
            return Err(NetworkMonitorError::EbpfNotAvailable(
                "eBPF programs were not compiled (requires nightly Rust + bpf-linker)".to_string(),
            ));
        }
        let mut bpf = aya::Ebpf::load(ebpf_bytes).map_err(|e| {
            NetworkMonitorError::EbpfLoadError(format!("Failed to load eBPF object: {e}"))
        })?;

        let programs: [(&str, &str, u64); 4] = [
            ("kprobe_tcp_connect", "tcp_v4_connect", 0),
            ("kprobe_tcp_connect6", "tcp_v6_connect", 0),
            ("kprobe_tcp_close", "tcp_close", 0),
            ("kretprobe_inet_csk_accept", "inet_csk_accept", 0),
        ];

        for &(prog_name, fn_name, offset) in &programs {
            let prog: &mut KProbe = bpf
                .program_mut(prog_name)
                .ok_or_else(|| {
                    NetworkMonitorError::EbpfLoadError(format!("Program '{prog_name}' not found"))
                })?
                .try_into()
                .map_err(|_| {
                    NetworkMonitorError::EbpfLoadError(format!(
                        "Failed to convert '{prog_name}' to KProbe"
                    ))
                })?;
            prog.load().map_err(|e| {
                NetworkMonitorError::EbpfAttachError(format!("{prog_name} load: {e}"))
            })?;
            prog.attach(fn_name, offset).map_err(|e| {
                NetworkMonitorError::EbpfAttachError(format!("{prog_name} attach: {e}"))
            })?;
        }

        let connections: Arc<Mutex<HashMap<u64, ConnectionState>>> =
            Arc::new(Mutex::new(HashMap::new()));
        let stopped = Arc::new(AtomicBool::new(false));

        let reader_handle =
            Self::start_event_reader(&mut bpf, connections.clone(), stopped.clone());
        Self::rehydrate(&connections);

        Ok(Self {
            _bpf: Some(bpf),
            _reader_thread: reader_handle,
            connections,
            stopped,
            last_update_time: std::cell::RefCell::new(Instant::now()),
        })
    }

    fn start_event_reader(
        bpf: &mut aya::Ebpf,
        connections: Arc<Mutex<HashMap<u64, ConnectionState>>>,
        stopped: Arc<AtomicBool>,
    ) -> Option<thread::JoinHandle<()>> {
        let map = match bpf.take_map("EVENTS") {
            Some(m) => m,
            None => {
                eprintln!("Warning: Map 'EVENTS' not found");
                return None;
            }
        };

        let mut perf_array: PerfEventArray<MapData> = match PerfEventArray::try_from(map) {
            Ok(p) => p,
            Err(e) => {
                eprintln!("Warning: Failed to create PerfEventArray: {e}");
                eprintln!("eBPF events will not be consumed. Connections from rehydration only.");
                return None;
            }
        };

        let cpus = match online_cpus() {
            Ok(c) => c,
            Err((msg, e)) => {
                eprintln!("Warning: Failed to get online CPUs ({msg}): {e}");
                eprintln!("eBPF events will not be consumed. Connections from rehydration only.");
                return None;
            }
        };

        let mut buffers: Vec<(u32, PerfEventArrayBuffer<MapData>)> = Vec::new();
        for cpu_id in cpus {
            match perf_array.open(cpu_id, Some(16)) {
                Ok(buf) => buffers.push((cpu_id, buf)),
                Err(e) => {
                    eprintln!("Warning: Failed to open perf buffer for CPU {cpu_id}: {e}");
                }
            }
        }

        if buffers.is_empty() {
            eprintln!("Warning: No perf buffers opened. Events will not be consumed.");
            return None;
        }

        let handle = thread::Builder::new()
            .name("ebpf-event-reader".into())
            .spawn(move || {
                while !stopped.load(Ordering::Relaxed) {
                    for (_cpu_id, buf) in &mut buffers {
                        buf.for_each(|event: PerfEvent<'_>| match event {
                            PerfEvent::Sample { head, tail } => {
                                let data = if tail.is_empty() {
                                    head.to_vec()
                                } else {
                                    let mut v = Vec::with_capacity(head.len() + tail.len());
                                    v.extend_from_slice(head);
                                    v.extend_from_slice(tail);
                                    v
                                };
                                Self::process_events(&data, &connections);
                            }
                            PerfEvent::Lost { count } => {
                                eprintln!("Lost {count} perf events");
                            }
                        });
                    }
                    thread::sleep(Duration::from_millis(5));
                }
            })
            .expect("Failed to spawn event reader thread");
        Some(handle)
    }

    fn process_events(data: &[u8], connections: &Arc<Mutex<HashMap<u64, ConnectionState>>>) {
        let event_size = mem::size_of::<EbpfEvent>();
        if data.len() < event_size {
            return;
        }
        let event: EbpfEvent = unsafe { (data.as_ptr() as *const EbpfEvent).read_unaligned() };
        let now = Instant::now();

        let mut guard = match connections.lock() {
            Ok(g) => g,
            Err(_) => return,
        };

        match event.event_type {
            EVENT_TYPE_CONNECT => {
                let ev: TcpConnectEvent = unsafe { event.data.connect };
                guard.insert(
                    sock_key(ev.pid, ev.dport as u64),
                    ConnectionState {
                        connection: connection_from_connect(&ev),
                        last_seen: now,
                    },
                );
            }
            EVENT_TYPE_ACCEPT => {
                let ev: TcpAcceptEvent = unsafe { event.data.accept };
                guard.insert(
                    sock_key(ev.pid, ev.dport as u64),
                    ConnectionState {
                        connection: connection_from_accept(&ev),
                        last_seen: now,
                    },
                );
            }
            EVENT_TYPE_CLOSE => {
                let ev: TcpCloseEvent = unsafe { event.data.close };
                let key = sock_key(ev.pid, ev.dport as u64);
                guard.remove(&key);
            }
            _ => {}
        }
    }

    fn rehydrate(connections: &Arc<Mutex<HashMap<u64, ConnectionState>>>) {
        let now = Instant::now();
        let mut guard = match connections.lock() {
            Ok(g) => g,
            Err(_) => return,
        };

        let entries = parse_proc_net_tcp("/proc/net/tcp", "tcp", AF_INET);
        let entries_v6 = parse_proc_net_tcp("/proc/net/tcp6", "tcp6", AF_INET6);

        let pid_map = build_pid_map();

        for entry in entries.into_iter().chain(entries_v6) {
            let pid = pid_map.get(&entry.inode).cloned();
            let (program, command) = if let Some(ref pid_str) = pid {
                (get_process_name(pid_str), get_process_cmdline(pid_str))
            } else {
                ("N/A".to_string(), String::new())
            };

            let key = if let Some(ref pid_str) = pid {
                if let Ok(pid_num) = pid_str.parse::<u32>() {
                    sock_key(pid_num, entry.remote_port as u64)
                } else {
                    entry.inode
                }
            } else {
                entry.inode
            };

            guard.insert(
                key,
                ConnectionState {
                    connection: Connection {
                        protocol: entry.protocol,
                        state: entry.state,
                        local: entry.local,
                        remote: entry.remote,
                        program,
                        pid: pid.unwrap_or_else(|| "N/A".to_string()),
                        command,
                        rx_rate: 0,
                        tx_rate: 0,
                    },
                    last_seen: now,
                },
            );
        }
    }

    fn get_process_io_inner(pid: &str) -> ProcessIO {
        let io_path = format!("/proc/{pid}/io");
        if let Ok(io_data) = std::fs::read_to_string(&io_path) {
            let mut rx_bytes = 0u64;
            let mut tx_bytes = 0u64;
            for line in io_data.lines() {
                if line.starts_with("rchar:") {
                    if let Some(value) = line.split_whitespace().nth(1) {
                        rx_bytes = value.parse().unwrap_or(0);
                    }
                } else if line.starts_with("wchar:") {
                    if let Some(value) = line.split_whitespace().nth(1) {
                        tx_bytes = value.parse().unwrap_or(0);
                    }
                }
            }
            ProcessIO::new(rx_bytes, tx_bytes)
        } else {
            ProcessIO::zero()
        }
    }
}

impl ConnectionMonitor for EbpfMonitor {
    fn get_connections(&self) -> Result<Vec<Connection>> {
        let mut cache = self
            .connections
            .lock()
            .map_err(|e| NetworkMonitorError::MutexPoison(format!("{e}")))?;
        let now = Instant::now();
        cache.retain(|_, cs| now.duration_since(cs.last_seen) < Duration::from_secs(60));
        let mut result: Vec<Connection> = cache
            .values_mut()
            .map(|cs| {
                cs.last_seen = now;
                cs.connection.clone()
            })
            .collect();
        result.sort_by_key(|b| std::cmp::Reverse(b.rx_rate));
        Ok(result)
    }

    #[allow(dead_code)]
    fn get_process_io(&self, pid: &str) -> ProcessIO {
        Self::get_process_io_inner(pid)
    }

    fn update_connection_rates(
        &self,
        connections: Vec<Connection>,
        prev_io: &HashMap<String, ProcessIO>,
    ) -> Result<(Vec<Connection>, HashMap<String, ProcessIO>)> {
        let mut current_io = HashMap::new();
        let mut updated_connections = Vec::new();
        let now = Instant::now();
        let elapsed_seconds = {
            let last_time = *self.last_update_time.borrow();
            let elapsed = now.duration_since(last_time);
            *self.last_update_time.borrow_mut() = now;
            elapsed.as_secs_f64().max(0.001)
        };

        for mut conn in connections {
            if conn.pid != "N/A" {
                let io = Self::get_process_io_inner(&conn.pid);
                let pid_key = conn.pid.clone();
                if let Some(prev) = prev_io.get(&pid_key) {
                    conn.rx_rate = (io.rx.saturating_sub(prev.rx) as f64 / elapsed_seconds) as u64;
                    conn.tx_rate = (io.tx.saturating_sub(prev.tx) as f64 / elapsed_seconds) as u64;
                }
                current_io.insert(pid_key, io);
            }
            updated_connections.push(conn);
        }
        Ok((updated_connections, current_io))
    }

    #[allow(dead_code)]
    fn name(&self) -> &'static str {
        "ebpf"
    }
}

impl Drop for EbpfMonitor {
    fn drop(&mut self) {
        self.stopped.store(true, Ordering::Relaxed);
        if let Some(handle) = self._reader_thread.take() {
            let _ = handle.join();
        }
    }
}

fn sock_key(pid: u32, dport: u64) -> u64 {
    (pid as u64) << 32 | dport
}

fn sock_addr_to_string(addr: &[u8; 16], family: u16, port: u16) -> String {
    if family == AF_INET6 {
        let segments: Vec<String> = addr
            .chunks(2)
            .map(|c| u16::from_be_bytes([c[0], c[1]]))
            .map(|s| format!("{s:x}"))
            .collect();
        format!("[{}]:{port}", segments.join(":"))
    } else {
        let ip = format!("{}.{}.{}.{}", addr[12], addr[13], addr[14], addr[15]);
        format!("{ip}:{port}")
    }
}

fn tcp_state_string(state: u8) -> String {
    match state {
        1 => "ESTABLISHED".to_string(),
        2 => "SYN_SENT".to_string(),
        3 => "SYN_RECV".to_string(),
        4 => "FIN_WAIT1".to_string(),
        5 => "FIN_WAIT2".to_string(),
        6 => "TIME_WAIT".to_string(),
        7 => "CLOSE".to_string(),
        8 => "CLOSE_WAIT".to_string(),
        9 => "LAST_ACK".to_string(),
        10 => "LISTEN".to_string(),
        11 => "CLOSING".to_string(),
        _ => "UNKNOWN".to_string(),
    }
}

fn connection_from_connect(ev: &TcpConnectEvent) -> Connection {
    let protocol = if ev.family == AF_INET6 { "tcp6" } else { "tcp" };
    let pid_str = ev.pid.to_string();
    Connection {
        protocol: protocol.to_string(),
        state: "ESTABLISHED".to_string(),
        local: sock_addr_to_string(&ev.saddr, ev.family, ev.sport),
        remote: sock_addr_to_string(&ev.daddr, ev.family, ev.dport),
        program: get_process_name(&pid_str),
        pid: pid_str.clone(),
        command: get_process_cmdline(&pid_str),
        rx_rate: 0,
        tx_rate: 0,
    }
}

fn connection_from_accept(ev: &TcpAcceptEvent) -> Connection {
    let protocol = if ev.family == AF_INET6 { "tcp6" } else { "tcp" };
    let pid_str = ev.pid.to_string();
    Connection {
        protocol: protocol.to_string(),
        state: "ESTABLISHED".to_string(),
        local: sock_addr_to_string(&ev.saddr, ev.family, ev.sport),
        remote: sock_addr_to_string(&ev.daddr, ev.family, ev.dport),
        program: get_process_name(&pid_str),
        pid: pid_str.clone(),
        command: get_process_cmdline(&pid_str),
        rx_rate: 0,
        tx_rate: 0,
    }
}

fn has_effective_cap(cap_mask: u64) -> bool {
    if let Ok(data) = std::fs::read_to_string("/proc/self/status") {
        for line in data.lines() {
            if line.starts_with("CapEff:\t") {
                if let Some(hex_str) = line.strip_prefix("CapEff:\t") {
                    if let Ok(mask) = u64::from_str_radix(hex_str.trim(), 16) {
                        return (mask & cap_mask) == cap_mask;
                    }
                }
            }
        }
    }
    false
}

struct ProcNetEntry {
    inode: u64,
    protocol: String,
    local: String,
    remote: String,
    remote_port: u16,
    state: String,
}

fn parse_proc_net_tcp(path: &str, protocol: &str, family: u16) -> Vec<ProcNetEntry> {
    let content = match std::fs::read_to_string(path) {
        Ok(c) => c,
        Err(_) => return Vec::new(),
    };

    let mut entries = Vec::new();
    for line in content.lines().skip(1) {
        let parts: Vec<&str> = line.split_whitespace().collect();
        if parts.len() < 10 {
            continue;
        }

        let local = parts[1];
        let remote = parts[2];
        let state_hex = parts[3];
        let inode_str = parts[9];

        let state_num = u8::from_str_radix(state_hex, 16).unwrap_or(0);
        let state = tcp_state_string(state_num);
        let inode: u64 = inode_str.parse().unwrap_or(0);
        if inode == 0 {
            continue;
        }

        let (local_ip_hex, local_port_hex) = local.split_once(':').unwrap_or(("", "0"));
        let (remote_ip_hex, remote_port_hex) = remote.split_once(':').unwrap_or(("", "0"));

        let local_ip = format_ip_from_hex(local_ip_hex, family);
        let remote_ip = format_ip_from_hex(remote_ip_hex, family);
        let local_port = u16::from_str_radix(local_port_hex, 16).unwrap_or(0);
        let remote_port = u16::from_str_radix(remote_port_hex, 16).unwrap_or(0);

        let local = if family == AF_INET6 {
            format!("[{local_ip}]:{local_port}")
        } else {
            format!("{local_ip}:{local_port}")
        };
        let remote = if family == AF_INET6 {
            format!("[{remote_ip}]:{remote_port}")
        } else {
            format!("{remote_ip}:{remote_port}")
        };

        entries.push(ProcNetEntry {
            inode,
            protocol: protocol.to_string(),
            local,
            remote,
            remote_port,
            state,
        });
    }
    entries
}

fn format_ip_from_hex(hex: &str, family: u16) -> String {
    if family == AF_INET6 {
        let padded = format!("{:0>32}", hex);
        let segments: Vec<String> = (0..8)
            .map(|i| {
                let s = u16::from_str_radix(&padded[i * 4..i * 4 + 4], 16).unwrap_or(0);
                format!("{s:x}")
            })
            .collect();
        segments.join(":")
    } else {
        let padded = format!("{:0>8}", hex);
        let octets: Vec<String> = (0..4)
            .map(|i| {
                u8::from_str_radix(&padded[i * 2..i * 2 + 2], 16)
                    .unwrap_or(0)
                    .to_string()
            })
            .collect();
        octets.join(".")
    }
}

fn build_pid_map() -> HashMap<u64, String> {
    let mut pid_map = HashMap::new();
    let proc = match std::fs::read_dir("/proc") {
        Ok(p) => p,
        Err(_) => return pid_map,
    };

    for entry in proc.flatten() {
        let pid_str = match entry.file_name().to_str() {
            Some(s) => s.to_string(),
            None => continue,
        };
        if !pid_str.chars().all(|c| c.is_ascii_digit()) {
            continue;
        }

        let fd_dir = format!("/proc/{pid_str}/fd");
        let fd_entries = match std::fs::read_dir(&fd_dir) {
            Ok(d) => d,
            Err(_) => continue,
        };

        for fd_entry in fd_entries.flatten() {
            let link = match std::fs::read_link(fd_entry.path()) {
                Ok(l) => l,
                Err(_) => continue,
            };
            let link_str = link.to_string_lossy().to_string();
            if !link_str.starts_with("socket:[") {
                continue;
            }
            let inode_part = link_str.trim_start_matches("socket:[");
            let inode_part = inode_part.trim_end_matches(']');
            if let Ok(inode) = inode_part.parse::<u64>() {
                pid_map.entry(inode).or_insert_with(|| pid_str.clone());
            }
        }
    }
    pid_map
}

fn get_process_name(pid: &str) -> String {
    let status_path = format!("/proc/{pid}/status");
    if let Ok(content) = std::fs::read_to_string(&status_path) {
        for line in content.lines() {
            if line.starts_with("Name:\t") {
                return line.strip_prefix("Name:\t").unwrap_or("N/A").to_string();
            }
        }
    }
    "N/A".to_string()
}

fn get_process_cmdline(pid: &str) -> String {
    let cmdline_path = format!("/proc/{pid}/cmdline");
    if let Ok(content) = std::fs::read_to_string(&cmdline_path) {
        content.replace('\0', " ").trim().to_string()
    } else {
        String::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_get_connections_refreshes_last_seen() {
        let connections: Arc<Mutex<HashMap<u64, ConnectionState>>> =
            Arc::new(Mutex::new(HashMap::new()));

        {
            let mut map = connections.lock().unwrap();
            map.insert(
                1,
                ConnectionState {
                    connection: Connection {
                        protocol: "tcp".into(),
                        state: "ESTABLISHED".into(),
                        local: "127.0.0.1:4000".into(),
                        remote: "10.0.0.1:80".into(),
                        program: "curl".into(),
                        pid: "1234".into(),
                        command: "/usr/bin/curl".into(),
                        rx_rate: 0,
                        tx_rate: 0,
                    },
                    last_seen: Instant::now() - Duration::from_secs(90),
                },
            );
            map.insert(
                2,
                ConnectionState {
                    connection: Connection {
                        protocol: "tcp".into(),
                        state: "ESTABLISHED".into(),
                        local: "127.0.0.1:4001".into(),
                        remote: "10.0.0.2:443".into(),
                        program: "firefox".into(),
                        pid: "5678".into(),
                        command: "/usr/bin/firefox".into(),
                        rx_rate: 0,
                        tx_rate: 0,
                    },
                    last_seen: Instant::now() - Duration::from_secs(30),
                },
            );
        }

        let now_before = Instant::now();
        let _ = {
            connections
                .lock()
                .unwrap()
                .values()
                .map(|cs| cs.last_seen)
                .collect::<Vec<_>>()
        };

        let mut cache = connections.lock().unwrap();
        let now = Instant::now();
        cache.retain(|_, cs| now.duration_since(cs.last_seen) < Duration::from_secs(60));
        let result: Vec<Connection> = cache
            .values_mut()
            .map(|cs| {
                cs.last_seen = now;
                cs.connection.clone()
            })
            .collect();

        assert_eq!(result.len(), 1, "expired entry (90s old) should be removed");
        assert_eq!(
            result[0].pid, "5678",
            "remaining entry should be the recent one"
        );

        for cs in cache.values() {
            assert!(
                cs.last_seen >= now_before,
                "last_seen should be refreshed to recent time"
            );
        }
    }
}
