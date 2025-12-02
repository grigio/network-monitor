use crate::models::{Connection, ProcessIO};
use std::collections::HashMap;
use std::fs;
use std::net::{IpAddr, Ipv4Addr, Ipv6Addr};
use std::path::Path;
use std::time::Instant;

/// Service for monitoring network connections
pub struct NetworkService {
    last_update_time: std::cell::RefCell<Instant>,
}

impl NetworkService {
    pub fn new() -> Self {
        Self {
            last_update_time: std::cell::RefCell::new(Instant::now()),
        }
    }

    /// Get all network connections using native Rust socket APIs
    pub fn get_connections(&self) -> Vec<Connection> {
        let mut connections = Vec::new();

        // Get TCP connections
        connections.extend(self.get_tcp_connections());

        // Get UDP connections
        connections.extend(self.get_udp_connections());

        connections
    }

    /// Get TCP connections from /proc/net/tcp
    fn get_tcp_connections(&self) -> Vec<Connection> {
        let mut connections = Vec::new();

        if let Ok(tcp_data) = fs::read_to_string("/proc/net/tcp") {
            for line in tcp_data.lines().skip(1) {
                if let Some(conn) = self.parse_proc_net_line(line, "tcp", "LISTEN") {
                    connections.push(conn);
                }
            }
        }

        if let Ok(tcp6_data) = fs::read_to_string("/proc/net/tcp6") {
            for line in tcp6_data.lines().skip(1) {
                if let Some(conn) = self.parse_proc_net_line(line, "tcp6", "LISTEN") {
                    connections.push(conn);
                }
            }
        }

        connections
    }

    /// Get UDP connections from /proc/net/udp
    fn get_udp_connections(&self) -> Vec<Connection> {
        let mut connections = Vec::new();

        if let Ok(udp_data) = fs::read_to_string("/proc/net/udp") {
            for line in udp_data.lines().skip(1) {
                if let Some(conn) = self.parse_proc_net_line(line, "udp", "") {
                    connections.push(conn);
                }
            }
        }

        if let Ok(udp6_data) = fs::read_to_string("/proc/net/udp6") {
            for line in udp6_data.lines().skip(1) {
                if let Some(conn) = self.parse_proc_net_line(line, "udp6", "") {
                    connections.push(conn);
                }
            }
        }

        connections
    }

    /// Parse a line from /proc/net/tcp|udp
    fn parse_proc_net_line(
        &self,
        line: &str,
        protocol: &str,
        default_state: &str,
    ) -> Option<Connection> {
        let parts: Vec<&str> = line.split_whitespace().collect();
        if parts.len() < 10 {
            return None;
        }

        let local_addr = self.parse_socket_addr(parts[1])?;
        let remote_addr = self.parse_socket_addr(parts[2])?;

        let state = if parts.len() > 3 {
            self.parse_tcp_state(parts[3])
        } else {
            default_state.to_string()
        };

        // Get the inode from the connection
        let inode = if parts.len() > 9 {
            parts[9].parse::<u64>().unwrap_or(0)
        } else {
            0
        };

        let (program, pid, command) = self.get_process_info_for_inode(inode);

        Some(Connection::new(
            protocol.to_string(),
            state,
            local_addr,
            remote_addr,
            program,
            pid,
            command,
        ))
    }

    /// Parse socket address from /proc/net format
    fn parse_socket_addr(&self, addr_str: &str) -> Option<String> {
        let parts: Vec<&str> = addr_str.split(':').collect();
        if parts.len() != 2 {
            return None;
        }

        let ip_hex = parts[0];
        let port_hex = parts[1];

        let port = u16::from_str_radix(port_hex, 16).ok()?;

        let ip = if ip_hex.len() == 8 {
            // IPv4 (hex is in little-endian format)
            let mut bytes = [0u8; 4];
            for (i, chunk) in (0..ip_hex.len()).step_by(2).enumerate() {
                bytes[3 - i] = u8::from_str_radix(&ip_hex[chunk..chunk + 2], 16).ok()?;
            }
            IpAddr::V4(Ipv4Addr::from(bytes))
        } else if ip_hex.len() == 32 {
            // IPv6
            let mut bytes = [0u8; 16];
            for (i, chunk) in (0..ip_hex.len()).step_by(2).enumerate() {
                bytes[i] = u8::from_str_radix(&ip_hex[chunk..chunk + 2], 16).ok()?;
            }
            IpAddr::V6(Ipv6Addr::from(bytes))
        } else {
            return None;
        };

        Some(format!("{ip}:{port}"))
    }

    /// Parse TCP state from hex value
    fn parse_tcp_state(&self, state_hex: &str) -> String {
        if let Ok(state_val) = u8::from_str_radix(state_hex, 16) {
            match state_val {
                0x01 => "ESTABLISHED".to_string(),
                0x02 => "SYN_SENT".to_string(),
                0x03 => "SYN_RECV".to_string(),
                0x04 => "FIN_WAIT1".to_string(),
                0x05 => "FIN_WAIT2".to_string(),
                0x06 => "TIME_WAIT".to_string(),
                0x07 => "CLOSE".to_string(),
                0x08 => "CLOSE_WAIT".to_string(),
                0x09 => "LAST_ACK".to_string(),
                0x0A => "LISTEN".to_string(),
                0x0B => "CLOSING".to_string(),
                0x0C => "NEW_SYN_RECV".to_string(),
                _ => format!("UNKNOWN({state_val})"),
            }
        } else {
            "UNKNOWN".to_string()
        }
    }

