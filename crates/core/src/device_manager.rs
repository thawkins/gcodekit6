use anyhow::Result;
use gcodekit_device_adapters::Transport;
use std::net::SocketAddr;
use tracing::{debug, info};

/// High-level device manager that orchestrates adapter connections.
pub struct DeviceManager {}

/// Rich device information returned to UIs and callers that need metadata.
#[derive(Debug, Clone)]
pub struct DiscoveredDevice {
    pub id: String,
    pub display: String,
    pub transport: String,
    pub product: Option<String>,
    pub manufacturer: Option<String>,
    pub vid: Option<u16>,
    pub pid: Option<u16>,
}

impl DeviceManager {
    /// Discover available devices on the system. Returns tuples of (id, display_name, transport_hint)
    /// For now this includes serial ports and a placeholder for network discovery.
    pub fn discover_devices() -> Result<Vec<(String, String, String)>> {
        let mut devices = Vec::new();
        // Pre-load any persisted devices so discovery can include known devices
        if let Ok(persisted) = crate::device::load_devices() {
            for d in persisted.iter() {
                devices.push((d.id.clone(), d.name.clone(), format!("{:?}", d.transport)));
            }
        }
        // Serial ports (structured info)
        if let Ok(ports) = gcodekit_device_adapters::serial::list_serial_ports() {
            for p in ports {
                let display = if let Some(prod) = &p.product {
                    format!("{} ({})", p.path, prod)
                } else if let Some(mfr) = &p.manufacturer {
                    format!("{} ({})", p.path, mfr)
                } else if let (Some(vid), Some(pid)) = (p.vid, p.pid) {
                    format!("{} ({:04x}:{:04x})", p.path, vid, pid)
                } else {
                    p.path.clone()
                };
                let id = format!("serial:{}", p.path);
                devices.push((id.clone(), display, "serial".to_string()));
            }
        }

        // Network discovery placeholder: return empty for now
        Ok(devices)
    }

    /// Discover devices and return rich metadata where available.
    pub fn discover_devices_detailed() -> Result<Vec<DiscoveredDevice>> {
        let mut out = Vec::new();
        // Load persisted devices first
        if let Ok(persisted) = crate::device::load_devices() {
            for d in persisted.into_iter() {
                out.push(DiscoveredDevice {
                    id: d.id.clone(),
                    display: d.name.clone(),
                    transport: format!("{:?}", d.transport),
                    product: None,
                    manufacturer: None,
                    vid: None,
                    pid: None,
                });
            }
        }

        // Structured serial ports
        if let Ok(ports) = gcodekit_device_adapters::serial::list_serial_ports() {
            for p in ports {
                let display = if let Some(prod) = &p.product {
                    format!("{} ({})", p.path, prod)
                } else if let Some(mfr) = &p.manufacturer {
                    format!("{} ({})", p.path, mfr)
                } else if let (Some(vid), Some(pid)) = (p.vid, p.pid) {
                    format!("{} ({:04x}:{:04x})", p.path, vid, pid)
                } else {
                    p.path.clone()
                };
                out.push(DiscoveredDevice {
                    id: format!("serial:{}", p.path),
                    display,
                    transport: "serial".to_string(),
                    product: p.product.clone(),
                    manufacturer: p.manufacturer.clone(),
                    vid: p.vid,
                    pid: p.pid,
                });
            }
        }

        Ok(out)
    }

    /// Attempt to discover a TCP peer at the provided address string using a short timeout.
    /// Returns the peer string on success.
    pub fn discover_tcp_peer(addr: &str, timeout: std::time::Duration) -> Result<String> {
        let res = gcodekit_device_adapters::network::discover_tcp_peer(addr, timeout)?;
        Ok(res)
    }

    /// Connect to a network device by socket address and return a boxed `Transport`.
    pub fn connect_network(addr: SocketAddr) -> Result<Box<dyn Transport>> {
        // Use the adapter factory to create a transport instance (TCP for now)
        info!(addr = %addr, "device_manager::connect_network: attempting");
        let transport = gcodekit_device_adapters::create_tcp_transport(addr)?;
        debug!(addr = %addr, "device_manager::connect_network: transport created");
        // Persist a simple Device record for this network connection
        let dev = crate::models::Device {
            id: format!("tcp:{}", addr),
            name: format!("TCP Device {}", addr),
            port: addr.to_string(),
            baud: None,
            firmware: None,
            capabilities: vec![],
            status: crate::models::DeviceStatus::Connected,
            transport: crate::models::Transport::Tcp,
        };
        // Save best-effort; ignore errors so connect still returns success
        let _ = crate::device::save_devices(vec![dev]);
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
                info!(endpoint = %endpoint, "device_manager::connect_endpoint: websocket requested");
                let transport = gcodekit_device_adapters::create_websocket_transport(endpoint)?;
                debug!(endpoint = %endpoint, "device_manager::connect_endpoint: websocket transport created");
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
            info!(endpoint = %ep, sock = %sock, "device_manager::connect_endpoint: tcp requested");
            let transport = gcodekit_device_adapters::create_tcp_transport(sock)?;
            debug!(endpoint = %ep, sock = %sock, "device_manager::connect_endpoint: tcp transport created");
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
            debug!(path = %path_str, baud = %opts.baud, "device_manager::connect_endpoint: serial requested");
            let transport = gcodekit_device_adapters::create_serial_transport_with_options(path_str, opts)?;
            debug!(path = %path_str, "device_manager::connect_endpoint: serial transport created");
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
