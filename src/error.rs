#[derive(Debug, thiserror::Error)]
#[allow(dead_code)]
pub enum NetworkMonitorError {
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),

    #[error("Failed to parse network data: {0}")]
    ParseError(String),

    #[error("Mutex lock poisoned: {0}")]
    MutexPoison(String),

    #[error("Failed to resolve hostname: {0}")]
    ResolutionError(String),

    #[error("GTK initialization failed")]
    GtkInitError,

    #[error("Terminal initialization failed")]
    TerminalError,

    #[error("eBPF is not available: {0}")]
    EbpfNotAvailable(String),

    #[error("eBPF program failed to load: {0}")]
    EbpfLoadError(String),

    #[error("eBPF program failed to attach: {0}")]
    EbpfAttachError(String),

    #[error("eBPF permission denied: {0}")]
    EbpfPermissionError(String),
}

pub type Result<T> = std::result::Result<T, NetworkMonitorError>;
