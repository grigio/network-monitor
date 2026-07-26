#[cfg(test)]
mod error_handling_tests {
    use crate::error::{NetworkMonitorError, Result};

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

        match result {
            Err(NetworkMonitorError::ParseError(msg)) => {
                assert!(msg.contains("test error"));
            }
            _ => panic!("Expected ParseError"),
        }
    }
}
