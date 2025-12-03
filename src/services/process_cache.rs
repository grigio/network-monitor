use crate::models::connection::ProcessInfo;
use std::collections::HashMap;
use std::fs;
use std::path::Path;
use std::time::{Duration, Instant};

/// Cache for mapping socket inodes to process information
pub struct ProcessCache {
    inode_to_pid: HashMap<u64, String>,
    pid_to_process: HashMap<String, ProcessInfo>,
    last_update: Instant,
    update_interval: Duration,
}

impl ProcessCache {
    pub fn new() -> Self {
        Self {
            inode_to_pid: HashMap::new(),
            pid_to_process: HashMap::new(),
            last_update: Instant::now(),
            update_interval: Duration::from_secs(5), // Update every 5 seconds
        }
    }

    /// Get process info for a given socket inode
    pub fn get_process_info(&mut self, inode: u64) -> (String, String, String) {
        if inode == 0 {
            return ("N/A".to_string(), "N/A".to_string(), "N/A".to_string());
        }

        // Update cache if needed - handle errors gracefully
        if self.last_update.elapsed() > self.update_interval {
            // Ignore cache update errors to prevent app crashes
            let _ = self.update_cache();
        }

        // Check cache first
        if let Some(pid) = self.inode_to_pid.get(&inode) {
            if let Some(process_info) = self.pid_to_process.get(pid) {
                return (
                    process_info.name.clone(),
                    pid.clone(),
                    process_info.command.clone(),
                );
            }
        }

        // Fallback to direct lookup
        self.lookup_process_info(inode)
    }

    /// Update the cache by scanning /proc filesystem
    fn update_cache(&mut self) -> std::result::Result<(), crate::error::NetworkMonitorError> {
        let mut new_inode_to_pid = HashMap::new();
        let mut new_pid_to_process = HashMap::new();

        if let Ok(proc_dir) = fs::read_dir("/proc") {
            for entry in proc_dir.flatten() {
                let path = entry.path();
                if let Some(pid_str) = path.file_name().and_then(|n| n.to_str()) {
                    if pid_str.chars().all(|c| c.is_ascii_digit()) {
                        // Get process info
                        let (name, command) = self.get_process_details(pid_str);
                        if !name.is_empty() && name != "N/A" {
                            let process_info = ProcessInfo {
                                name: name.clone(),
                                command: command.clone(),
                                last_seen: Instant::now(),
                            };
                            new_pid_to_process.insert(pid_str.to_string(), process_info);

                            // Scan file descriptors for socket inodes
                            if let Some(inodes) = self.get_process_inodes(&path) {
                                for inode in inodes {
                                    new_inode_to_pid.insert(inode, pid_str.to_string());
                                }
                            }
                        }
                    }
                }
            }
        }

        self.inode_to_pid = new_inode_to_pid;
        self.pid_to_process = new_pid_to_process;
        self.last_update = Instant::now();
        Ok(())
    }

    /// Get process details from /proc
    fn get_process_details(&self, pid: &str) -> (String, String) {
        let name = self.get_process_name(pid);
        let command = self.get_process_command(pid);
        (name, command)
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

    /// Get process command from /proc/[pid]/cmdline
    fn get_process_command(&self, pid: &str) -> String {
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

    /// Get all socket inodes for a process
    fn get_process_inodes(&self, proc_path: &Path) -> Option<Vec<u64>> {
        let fd_path = proc_path.join("fd");
        let mut inodes = Vec::new();

        if let Ok(fd_dir) = fs::read_dir(&fd_path) {
            for fd_entry in fd_dir.flatten() {
                let fd_link_path = fd_entry.path();
                // Skip file descriptors we can't read (permission denied for other users' processes)
                if let Ok(link_target) = fs::read_link(&fd_link_path) {
                    if let Some(link_str) = link_target.to_str() {
                        if link_str.starts_with("socket:[") && link_str.ends_with(']') {
                            let inode_str = &link_str[8..link_str.len() - 1];
                            if let Ok(inode) = inode_str.parse::<u64>() {
                                inodes.push(inode);
                            }
                        }
                    }
                }
            }
        }

        if inodes.is_empty() {
            None
        } else {
            Some(inodes)
        }
    }

    /// Fallback direct lookup for process info
    fn lookup_process_info(&self, inode: u64) -> (String, String, String) {
        if let Ok(proc_dir) = fs::read_dir("/proc") {
            for entry in proc_dir.flatten() {
                let path = entry.path();
                if let Some(pid_str) = path.file_name().and_then(|n| n.to_str()) {
                    if pid_str.chars().all(|c| c.is_ascii_digit()) {
                        if let Some(inodes) = self.get_process_inodes(&path) {
                            if inodes.contains(&inode) {
                                let (name, command) = self.get_process_details(pid_str);
                                return (name, pid_str.to_string(), command);
                            }
                        }
                    }
                }
            }
        }

        ("N/A".to_string(), "N/A".to_string(), "N/A".to_string())
    }

    /// Clear the cache
    #[allow(dead_code)]
    pub fn clear(&mut self) {
        self.inode_to_pid.clear();
        self.pid_to_process.clear();
        self.last_update = Instant::now();
    }

    /// Set cache update interval
    #[allow(dead_code)]
    pub fn set_update_interval(&mut self, interval: Duration) {
        self.update_interval = interval;
    }
}

impl Default for ProcessCache {
    fn default() -> Self {
        Self::new()
    }
}
