use std::collections::{HashMap, HashSet};
use std::sync::{Arc, Mutex};
use std::thread;

/// Service for resolving IP addresses to hostnames
#[derive(Clone)]
pub struct AddressResolver {
    cache: Arc<Mutex<HashMap<String, String>>>,
    pending: Arc<Mutex<HashSet<String>>>,
    resolve_hosts: Arc<Mutex<bool>>,
}

impl AddressResolver {
    pub fn new(resolve_hosts: bool) -> Self {
        Self {
            cache: Arc::new(Mutex::new(HashMap::new())),
            pending: Arc::new(Mutex::new(HashSet::new())),
            resolve_hosts: Arc::new(Mutex::new(resolve_hosts)),
        }
    }

    /// Resolve an address to hostname if resolution is enabled
    pub fn resolve_address(&self, addr: &str) -> String {
        // Handle special cases
        if addr == "0.0.0.0:*" || addr == "*:*" || addr == "[::]:*" {
            return "ANY".to_string();
        } else if addr.starts_with("127.0.0.1:") || addr.starts_with("[::1]:") {
            return "LOCALHOST".to_string();
        } else if addr.starts_with("224.0.0.251:") {
            return "MDNS".to_string();
        }

        // Check if resolution is disabled
        let resolve_hosts = *self.resolve_hosts.lock().unwrap();
        if !resolve_hosts {
            return addr.to_string();
        }

        // Check cache first
        {
            let cache = self.cache.lock().unwrap();
            if let Some(resolved) = cache.get(addr) {
                return resolved.clone();
            }
        }

        // Extract IP address and port
        let (ip_part, port) = if let Some(last_colon) = addr.rfind(':') {
            let ip_with_brackets = &addr[..last_colon];
            let port = &addr[last_colon + 1..];

            let ip_part = if ip_with_brackets.starts_with('[') && ip_with_brackets.ends_with(']') {
                &ip_with_brackets[1..ip_with_brackets.len() - 1]
            } else {
                ip_with_brackets
            };

            (ip_part.to_string(), port.to_string())
        } else {
            (addr.to_string(), "".to_string())
        };

        // Start async resolution if not already pending
        {
            let mut pending = self.pending.lock().unwrap();
            if !pending.contains(&ip_part) {
                pending.insert(ip_part.clone());

                let addr = addr.to_string();
                let cache = self.cache.clone();
                let pending = self.pending.clone();

                thread::spawn(move || {
                    // Simple hostname resolution using host command
                    let resolved = match std::process::Command::new("host").arg(&ip_part).output() {
                        Ok(output) => {
                            let output_str = String::from_utf8_lossy(&output.stdout);
                            // Simple parsing for hostname
                            let mut result = addr.clone();
                            for line in output_str.lines() {
                                if line.contains("domain name pointer")
                                    || line.contains("is an alias for")
                                {
                                    let parts: Vec<&str> = line.split_whitespace().collect();
                                    for (i, part) in parts.iter().enumerate() {
                                        if (*part == "pointer" || *part == "alias")
                                            && i + 1 < parts.len()
                                        {
                                            let hostname = parts[i + 1].trim_end_matches('.');
                                            if port.is_empty() {
                                                result = hostname.to_string();
                                            } else {
                                                result = format!("{hostname}:{port}");
                                            }
                                            break;
                                        }
                                    }
                                }
                            }
                            result
                        }
                        Err(_) => addr.clone(),
                    };

                    // Update cache
                    {
                        let mut cache = cache.lock().unwrap();
                        cache.insert(addr.clone(), resolved);
                    }

                    // Remove from pending
                    {
                        let mut pending = pending.lock().unwrap();
                        pending.remove(&ip_part);
                    }
                });
            }
        }

        addr.to_string()
    }

    /// Set whether to resolve hostnames
    pub fn set_resolve_hosts(&self, resolve: bool) {
        *self.resolve_hosts.lock().unwrap() = resolve;
        if !resolve {
            self.cache.lock().unwrap().clear();
        }
    }

    /// Get current resolve hosts setting
    #[allow(dead_code)]
    pub fn get_resolve_hosts(&self) -> bool {
        *self.resolve_hosts.lock().unwrap()
    }

    /// Clear the resolution cache
    #[allow(dead_code)]
    pub fn clear_cache(&self) {
        self.cache.lock().unwrap().clear();
    }
}
