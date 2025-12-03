/// Utility for formatting byte values and other common formatting tasks
pub struct Formatter;

impl Formatter {
    /// Format bytes as human readable string with rate (per second)
    pub fn format_bytes(bytes_val: u64) -> String {
        let mut bytes_val = bytes_val as f64;
        let units = ["B", "KB", "MB", "GB"];

        for unit in &units {
            if bytes_val < 1024.0 {
                return format!("{bytes_val:.1}{unit}/s");
            }
            bytes_val /= 1024.0;
        }
        format!("{bytes_val:.1}TB/s")
    }

    /// Format bytes as human readable string (total)
    #[allow(dead_code)]
    pub fn format_bytes_total(bytes_val: u64) -> String {
        let bytes_val = bytes_val as f64;

        // Always show in MB for consistency, with 2 decimal places
        if bytes_val < 1024.0 {
            format!("{bytes_val:.1} B")
        } else if bytes_val < 1024.0 * 1024.0 {
            format!("{:.1} KB", bytes_val / 1024.0)
        } else {
            format!("{:.2} MB", bytes_val / (1024.0 * 1024.0))
        }
    }

    /// Format bytes with custom precision
    #[allow(dead_code)]
    pub fn format_bytes_precise(bytes_val: u64, precision: usize) -> String {
        let mut bytes_val = bytes_val as f64;
        let units = ["B", "KB", "MB", "GB", "TB"];

        for unit in &units {
            if bytes_val < 1024.0 {
                return format!("{bytes_val:.precision$}{unit}/s", precision = precision);
            }
            bytes_val /= 1024.0;
        }
        format!("{bytes_val:.precision$}PB/s", precision = precision)
    }

    /// Format duration in seconds to human readable string
    #[allow(dead_code)]
    pub fn format_duration(seconds: u64) -> String {
        if seconds < 60 {
            format!("{}s", seconds)
        } else if seconds < 3600 {
            let minutes = seconds / 60;
            let secs = seconds % 60;
            format!("{}m {}s", minutes, secs)
        } else if seconds < 86400 {
            let hours = seconds / 3600;
            let minutes = (seconds % 3600) / 60;
            format!("{}h {}m", hours, minutes)
        } else {
            let days = seconds / 86400;
            let hours = (seconds % 86400) / 3600;
            format!("{}d {}h", days, hours)
        }
    }

    /// Format connection count with proper pluralization
    #[allow(dead_code)]
    pub fn format_connection_count(count: usize) -> String {
        match count {
            0 => "No connections".to_string(),
            1 => "1 connection".to_string(),
            _ => format!("{} connections", count),
        }
    }

    /// Format protocol name consistently
    #[allow(dead_code)]
    pub fn format_protocol(protocol: &str) -> String {
        match protocol.to_uppercase().as_str() {
            "TCP" => "TCP".to_string(),
            "TCP6" => "TCP6".to_string(),
            "UDP" => "UDP".to_string(),
            "UDP6" => "UDP6".to_string(),
            _ => protocol.to_uppercase(),
        }
    }

    /// Format state name consistently
    #[allow(dead_code)]
    pub fn format_state(state: &str) -> String {
        match state {
            "ESTABLISHED" => "ESTABLISHED".to_string(),
            "LISTEN" => "LISTEN".to_string(),
            "TIME_WAIT" => "TIME_WAIT".to_string(),
            "CLOSE_WAIT" => "CLOSE_WAIT".to_string(),
            "SYN_SENT" => "SYN_SENT".to_string(),
            "SYN_RECV" => "SYN_RECV".to_string(),
            "FIN_WAIT1" => "FIN_WAIT1".to_string(),
            "FIN_WAIT2" => "FIN_WAIT2".to_string(),
            "CLOSED" => "CLOSED".to_string(),
            "CLOSING" => "CLOSING".to_string(),
            "LAST_ACK" => "LAST_ACK".to_string(),
            "NEW_SYN_RECV" => "NEW_SYN_RECV".to_string(),
            _ => state.to_string(),
        }
    }

    /// Truncate string to fit within max length with ellipsis
    #[allow(dead_code)]
    pub fn truncate_string(s: &str, max_len: usize) -> String {
        if s.len() <= max_len {
            s.to_string()
        } else if max_len <= 3 {
            "...".to_string()
        } else {
            format!("{}...", &s[..max_len - 3])
        }
    }

