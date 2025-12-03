/// Helper utilities for common parsing operations
use crate::error::{NetworkMonitorError, Result};

/// Parse a hexadecimal string to u8 with proper error context
pub fn parse_hex_u8(input: &str, context: &str) -> Result<u8> {
    u8::from_str_radix(input, 16).map_err(|e| {
        NetworkMonitorError::HexParseError(format!(
            "{}: Failed to parse hex '{}': {}",
            context, input, e
        ))
    })
}

/// Parse a hexadecimal string to u16 with proper error context
pub fn parse_hex_u16(input: &str, context: &str) -> Result<u16> {
    u16::from_str_radix(input, 16).map_err(|e| {
        NetworkMonitorError::HexParseError(format!(
            "{}: Failed to parse hex '{}': {}",
            context, input, e
        ))
    })
}

/// Parse a hexadecimal string to u64 with proper error context
#[allow(dead_code)]
pub fn parse_hex_u64(input: &str, context: &str) -> Result<u64> {
    u64::from_str_radix(input, 16).map_err(|e| {
        NetworkMonitorError::HexParseError(format!(
            "{}: Failed to parse hex '{}': {}",
            context, input, e
        ))
    })
}

/// Parse a decimal string with proper error context
pub fn parse_decimal<T>(input: &str, context: &str) -> Result<T>
where
    T: std::str::FromStr,
    T::Err: std::fmt::Display,
{
    input.parse::<T>().map_err(|e| {
        NetworkMonitorError::ParseError(format!("{}: Failed to parse '{}': {}", context, input, e))
    })
}

/// Parse a port from hexadecimal string
pub fn parse_port(port_hex: &str) -> Result<u16> {
    parse_hex_u16(port_hex, "port")
}

/// Parse an inode from string
#[allow(dead_code)]
pub fn parse_inode(inode_str: &str) -> Result<u64> {
    parse_decimal(inode_str, "inode")
}

/// Parse IPv4 address from hex string (little-endian format)
pub fn parse_ipv4_hex(ip_hex: &str) -> Result<std::net::Ipv4Addr> {
    if ip_hex.len() != 8 {
        return Err(NetworkMonitorError::InvalidAddress(format!(
            "Invalid IPv4 hex length: {} (expected 8)",
            ip_hex.len()
        )));
    }

    let mut bytes = [0u8; 4];
    for (i, chunk) in (0..ip_hex.len()).step_by(2).enumerate() {
        bytes[3 - i] = parse_hex_u8(&ip_hex[chunk..chunk + 2], "IPv4 byte")?;
    }
    Ok(std::net::Ipv4Addr::from(bytes))
}

/// Parse IPv6 address from hex string
pub fn parse_ipv6_hex(ip_hex: &str) -> Result<std::net::Ipv6Addr> {
    if ip_hex.len() != 32 {
        return Err(NetworkMonitorError::InvalidAddress(format!(
            "Invalid IPv6 hex length: {} (expected 32)",
            ip_hex.len()
        )));
    }

    let mut bytes = [0u8; 16];
    for (i, chunk) in (0..ip_hex.len()).step_by(2).enumerate() {
        bytes[i] = parse_hex_u8(&ip_hex[chunk..chunk + 2], "IPv6 byte")?;
    }
    Ok(std::net::Ipv6Addr::from(bytes))
}

/// Parse TCP state from hex value
pub fn parse_tcp_state(state_hex: &str) -> String {
    if let Ok(state_val) = parse_hex_u8(state_hex, "TCP state") {
        match state_val {
            0x01 => "ESTABLISHED".to_string(),
            0x02 => "SYN_SENT".to_string(),
            0x03 => "SYN_RECV".to_string(),
            0x04 => "FIN_WAIT1".to_string(),
            0x05 => "FIN_WAIT2".to_string(),
            0x06 => "TIME_WAIT".to_string(),
            0x07 => "CLOSE".to_string(),
            0x08 => "CLOSE_WAIT".to_string(),
            0x09 => "LAST_ACK".to_string(),
            0x0A => "LISTEN".to_string(),
            0x0B => "CLOSING".to_string(),
            0x0C => "NEW_SYN_RECV".to_string(),
            _ => format!("UNKNOWN({})", state_val),
        }
    } else {
        "UNKNOWN".to_string()
    }
}

