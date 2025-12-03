use serde::{Deserialize, Serialize};

/// Process information for caching
#[derive(Debug, Clone)]
#[allow(dead_code)]
pub struct ProcessInfo {
    pub name: String,
    pub command: String,
    pub last_seen: std::time::Instant,
}

/// Network connection information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Connection {
    pub protocol: String,
    pub state: String,
    pub local: String,
    pub remote: String,
    pub program: String,
    pub pid: String,
    pub command: String,
    pub rx_rate: u64,
    pub tx_rate: u64,
}

impl Connection {
    pub fn new(
        protocol: String,
        state: String,
        local: String,
        remote: String,
        program: String,
        pid: String,
        command: String,
    ) -> Self {
        Self {
            protocol,
            state,
            local,
            remote,
            program,
            pid,
            command,
            rx_rate: 0,
            tx_rate: 0,
        }
    }

    pub fn is_active(&self) -> bool {
        self.rx_rate > 0 || self.tx_rate > 0
    }

    pub fn get_process_display(&self) -> String {
        if self.pid != "N/A" {
            format!("{}({})", self.program, self.pid)
        } else {
            self.program.clone()
        }
    }
}

/// Process I/O statistics
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProcessIO {
    pub rx: u64,
    pub tx: u64,
}

impl ProcessIO {
    pub fn new(rx: u64, tx: u64) -> Self {
        Self { rx, tx }
    }

    pub fn zero() -> Self {
        Self { rx: 0, tx: 0 }
    }
}
