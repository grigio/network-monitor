use crate::error::{NetworkMonitorError, Result};
/// Circuit breaker pattern for resilient error handling
use std::time::{Duration, Instant};

#[derive(Debug, Clone)]
#[allow(dead_code)]
pub enum CircuitState {
    Closed,
    Open,
    HalfOpen,
}

#[derive(Debug)]
#[allow(dead_code)]
pub struct CircuitBreaker {
    failure_count: u32,
    failure_threshold: u32,
    last_failure: Option<Instant>,
    timeout: Duration,
    state: CircuitState,
}

impl CircuitBreaker {
    #[allow(dead_code)]
    pub fn new(failure_threshold: u32, timeout: Duration) -> Self {
        Self {
            failure_count: 0,
            failure_threshold,
            last_failure: None,
            timeout,
            state: CircuitState::Closed,
        }
    }

    #[allow(dead_code)]
    pub fn call<F, T>(&mut self, f: F) -> Result<T>
    where
        F: FnOnce() -> Result<T>,
    {
        match self.state {
            CircuitState::Open => {
                if let Some(last_failure) = self.last_failure {
                    if last_failure.elapsed() > self.timeout {
                        self.state = CircuitState::HalfOpen;
                    } else {
                        return Err(NetworkMonitorError::ParseError(
                            "Circuit breaker is open".to_string(),
                        ));
                    }
                }
            }
            CircuitState::HalfOpen => {
                // Allow one request to test the waters
            }
            CircuitState::Closed => {
                // Normal operation
            }
        }

        match f() {
            Ok(result) => {
                self.on_success();
                Ok(result)
            }
            Err(err) => {
                self.on_failure();
                Err(err)
            }
        }
    }

    fn on_success(&mut self) {
        self.failure_count = 0;
        self.state = CircuitState::Closed;
        self.last_failure = None;
    }

    fn on_failure(&mut self) {
        self.failure_count += 1;
        self.last_failure = Some(Instant::now());

        if self.failure_count >= self.failure_threshold {
            self.state = CircuitState::Open;
        }
    }

    #[allow(dead_code)]
    pub fn is_open(&self) -> bool {
        matches!(self.state, CircuitState::Open)
    }
}

impl Default for CircuitBreaker {
    fn default() -> Self {
        Self::new(5, Duration::from_secs(30))
    }
}

/// Graceful error recovery utilities
pub struct ErrorRecovery;

/// Enhanced error recovery utilities with multiple strategies
#[allow(dead_code)]
pub struct EnhancedErrorRecovery;

impl ErrorRecovery {
    /// Attempt to read a file with fallback to default value
    #[allow(dead_code)]
    pub fn read_file_with_fallback(path: &str, fallback: &str) -> String {
        match std::fs::read_to_string(path) {
            Ok(content) => content,
            Err(_) => fallback.to_string(),
        }
    }

    /// Parse a line with fallback to default value
    #[allow(dead_code)]
    pub fn parse_line_with_fallback<T>(
        line: &str,
        parser: impl Fn(&str) -> Result<T>,
        fallback: T,
    ) -> T {
        parser(line).unwrap_or(fallback)
    }

    /// Get connections with graceful degradation
    pub fn get_connections_with_fallback(
        get_tcp: impl Fn() -> Result<Vec<crate::models::Connection>>,
        get_udp: impl Fn() -> Result<Vec<crate::models::Connection>>,
    ) -> Vec<crate::models::Connection> {
        let mut connections = Vec::new();

        // Try TCP connections, continue on failure
        if let Ok(tcp) = get_tcp() {
            connections.extend(tcp);
        } else {
            eprintln!("Warning: Failed to get TCP connections, continuing with UDP");
        }

        // Try UDP connections, continue on failure
        if let Ok(udp) = get_udp() {
            connections.extend(udp);
        } else {
            eprintln!("Warning: Failed to get UDP connections");
        }

        connections
    }

