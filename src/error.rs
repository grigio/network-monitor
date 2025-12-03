use std::sync::PoisonError;

/// Custom error types for Network Monitor
#[derive(Debug, thiserror::Error)]
#[allow(dead_code)]
pub enum NetworkMonitorError {
    #[error("Failed to read /proc filesystem: {0}")]
    ProcIo(#[from] std::io::Error),

    #[error("Invalid socket address format: {0}")]
    InvalidAddress(String),

    #[error("Process not found: {0}")]
    ProcessNotFound(String),

    #[error("Failed to parse network data: {0}")]
    ParseError(String),

    #[error("Failed to parse hex value: {0}")]
    HexParseError(String),

    #[error("Invalid PID format: {0}")]
    InvalidPid(String),

    #[error("Mutex lock poisoned: {0}")]
    MutexPoison(String),

    #[error("Failed to resolve hostname: {0}")]
    ResolutionError(String),

    #[error("GTK initialization failed")]
    GtkInitError,

    #[error("Terminal initialization failed")]
    TerminalError,
}

/// Result type alias for convenience
pub type Result<T> = std::result::Result<T, NetworkMonitorError>;

/// Trait for converting mutex poisoning errors
#[allow(dead_code)]
pub trait MutexResult<T> {
    fn handle_mutex(self, context: &str) -> Result<T>;
}

impl<T> MutexResult<T> for std::result::Result<T, PoisonError<T>> {
    fn handle_mutex(self, context: &str) -> Result<T> {
        self.map_err(|_| NetworkMonitorError::MutexPoison(context.to_string()))
    }
}

/// Macro for consistent error context
#[macro_export]
macro_rules! error_context {
    ($result:expr, $context:expr) => {
        $result
            .map_err(|e| $crate::error::NetworkMonitorError::from(e))
            .map_err(|e| {
                $crate::error::NetworkMonitorError::ParseError(format!("{}: {}", $context, e))
            })
    };
}