/// Validate that a string contains only digits (for PID validation)
#[allow(dead_code)]
pub fn validate_pid(pid_str: &str) -> Result<()> {
    if pid_str.chars().all(|c| c.is_ascii_digit()) {
        Ok(())
    } else {
        Err(NetworkMonitorError::InvalidPid(format!(
            "PID contains non-digit characters: {}",
            pid_str
        )))
    }
}

/// Split socket address into IP and port components
pub fn split_socket_addr(addr_str: &str) -> Result<(&str, &str)> {
    let parts: Vec<&str> = addr_str.split(':').collect();
    if parts.len() != 2 {
        return Err(NetworkMonitorError::InvalidAddress(format!(
            "Invalid socket address format: {}",
            addr_str
        )));
    }
    Ok((parts[0], parts[1]))
}

/// Normalize common address patterns for better readability
#[allow(dead_code)]
pub fn normalize_address(addr: &str) -> std::borrow::Cow<'static, str> {
    match addr {
        "0.0.0.0:*" | "*:*" => std::borrow::Cow::Borrowed("ANY"),
        "127.0.0.1:*" | "[::1]:*" => std::borrow::Cow::Borrowed("LOCALHOST"),
        _ => std::borrow::Cow::Owned(addr.to_string()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_hex() {
        assert_eq!(parse_hex_u16("1234", "test").unwrap(), 0x1234);
        assert_eq!(parse_hex_u8("FF", "test").unwrap(), 255);
        assert!(parse_hex_u16("ZZ", "test").is_err());
    }

    #[test]
    fn test_parse_decimal() {
        assert_eq!(parse_decimal::<u32>("1234", "test").unwrap(), 1234);
        assert!(parse_decimal::<u32>("abc", "test").is_err());
    }

    #[test]
    fn test_parse_port() {
        assert_eq!(parse_port("1234").unwrap(), 0x1234);
        assert!(parse_port("ZZZZ").is_err());
    }

    #[test]
    fn test_parse_ipv4_hex() {
        let ip = parse_ipv4_hex("0100007F").unwrap(); // 127.0.0.1 in little-endian
        assert_eq!(ip.to_string(), "127.0.0.1");
        assert!(parse_ipv4_hex("123").is_err()); // Wrong length
    }

    #[test]
    fn test_parse_ipv6_hex() {
        let ip = parse_ipv6_hex("00000000000000000000000001000000").unwrap(); // ::100:0
        assert_eq!(ip.to_string(), "::100:0");
        assert!(parse_ipv6_hex("123").is_err()); // Wrong length
    }

    #[test]
    fn test_parse_tcp_state() {
        assert_eq!(parse_tcp_state("01"), "ESTABLISHED");
        assert_eq!(parse_tcp_state("0A"), "LISTEN");
        assert_eq!(parse_tcp_state("FF"), "UNKNOWN(255)");
        assert_eq!(parse_tcp_state("ZZ"), "UNKNOWN");
    }

    #[test]
    fn test_validate_pid() {
        assert!(validate_pid("1234").is_ok());
        assert!(validate_pid("0").is_ok());
        assert!(validate_pid("abc").is_err());
        assert!(validate_pid("12a4").is_err());
    }

    #[test]
    fn test_split_socket_addr() {
        let (ip, port) = split_socket_addr("0100007F:1234").unwrap();
        assert_eq!(ip, "0100007F");
        assert_eq!(port, "1234");
        assert!(split_socket_addr("invalid").is_err());
    }

    #[test]
    fn test_normalize_address() {
        assert_eq!(normalize_address("0.0.0.0:*"), "ANY");
        assert_eq!(normalize_address("127.0.0.1:*"), "LOCALHOST");
        assert_eq!(normalize_address("192.168.1.1:8080"), "192.168.1.1:8080");
    }
}
