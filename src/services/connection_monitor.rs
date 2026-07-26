use crate::error::Result;
use crate::models::{Connection, ProcessIO};
use std::collections::HashMap;

#[allow(dead_code)]
pub trait ConnectionMonitor: Send {
    fn get_connections(&self) -> Result<Vec<Connection>>;
    fn get_process_io(&self, pid: &str) -> ProcessIO;
    fn update_connection_rates(
        &self,
        connections: Vec<Connection>,
        prev_io: &HashMap<String, ProcessIO>,
    ) -> Result<(Vec<Connection>, HashMap<String, ProcessIO>)>;
    fn name(&self) -> &'static str;
}

pub fn detect_best_monitor() -> Result<Box<dyn ConnectionMonitor>> {
    let monitor = crate::services::ebpf_monitor::EbpfMonitor::new().map_err(|e| {
        eprintln!("Warning: Failed to create eBPF monitor: {e}");
        eprintln!("eBPF requires Linux 5.8+ and the following capabilities:");
        eprintln!("  sudo setcap cap_bpf,cap_net_admin,cap_perfmon+ep <binary>");
        e
    })?;
    Ok(Box::new(monitor))
}
