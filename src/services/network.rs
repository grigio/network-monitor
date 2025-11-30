use crate::models::{Connection, ProcessIO};
use regex::Regex;
use std::collections::HashMap;
use std::fs;
#[cfg(target_os = "linux")]
use std::os::unix::process::ExitStatusExt;
use std::process::Command;

/// Service for monitoring network connections
pub struct NetworkService {
    pid_regex: Regex,
    prog_regex: Regex,
}

impl NetworkService {
    pub fn new() -> Self {
        Self {
            pid_regex: Regex::new(r"pid=(\d+)").unwrap(),
            prog_regex: Regex::new(r#""([^"]+)""#).unwrap(),
        }
    }

    /// Get all network connections using ss command (Linux)
    #[cfg(target_os = "linux")]
    pub fn get_connections(&self) -> Vec<Connection> {
        let output = Command::new("ss")
            .args(["-tulnape"])
            .output()
            .unwrap_or_else(|_| std::process::Output {
                status: std::process::ExitStatus::from_raw(1),
                stdout: Vec::new(),
                stderr: Vec::new(),
            });

        let stdout = String::from_utf8_lossy(&output.stdout);
        let lines: Vec<&str> = stdout.lines().skip(1).collect();

        let mut connections = Vec::new();

        for line in lines {
            if line.trim().is_empty() {
                continue;
            }

            let parts: Vec<&str> = line.split_whitespace().collect();
            if parts.len() < 6 {
                continue;
            }

            let protocol = parts[0].to_string();
            let state = parts[1].to_string();
            let local_addr = parts[4].to_string();
            let remote_addr = parts[5].to_string();

            let mut program = "N/A".to_string();
            let mut pid = "N/A".to_string();

            for part in &parts {
                if part.starts_with("users:((") {
                    if let Some(caps) = self.pid_regex.captures(part) {
                        pid = caps[1].to_string();
                    }
                    if let Some(caps) = self.prog_regex.captures(part) {
                        program = caps[1].to_string();
                    }
                    break;
                }
            }

            connections.push(Connection::new(
                protocol,
                state,
                local_addr,
                remote_addr,
                program,
                pid,
            ));
        }

        connections
    }

    /// Get all network connections using lsof command (macOS)
    #[cfg(target_os = "macos")]
    pub fn get_connections(&self) -> Vec<Connection> {
        // Use lsof -i -n -P for network connections
        // -i: internet connections only
        // -n: no DNS resolution (faster)
        // -P: no port name resolution (show numbers)
        let output = Command::new("lsof")
            .args(["-i", "-n", "-P"])
            .output()
            .unwrap_or_else(|_| std::process::Output {
                status: std::process::ExitStatus::default(),
                stdout: Vec::new(),
                stderr: Vec::new(),
            });

        let stdout = String::from_utf8_lossy(&output.stdout);
        let lines: Vec<&str> = stdout.lines().skip(1).collect();

        let mut connections = Vec::new();

        for line in lines {
            if line.trim().is_empty() {
                continue;
            }

            let parts: Vec<&str> = line.split_whitespace().collect();
            if parts.len() < 9 {
                continue;
            }

            let program = parts[0].to_string();
            let pid = parts[1].to_string();
            let protocol = parts[7].to_string();
            
            // Parse connection details from parts[8]
            // Format: local_address->remote_address (STATE) or *:port (LISTEN)
            let conn_info = parts[8];
            
            let (local_addr, remote_addr, state) = if conn_info.contains("->") {
                // Established connection: local->remote (STATE)
                let parts_split: Vec<&str> = conn_info.split("->").collect();
                let local = parts_split[0].to_string();
                let remote_and_state = parts_split.get(1).unwrap_or(&"");
                
                // Extract state if present
                let (remote, state) = if remote_and_state.contains('(') {
                    let idx = remote_and_state.find('(').unwrap();
                    let remote = remote_and_state[..idx].to_string();
                    let state = remote_and_state[idx+1..]
                        .trim_end_matches(')')
                        .to_string();
                    (remote, state)
                } else {
                    (remote_and_state.to_string(), "ESTABLISHED".to_string())
                };
                
                (local, remote, state)
            } else {
                // Listening connection: *:port (LISTEN) or host:port
                let state = if conn_info.contains("LISTEN") {
                    "LISTEN".to_string()
                } else {
                    "UNKNOWN".to_string()
                };
                
                let local = conn_info.replace("(LISTEN)", "").trim().to_string();
                (local, "*:*".to_string(), state)
            };

            connections.push(Connection::new(
                protocol,
                state,
                local_addr,
                remote_addr,
                program,
                pid,
            ));
        }

        connections
    }

    /// Get I/O statistics for a process (Linux)
    #[cfg(target_os = "linux")]
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

    /// Get I/O statistics for a process (macOS)
    #[cfg(target_os = "macos")]
    pub fn get_process_io(&self, pid: &str) -> ProcessIO {
        // macOS doesn't have /proc/[pid]/io
        // We'll use a simpler approach with ps or return approximations
        // For now, we'll use netstat to get per-process network stats
        // This is less accurate but provides similar functionality
        
        // Try to get network statistics from netstat
        let output = Command::new("netstat")
            .args(["-I", "en0", "-b"])
            .output();

        if let Ok(output) = output {
            let stdout = String::from_utf8_lossy(&output.stdout);
            // This is simplified - in a real implementation, we'd parse
            // the netstat output more carefully
            // For now, return zero as this requires more complex parsing
            ProcessIO::zero()
        } else {
            ProcessIO::zero()
        }
    }

    /// Get process command line (Linux)
    #[cfg(target_os = "linux")]
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

    /// Get process command line (macOS)
    #[cfg(target_os = "macos")]
    pub fn get_process_path(&self, pid: &str) -> String {
        // Use ps to get process command line on macOS
        let output = Command::new("ps")
            .args(["-p", pid, "-o", "command="])
            .output();

        if let Ok(output) = output {
            let stdout = String::from_utf8_lossy(&output.stdout);
            let cmdline = stdout.trim();
            if !cmdline.is_empty() {
                cmdline.to_string()
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

        for mut conn in connections {
            if conn.pid != "N/A" {
                let io = self.get_process_io(&conn.pid);
                let pid_key = conn.pid.clone();

                // Calculate rates based on previous I/O data
                if let Some(prev) = prev_io.get(&pid_key) {
                    conn.rx_rate = io.rx.saturating_sub(prev.rx);
                    conn.tx_rate = io.tx.saturating_sub(prev.tx);
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
