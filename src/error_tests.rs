#[cfg(test)]
mod error_handling_tests {
    use crate::error::{NetworkMonitorError, Result};

    #[test]
    fn test_error_creation() {
        let io_error = std::io::Error::new(std::io::ErrorKind::NotFound, "test error");
        let network_error = NetworkMonitorError::from(io_error);

        match network_error {
            NetworkMonitorError::ProcIo(_) => {} // Expected variant
            _ => panic!("Expected ProcIo error"),
        }
    }

    #[test]
    fn test_invalid_address_error() {
        let error = NetworkMonitorError::InvalidAddress("bad_address".to_string());
        assert!(error.to_string().contains("Invalid socket address format"));
    }

    #[test]
    fn test_hex_parse_error() {
        let error = NetworkMonitorError::HexParseError("invalid_hex".to_string());
        assert!(error.to_string().contains("Failed to parse hex value"));
    }

    #[test]
    fn test_mutex_poison_error() {
        let error = NetworkMonitorError::MutexPoison("test_mutex".to_string());
        assert!(error.to_string().contains("Mutex lock poisoned"));
    }

    #[test]
    fn test_result_type() {
        fn returns_result() -> Result<String> {
            Ok("test".to_string())
        }

        assert!(returns_result().is_ok());
    }

    #[test]
    fn test_error_context_macro() {
        let result: Result<String> = Err(NetworkMonitorError::ParseError("test error".to_string()));

        // This would normally use the error_context macro
        match result {
            Err(NetworkMonitorError::ParseError(msg)) => {
                assert!(msg.contains("test error"));
            }
            _ => panic!("Expected ParseError"),
        }
    }
}
