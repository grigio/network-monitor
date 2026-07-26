use std::collections::{HashMap, HashSet};
use std::sync::{Arc, Mutex};
use std::thread;

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

    pub fn resolve_address(&self, addr: &str) -> String {
        if addr == "0.0.0.0:*" || addr == "*:*" || addr == "[::]:*" {
            return "ANY".to_string();
        } else if addr.starts_with("127.0.0.1:") || addr.starts_with("[::1]:") {
            return "LOCALHOST".to_string();
        } else if addr.starts_with("224.0.0.251:") {
            return "MDNS".to_string();
        }

        let resolve_hosts = match self.resolve_hosts.lock() {
            Ok(guard) => *guard,
            Err(_) => return addr.to_string(),
        };
        if !resolve_hosts {
            return addr.to_string();
        }

        {
            let cache = match self.cache.lock() {
                Ok(guard) => guard,
                Err(_) => return addr.to_string(),
            };
            if let Some(resolved) = cache.get(addr) {
                return resolved.clone();
            }
        }

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
            (addr.to_string(), String::new())
        };

        {
            let mut pending = match self.pending.lock() {
                Ok(guard) => guard,
                Err(_) => return addr.to_string(),
            };
            if !pending.contains(&ip_part) {
                pending.insert(ip_part.clone());

                let addr = addr.to_string();
                let cache = self.cache.clone();
                let pending = self.pending.clone();

                thread::spawn(move || {
                    let resolved = match std::process::Command::new("timeout")
                        .args(["5s", "host", &ip_part])
                        .output()
                    {
                        Ok(output) => {
                            let output_str = String::from_utf8_lossy(&output.stdout);
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

                    if let Ok(mut cache) = cache.lock() {
                        cache.insert(addr.clone(), resolved);
                    }

                    if let Ok(mut pending) = pending.lock() {
                        pending.remove(&ip_part);
                    }
                });
            }
        }

        addr.to_string()
    }

    pub fn set_resolve_hosts(&self, resolve: bool) {
        if let Ok(mut resolve_hosts) = self.resolve_hosts.lock() {
            *resolve_hosts = resolve;
            if !resolve {
                if let Ok(mut cache) = self.cache.lock() {
                    cache.clear();
                }
            }
        }
    }

    #[allow(dead_code)]
    pub fn get_resolve_hosts(&self) -> bool {
        self.resolve_hosts
            .lock()
            .map(|guard| *guard)
            .unwrap_or(false)
    }

    #[allow(dead_code)]
    pub fn clear_cache(&self) {
        if let Ok(mut cache) = self.cache.lock() {
            cache.clear();
        }
    }
}
