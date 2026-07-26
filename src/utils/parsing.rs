/// Helper utilities for common parsing operations
use crate::error::{NetworkMonitorError, Result};

/// Parse a hexadecimal string to u8 with proper error context
#[allow(dead_code)]
pub fn parse_hex_u8(input: &str, context: &str) -> Result<u8> {
    u8::from_str_radix(input, 16).map_err(|e| {
        NetworkMonitorError::ParseError(format!(
            "{}: Failed to parse hex '{}': {}",
            context, input, e
        ))
    })
}

/// Parse a hexadecimal string to u16 with proper error context
#[allow(dead_code)]
pub fn parse_hex_u16(input: &str, context: &str) -> Result<u16> {
    u16::from_str_radix(input, 16).map_err(|e| {
        NetworkMonitorError::ParseError(format!(
            "{}: Failed to parse hex '{}': {}",
            context, input, e
        ))
    })
}

/// Parse a decimal string with proper error context
#[allow(dead_code)]
pub fn parse_decimal<T>(input: &str, context: &str) -> Result<T>
where
    T: std::str::FromStr,
    T::Err: std::fmt::Display,
{
    input.parse::<T>().map_err(|e| {
        NetworkMonitorError::ParseError(format!("{}: Failed to parse '{}': {}", context, input, e))
    })
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
}