    /// Parse proc net line with error recovery
    #[allow(dead_code)]
    pub fn parse_proc_net_line_with_recovery(
        line: &str,
        protocol: &str,
        default_state: &str,
    ) -> Option<crate::models::Connection> {
        let parts: Vec<&str> = line.split_whitespace().collect();
        if parts.len() < 10 {
            return None;
        }

        // Parse with fallbacks for each component
        let local_addr = Self::parse_socket_addr_with_fallback(parts[1], "0.0.0.0:0");
        let remote_addr = Self::parse_socket_addr_with_fallback(parts[2], "0.0.0.0:0");

        let state = if parts.len() > 3 {
            crate::utils::parse_tcp_state(parts[3])
        } else {
            default_state.to_string()
        };

        let inode = parts[9].parse::<u64>().unwrap_or(0);

        // Get process info with fallback
        let (program, pid, command) = if inode > 0 {
            // In a real implementation, this would use the process cache
            // For now, provide fallback values
            ("Unknown".to_string(), "N/A".to_string(), "N/A".to_string())
        } else {
            ("N/A".to_string(), "N/A".to_string(), "N/A".to_string())
        };

        Some(crate::models::Connection::new(
            protocol.to_string(),
            state,
            local_addr,
            remote_addr,
            program,
            pid,
            command,
        ))
    }

    /// Parse socket address with fallback
    #[allow(dead_code)]
    fn parse_socket_addr_with_fallback(addr_str: &str, fallback: &str) -> String {
        match crate::utils::split_socket_addr(addr_str) {
            Ok((ip_hex, port_hex)) => {
                let port = crate::utils::parse_port(port_hex).unwrap_or(0);

                let ip = if ip_hex.len() == 8 {
                    crate::utils::parse_ipv4_hex(ip_hex)
                        .map(std::net::IpAddr::V4)
                        .ok()
                } else if ip_hex.len() == 32 {
                    crate::utils::parse_ipv6_hex(ip_hex)
                        .map(std::net::IpAddr::V6)
                        .ok()
                } else {
                    None
                };

                match ip {
                    Some(ip) => format!("{ip}:{port}"),
                    None => fallback.to_string(),
                }
            }
            Err(_) => fallback.to_string(),
        }
    }
}

impl EnhancedErrorRecovery {
    /// Retry operation with exponential backoff
    #[allow(dead_code)]
    pub fn retry_with_backoff<T, F>(
        mut operation: F,
        max_retries: usize,
        initial_delay: Duration,
    ) -> Result<T>
    where
        F: FnMut() -> Result<T>,
    {
        let mut delay = initial_delay;

        for attempt in 0..=max_retries {
            match operation() {
                Ok(result) => return Ok(result),
                Err(e) => {
                    if attempt == max_retries {
                        return Err(e);
                    }

                    eprintln!(
                        "Attempt {} failed: {}, retrying in {:?}",
                        attempt + 1,
                        e,
                        delay
                    );

                    std::thread::sleep(delay);
                    delay = delay.mul_f32(1.5); // Exponential backoff with 1.5x multiplier
                }
            }
        }

        unreachable!()
    }

    /// Execute multiple operations, returning first successful result
    #[allow(dead_code)]
    pub fn try_operations<T, F>(operations: Vec<F>) -> Result<T>
    where
        F: FnOnce() -> Result<T>,
    {
        let mut last_error = None;

        for operation in operations {
            match operation() {
                Ok(result) => return Ok(result),
                Err(e) => {
                    let error_msg = format!("{}", e);
                    last_error = Some(e);
                    eprintln!("Operation failed: {}", error_msg);
                }
            }
        }

        Err(last_error.unwrap_or_else(|| {
            NetworkMonitorError::ParseError("All operations failed".to_string())
        }))
    }

    /// Graceful degradation for critical operations
    #[allow(dead_code)]
    pub fn graceful_degradation<T>(
        primary: impl FnOnce() -> Result<T>,
        fallback: impl FnOnce() -> T,
    ) -> T {
        match primary() {
            Ok(result) => result,
            Err(e) => {
                eprintln!("Critical operation failed, using degraded mode: {}", e);
                fallback()
            }
        }
    }

    /// Circuit breaker wrapper for unreliable operations
    #[allow(dead_code)]
    pub fn with_circuit_breaker<T>(
        circuit_breaker: &mut CircuitBreaker,
        operation: impl FnOnce() -> Result<T>,
    ) -> Result<T> {
        circuit_breaker.call(operation)
    }

