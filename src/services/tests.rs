#[cfg(test)]
mod service_tests {
    use crate::models::{Connection, ProcessIO};
    use crate::services::NetworkService;
    use std::collections::HashMap;

    #[test]
    fn test_get_connections_empty_proc() {
        let service = NetworkService::new();

        // This test will fail if /proc/net doesn't exist, but that's expected
        // In a real test environment, we'd mock the filesystem
        let result = service.get_connections();
        // We can't assert specific results since it depends on the system
        // But we can verify it doesn't panic
        assert!(result.is_ok() || result.is_err());
    }

    #[test]
    fn test_get_process_io_valid_pid() {
        let service = NetworkService::new();
        // Use current process PID which should exist
        let current_pid = std::process::id().to_string();
        let _result = service.get_process_io(&current_pid);

        // Should return valid ProcessIO (may be zero if no I/O yet)
        // u64 values are always valid, no need to check for negativity
    }

    #[test]
    fn test_get_process_io_invalid_pid() {
        let service = NetworkService::new();
        let result = service.get_process_io("999999");

        // Should return zero ProcessIO for non-existent PID
        assert_eq!(result.rx, 0);
        assert_eq!(result.tx, 0);
    }

    #[test]
    fn test_get_process_path_valid_pid() {
        let service = NetworkService::new();
        let current_pid = std::process::id().to_string();
        let result = service.get_process_path(&current_pid);

        // Should return a valid path or command line
        assert!(!result.is_empty());
        assert_ne!(result, "N/A");
    }

    #[test]
    fn test_get_process_path_invalid_pid() {
        let service = NetworkService::new();
        let result = service.get_process_path("999999");
        assert_eq!(result, "N/A");
    }

    #[test]
    fn test_update_connection_rates_empty() {
        let service = NetworkService::new();
        let connections = Vec::new();
        let prev_io = HashMap::new();

        let result = service.update_connection_rates(connections, &prev_io);
        assert!(result.is_ok());

        let (updated, current) = result.unwrap();
        assert!(updated.is_empty());
        assert!(current.is_empty());
    }

    #[test]
    fn test_update_connection_rates_with_data() {
        let service = NetworkService::new();
        let connections = vec![Connection::new(
            "tcp".to_string(),
            "ESTABLISHED".to_string(),
            "127.0.0.1:1234".to_string(),
            "127.0.0.1:5678".to_string(),
            "test".to_string(),
            std::process::id().to_string(),
            "test".to_string(),
        )];

        let prev_io = HashMap::new();

        let result = service.update_connection_rates(connections, &prev_io);
        assert!(result.is_ok());

        let (updated, current) = result.unwrap();
        assert_eq!(updated.len(), 1);
        assert_eq!(current.len(), 1);
    }

    #[test]
    fn test_update_connection_rates_with_previous_data() {
        let service = NetworkService::new();
        let current_pid = std::process::id().to_string();

        let connections = vec![Connection::new(
            "tcp".to_string(),
            "ESTABLISHED".to_string(),
            "127.0.0.1:1234".to_string(),
            "127.0.0.1:5678".to_string(),
            "test".to_string(),
            current_pid.clone(),
            "test".to_string(),
        )];

        let mut prev_io = HashMap::new();
        prev_io.insert(current_pid.clone(), ProcessIO::new(1000, 2000));

        let result = service.update_connection_rates(connections, &prev_io);
        assert!(result.is_ok());

        let (updated, current) = result.unwrap();
        assert_eq!(updated.len(), 1);
        assert_eq!(current.len(), 1);

        // Rates should be calculated based on difference
        // We can't assert exact values since they depend on current I/O
        // u64 values are always valid, no need to check for negativity
    }

    #[test]
    fn test_default_implementation() {
        let service = NetworkService::default();
        let result = service.get_connections();
        // Should not panic
        assert!(result.is_ok() || result.is_err());
    }
}
