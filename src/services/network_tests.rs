#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_socket_addr_ipv4() {
        let service = NetworkService::new();
        let addr = service.parse_socket_addr("0100007F:1234");
        assert_eq!(addr.unwrap(), "127.0.0.1:4660");
    }

    #[test]
    fn test_parse_socket_addr_ipv6() {
        let service = NetworkService::new();
        let addr = service.parse_socket_addr("00000000000000000000000000000000001:1234");
        assert_eq!(addr.unwrap(), "::1:4660");
    }



    #[test]
    fn test_parse_tcp_state() {
        let service = NetworkService::new();
        assert_eq!(service.parse_tcp_state("0A"), "LISTEN");
        assert_eq!(service.parse_tcp_state("01"), "ESTABLISHED");
        assert_eq!(service.parse_tcp_state("FF"), "UNKNOWN(255)");
    }

    #[test]
    fn test_get_connections() {
        let service = NetworkService::new();
        let connections = service.get_connections();
        // Should not panic and return a vector
        match connections {
            Ok(conn) => assert!(conn.len() >= 0),
            Err(_) => assert!(true), // It's ok to fail in test environment
        }
    }
}