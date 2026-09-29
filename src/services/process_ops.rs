//! Process operations shared by the GTK and TUI frontends.

use crate::error::{NetworkMonitorError, Result};
use nix::sys::signal::{kill, Signal};
use nix::unistd::Pid;

/// Signal used to ask a process to exit.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum KillSignal {
    /// Graceful shutdown request (`SIGTERM`).
    Term,
    /// Immediate, non-recoverable termination (`SIGKILL`).
    Force,
}

impl KillSignal {
    pub fn signal(self) -> Signal {
        match self {
            KillSignal::Term => Signal::SIGTERM,
            KillSignal::Force => Signal::SIGKILL,
        }
    }

    /// Short signal name, e.g. `SIGTERM`.
    pub fn label(self) -> &'static str {
        match self {
            KillSignal::Term => "SIGTERM",
            KillSignal::Force => "SIGKILL",
        }
    }

    /// Human readable action label for menus and dialogs.
    pub fn description(self) -> &'static str {
        match self {
            KillSignal::Term => "Terminate",
            KillSignal::Force => "Force Kill",
        }
    }
}

/// Validate a pid coming from a [`crate::models::Connection`].
///
/// Connections may carry placeholder values (`N/A`, `...`) or data coming from
/// `/proc`, so the string is checked before it is turned into a number.
pub fn parse_pid(pid: &str) -> Result<i32> {
    let trimmed = pid.trim();

    if trimmed.is_empty() || !trimmed.chars().all(|c| c.is_ascii_digit()) || trimmed.len() > 10 {
        return Err(NetworkMonitorError::InvalidPid(pid.to_string()));
    }

    let parsed: i32 = trimmed
        .parse()
        .map_err(|_| NetworkMonitorError::InvalidPid(pid.to_string()))?;

    // Never allow signalling the kernel/init or ourselves by accident.
    if parsed <= 1 {
        return Err(NetworkMonitorError::InvalidPid(pid.to_string()));
    }
    if parsed == std::process::id() as i32 {
        return Err(NetworkMonitorError::InvalidPid(format!(
            "{parsed} (this application)"
        )));
    }

    Ok(parsed)
}

/// Send `signal` to the process identified by `pid`.
pub fn kill_process(pid: &str, signal: KillSignal) -> Result<()> {
    let parsed = parse_pid(pid)?;

    kill(Pid::from_raw(parsed), signal.signal()).map_err(|errno| {
        NetworkMonitorError::ProcessKillError(format!(
            "{} on PID {} failed: {}",
            signal.label(),
            parsed,
            errno
        ))
    })?;

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_pid_rejects_placeholders() {
        assert!(parse_pid("N/A").is_err());
        assert!(parse_pid("...").is_err());
        assert!(parse_pid("").is_err());
        assert!(parse_pid("abc").is_err());
        assert!(parse_pid("-5").is_err());
        assert!(parse_pid("1").is_err());
        assert!(parse_pid("0").is_err());
    }

    #[test]
    fn parse_pid_accepts_real_pids() {
        assert_eq!(parse_pid("4242").ok(), Some(4242));
        assert_eq!(parse_pid(" 4242 ").ok(), Some(4242));
        assert!(parse_pid(&std::process::id().to_string()).is_err());
    }

    #[test]
    fn kill_signal_labels() {
        assert_eq!(KillSignal::Term.label(), "SIGTERM");
        assert_eq!(KillSignal::Force.label(), "SIGKILL");
    }
}