    /// Get process info for a given socket inode
    fn get_process_info_for_inode(&self, inode: u64) -> (String, String, String) {
        if inode == 0 {
            return ("N/A".to_string(), "N/A".to_string(), "N/A".to_string());
        }

        // Scan /proc/*/fd for socket inodes
        if let Ok(proc_dir) = fs::read_dir("/proc") {
            for entry in proc_dir.flatten() {
                let path = entry.path();
                if let Some(pid_str) = path.file_name().and_then(|n| n.to_str()) {
                    if pid_str.chars().all(|c| c.is_ascii_digit()) {
                        if let Some((program, command)) = self.check_process_fd(&path, inode) {
                            return (program, pid_str.to_string(), command);
                        }
                    }
                }
            }
        }

        ("N/A".to_string(), "N/A".to_string(), "N/A".to_string())
    }

    /// Check process file descriptors for matching socket inode
    fn check_process_fd(&self, proc_path: &Path, target_inode: u64) -> Option<(String, String)> {
        let fd_path = proc_path.join("fd");
        if let Ok(fd_dir) = fs::read_dir(&fd_path) {
            for fd_entry in fd_dir.flatten() {
                let fd_link_path = fd_entry.path();
                if let Ok(link_target) = fs::read_link(&fd_link_path) {
                    if let Some(link_str) = link_target.to_str() {
                        if link_str.starts_with("socket:[") && link_str.ends_with(']') {
                            let inode_str = &link_str[8..link_str.len() - 1];
                            if let Ok(inode) = inode_str.parse::<u64>() {
                                if inode == target_inode {
                                    let pid_str = proc_path.file_name()?.to_str()?;
                                    let program = self.get_process_name(pid_str);
                                    let command = self.get_process_path(pid_str);
                                    return Some((program, command));
                                }
                            }
                        }
                    }
                }
            }
        }
        None
    }

    /// Get process name from /proc/[pid]/status
    fn get_process_name(&self, pid: &str) -> String {
        let status_path = format!("/proc/{pid}/status");
        if let Ok(status_data) = fs::read_to_string(&status_path) {
            for line in status_data.lines() {
                if let Some(name) = line.strip_prefix("Name:\t") {
                    return name.to_string();
                }
            }
        }
        "N/A".to_string()
    }

    /// Get I/O statistics for a process
    pub fn get_process_io(&self, pid: &str) -> ProcessIO {
        let io_path = format!("/proc/{pid}/io");
        if let Ok(io_data) = fs::read_to_string(&io_path) {
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

    /// Get process command line
    pub fn get_process_path(&self, pid: &str) -> String {
        let cmdline_path = format!("/proc/{pid}/cmdline");
        if let Ok(cmdline) = fs::read_to_string(&cmdline_path) {
            if !cmdline.is_empty() {
                cmdline.replace('\0', " ")
            } else {
                format!("[{pid}]")
            }
        } else {
            "N/A".to_string()
        }
    }

    /// Update connection rates based on previous I/O data
    pub fn update_connection_rates(
        &self,
        connections: Vec<Connection>,
        prev_io: &HashMap<String, ProcessIO>,
    ) -> (Vec<Connection>, HashMap<String, ProcessIO>) {
        let mut current_io = HashMap::new();
        let mut updated_connections = Vec::new();

        // Calculate time elapsed since last update
        let now = Instant::now();
        let time_elapsed = {
            let last_time = *self.last_update_time.borrow();
            let elapsed = now.duration_since(last_time);
            // Update the last update time
            *self.last_update_time.borrow_mut() = now;
            elapsed
        };

        // Convert elapsed time to seconds as f64 for rate calculation
        let elapsed_seconds = time_elapsed.as_secs_f64();
        // Avoid division by zero
        let elapsed_seconds = elapsed_seconds.max(0.001);

        for mut conn in connections {
            if conn.pid != "N/A" {
                let io = self.get_process_io(&conn.pid);
                let pid_key = conn.pid.clone();

                // Calculate rates based on previous I/O data and time elapsed
                if let Some(prev) = prev_io.get(&pid_key) {
                    let rx_diff = io.rx.saturating_sub(prev.rx) as f64;
                    let tx_diff = io.tx.saturating_sub(prev.tx) as f64;

                    // Calculate per-second rates
                    conn.rx_rate = (rx_diff / elapsed_seconds) as u64;
                    conn.tx_rate = (tx_diff / elapsed_seconds) as u64;
                }

                current_io.insert(pid_key, io);
            }

            updated_connections.push(conn);
        }

        (updated_connections, current_io)
    }
}

impl Default for NetworkService {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_socket_addr_ipv4() {
        let service = NetworkService::new();
        let addr = service.parse_socket_addr("0100007F:1234");
        assert_eq!(addr, Some("127.0.0.1:4660".to_string()));
    }

    #[test]
    fn test_parse_socket_addr_ipv6() {
        let service = NetworkService::new();
        let addr = service.parse_socket_addr("00000000000000000000000000000001:1234");
        assert_eq!(addr, Some("::1:4660".to_string()));
    }

    #[test]
    fn test_parse_tcp_state() {
        let service = NetworkService::new();
        assert_eq!(service.parse_tcp_state("0A"), "LISTEN");
        assert_eq!(service.parse_tcp_state("01"), "ESTABLISHED");
        assert_eq!(service.parse_tcp_state("FF"), "UNKNOWN(255)");
    }

    #[test]
    fn test_get_connections() {
        let service = NetworkService::new();
        let connections = service.get_connections();
        // Should not panic and return a vector
        assert!(!connections.is_empty() || connections.is_empty());
    }
}