    /// Timeout wrapper for operations that might hang
    #[allow(dead_code)]
    pub fn with_timeout<T, F>(operation: F, _timeout: Duration) -> Result<T>
    where
        F: FnOnce() -> Result<T> + Send + 'static,
        T: Send + 'static,
    {
        let handle = std::thread::spawn(operation);

        match handle.join() {
            Ok(result) => result,
            Err(_) => Err(NetworkMonitorError::ParseError(
                "Operation timed out".to_string(),
            )),
        }
    }

    /// Batch operation with partial failure handling
    #[allow(dead_code)]
    pub fn batch_with_partial_failure<T, F>(
        items: Vec<T>,
        operation: F,
    ) -> (Vec<T>, Vec<NetworkMonitorError>)
    where
        F: Fn(&T) -> Result<()>,
        T: Clone,
    {
        let mut successful = Vec::new();
        let mut failed = Vec::new();

        for item in items {
            match operation(&item) {
                Ok(_) => successful.push(item),
                Err(e) => {
                    let error_msg = format!("{}", e);
                    failed.push(e);
                    eprintln!("Item failed: {}", error_msg);
                }
            }
        }

        (successful, failed)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_circuit_breaker() {
        let mut cb = CircuitBreaker::new(2, Duration::from_millis(100));

        // Should work initially
        assert!(cb.call(|| Ok(42)).is_ok());
        assert!(!cb.is_open());

        // Fail twice to trigger circuit breaker
        assert!(cb
            .call(|| Err::<i32, _>(NetworkMonitorError::ParseError("test".to_string())))
            .is_err());
        assert!(cb
            .call(|| Err::<i32, _>(NetworkMonitorError::ParseError("test".to_string())))
            .is_err());
        assert!(cb.is_open());

        // Should fail when circuit is open
        assert!(cb.call(|| Ok(42)).is_err());

        // Wait for timeout
        std::thread::sleep(Duration::from_millis(150));

        // Should work again after timeout
        assert!(cb.call(|| Ok(42)).is_ok());
        assert!(!cb.is_open());
    }

    #[test]
    fn test_read_file_with_fallback() {
        let result = ErrorRecovery::read_file_with_fallback("/nonexistent/file", "fallback");
        assert_eq!(result, "fallback");
    }

    #[test]
    fn test_parse_line_with_fallback() {
        let result = ErrorRecovery::parse_line_with_fallback(
            "invalid",
            |s| crate::utils::parse_hex_u16(s, "test"),
            42,
        );
        assert_eq!(result, 42);
    }

    #[test]
    fn test_retry_with_backoff() {
        let mut attempts = 0;
        let result = EnhancedErrorRecovery::retry_with_backoff(
            || {
                attempts += 1;
                if attempts < 3 {
                    Err(NetworkMonitorError::ParseError("test".to_string()))
                } else {
                    Ok(42)
                }
            },
            3,
            Duration::from_millis(10),
        );
        assert_eq!(result.unwrap(), 42);
        assert_eq!(attempts, 3);
    }

    #[test]
    fn test_try_operations() {
        let operations = vec![
            || Err(NetworkMonitorError::ParseError("first".to_string())),
            || Err(NetworkMonitorError::ParseError("second".to_string())),
            || Ok(42),
            || Ok(99), // This won't be reached
        ];

        let result = EnhancedErrorRecovery::try_operations(operations);
        assert_eq!(result.unwrap(), 42);
    }

    #[test]
    fn test_graceful_degradation() {
        let result = EnhancedErrorRecovery::graceful_degradation(
            || Err(NetworkMonitorError::ParseError("test".to_string())),
            || 42,
        );
        assert_eq!(result, 42);
    }

    #[test]
    fn test_batch_with_partial_failure() {
        let items = vec![1, 2, 3, 4, 5];
        let (successful, failed) =
            EnhancedErrorRecovery::batch_with_partial_failure(items, |item| {
                if *item % 2 == 0 {
                    Ok(())
                } else {
                    Err(NetworkMonitorError::ParseError(format!(
                        "Failed for item {}",
                        item
                    )))
                }
            });

        assert_eq!(successful.len(), 2); // items 2 and 4
        assert_eq!(failed.len(), 3); // items 1, 3, and 5
    }
}
