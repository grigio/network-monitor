/// Utility for formatting byte values
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
}