    /// Format PID consistently
    #[allow(dead_code)]
    pub fn format_pid(pid: &str) -> String {
        if pid == "N/A" {
            "N/A".to_string()
        } else {
            pid.to_string()
        }
    }

    /// Format program name with fallback
    #[allow(dead_code)]
    pub fn format_program(program: &str) -> String {
        if program.is_empty() || program == "N/A" {
            "Unknown".to_string()
        } else {
            program.to_string()
        }
    }
}

/// Convenience functions for backward compatibility and easier access
#[allow(dead_code)]
pub fn format_bytes(bytes_val: u64) -> String {
    Formatter::format_bytes(bytes_val)
}

#[allow(dead_code)]
pub fn format_bytes_total(bytes_val: u64) -> String {
    Formatter::format_bytes_total(bytes_val)
}

#[allow(dead_code)]
pub fn format_duration(seconds: u64) -> String {
    Formatter::format_duration(seconds)
}

#[allow(dead_code)]
pub fn format_connection_count(count: usize) -> String {
    Formatter::format_connection_count(count)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_format_bytes() {
        assert_eq!(format_bytes(0), "0.0B/s");
        assert_eq!(format_bytes(512), "512.0B/s");
        assert_eq!(format_bytes(1024), "1.0KB/s");
        assert_eq!(format_bytes(1536), "1.5KB/s");
        assert_eq!(format_bytes(1024 * 1024), "1.0MB/s");
        assert_eq!(format_bytes(1024 * 1024 * 1024), "1.0GB/s");
    }

    #[test]
    fn test_format_bytes_total() {
        assert_eq!(format_bytes_total(0), "0.0 B");
        assert_eq!(format_bytes_total(512), "512.0 B");
        assert_eq!(format_bytes_total(1024), "1.0 KB");
        assert_eq!(format_bytes_total(1536), "1.5 KB");
        assert_eq!(format_bytes_total(1024 * 1024), "1.00 MB");
    }

    #[test]
    fn test_format_duration() {
        assert_eq!(format_duration(0), "0s");
        assert_eq!(format_duration(30), "30s");
        assert_eq!(format_duration(60), "1m 0s");
        assert_eq!(format_duration(90), "1m 30s");
        assert_eq!(format_duration(3600), "1h 0m");
        assert_eq!(format_duration(3661), "1h 1m");
        assert_eq!(format_duration(86400), "1d 0h");
        assert_eq!(format_duration(90061), "1d 1h");
    }

    #[test]
    fn test_format_connection_count() {
        assert_eq!(format_connection_count(0), "No connections");
        assert_eq!(format_connection_count(1), "1 connection");
        assert_eq!(format_connection_count(5), "5 connections");
    }

    #[test]
    fn test_format_protocol() {
        assert_eq!(Formatter::format_protocol("tcp"), "TCP");
        assert_eq!(Formatter::format_protocol("TCP6"), "TCP6");
        assert_eq!(Formatter::format_protocol("udp"), "UDP");
        assert_eq!(Formatter::format_protocol("unknown"), "UNKNOWN");
    }

    #[test]
    fn test_format_state() {
        assert_eq!(Formatter::format_state("ESTABLISHED"), "ESTABLISHED");
        assert_eq!(Formatter::format_state("LISTEN"), "LISTEN");
        assert_eq!(Formatter::format_state("unknown"), "unknown");
    }

    #[test]
    fn test_truncate_string() {
        assert_eq!(Formatter::truncate_string("short", 10), "short");
        assert_eq!(Formatter::truncate_string("exactlyten", 10), "exactlyten");
        let result = Formatter::truncate_string("thisislonger", 10);
        assert_eq!(result, "thisisl...");
        assert_eq!(Formatter::truncate_string("abc", 2), "...");
        assert_eq!(Formatter::truncate_string("abc", 3), "abc");
    }

    #[test]
    fn test_format_pid() {
        assert_eq!(Formatter::format_pid("1234"), "1234");
        assert_eq!(Formatter::format_pid("N/A"), "N/A");
    }

    #[test]
    fn test_format_program() {
        assert_eq!(Formatter::format_program("firefox"), "firefox");
        assert_eq!(Formatter::format_program(""), "Unknown");
        assert_eq!(Formatter::format_program("N/A"), "Unknown");
    }

    #[test]
    fn test_format_bytes_precise() {
        assert_eq!(Formatter::format_bytes_precise(1024, 2), "1.00KB/s");
        assert_eq!(Formatter::format_bytes_precise(1536, 3), "1.500KB/s");
    }
}
