use anyhow::Result;
use gcodekit_device_adapters::Transport;
use std::net::SocketAddr;

/// High-level device manager that orchestrates adapter connections.
pub struct DeviceManager {}

impl DeviceManager {
    /// Connect to a network device by socket address and return a boxed `Transport`.
    pub fn connect_network(addr: SocketAddr) -> Result<Box<dyn Transport>> {
        // Use the adapter factory to create a transport instance (TCP for now)
        let transport = gcodekit_device_adapters::create_tcp_transport(addr)?;
        Ok(transport)
    }

    /// Connect to a device by endpoint string. Supports:
    /// - tcp://host:port or host:port -> TCP
    /// - serial:///dev/ttyXXX or a path starting with '/' -> serial device
    ///   Returns a boxed `Transport` for the selected adapter.
    pub fn connect_endpoint(endpoint: &str) -> Result<Box<dyn Transport>> {
        // Quick heuristic: ws:// or wss:// indicates websocket, tcp:// or presence of ':' after a hostname indicates TCP
        if endpoint.starts_with("ws://") || endpoint.starts_with("wss://") {
            // The websocket transport factory is only available when the device-adapters
            // crate is compiled with the `websocket` feature. If it's not enabled,
            // return an error with a suggestion.
            #[cfg(feature = "websocket")]
            {
                let transport = gcodekit_device_adapters::create_websocket_transport(endpoint)?;
                return Ok(transport);
            }

            #[cfg(not(feature = "websocket"))]
            {
                return Err(anyhow::anyhow!("websocket transport requested but device-adapters not built with 'websocket' feature"));
            }
        }

        if endpoint.starts_with("tcp://") || (endpoint.contains(":") && !endpoint.starts_with('/'))
        {
            // Strip optional tcp://
            let ep = endpoint.trim_start_matches("tcp://");
            let sock: std::net::SocketAddr = ep.parse()?;
            let transport = gcodekit_device_adapters::create_tcp_transport(sock)?;
            Ok(transport)
        } else {
            // Assume a serial device path. Support optional serial:// prefix and
            // simple query params: baud and timeout_ms, e.g. serial:///dev/ttyUSB0?baud=115200&timeout_ms=500
            let mut baud = 115200u32;
            let mut timeout = std::time::Duration::from_millis(200);
            // We'll optionally own a path string if parsing a URL; otherwise
            // we reuse the borrowed endpoint string.
            let mut owned_path: Option<String> = None;
            if endpoint.starts_with("serial://") || endpoint.contains('?') {
                // Use the url crate to parse query params safely.
                let url = url::Url::parse(endpoint)
                    .or_else(|_| url::Url::parse(&format!("serial://{}", endpoint)))?;
                let path_owned = url.path().to_string();
                let p = path_owned
                    .strip_prefix('/')
                    .unwrap_or(&path_owned)
                    .to_string();
                owned_path = Some(p);
                for (k, v) in url.query_pairs() {
                    match k.as_ref() {
                        "baud" => {
                            if let Ok(b) = v.parse::<u32>() {
                                baud = b
                            }
                        }
                        "timeout_ms" => {
                            if let Ok(ms) = v.parse::<u64>() {
                                timeout = std::time::Duration::from_millis(ms)
                            }
                        }
                        _ => {}
                    }
                }
            } else if endpoint.starts_with('/') {
                owned_path = Some(endpoint.to_string());
            }

            let path_str = owned_path.as_deref().unwrap_or(endpoint);

            let opts = gcodekit_device_adapters::SerialOptions {
                baud,
                timeout,
                parity: None,
                flow_control: None,
            };
            let transport =
                gcodekit_device_adapters::create_serial_transport_with_options(path_str, opts)?;
            Ok(transport)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::net::TcpListener;

    #[test]
    fn test_connect_network_to_local_listener() {
        let listener = TcpListener::bind("127.0.0.1:0").expect("bind");
        let addr = listener.local_addr().unwrap();

        // Spawn accept thread
        std::thread::spawn(move || {
            if let Ok((_s, _p)) = listener.accept() {
                std::thread::sleep(std::time::Duration::from_millis(50));
            }
        });

        let endpoint = format!("127.0.0.1:{}", addr.port());
        let mut conn = DeviceManager::connect_endpoint(&endpoint).expect("connect network");
        // Basic smoke: send a line
        let _ = conn.send_line("M115");
    }

    #[test]
    #[ignore]
    fn test_connect_serial_endpoint_ignored() {
        // This test is ignored by default; it demonstrates the serial endpoint
        // parsing path. Replace with a real device path to run locally.
        let path = "/dev/ttyUSB0";
        let _ = DeviceManager::connect_endpoint(path);
    }
}
