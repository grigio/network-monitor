use crate::models::{Connection, ProcessIO};
use regex::Regex;
use std::collections::HashMap;
use std::fs;
use std::os::unix::process::ExitStatusExt;
use std::process::Command;
use std::time::Instant;

/// Service for monitoring network connections
pub struct NetworkService {
    pid_regex: Regex,
    prog_regex: Regex,
    last_update_time: std::cell::RefCell<Instant>,
}

impl NetworkService {
    pub fn new() -> Self {
        Self {
            pid_regex: Regex::new(r"pid=(\d+)").unwrap(),
            prog_regex: Regex::new(r#""([^"]+)""#).unwrap(),
            last_update_time: std::cell::RefCell::new(Instant::now()),
        }
    }

    /// Get all network connections using ss command
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
