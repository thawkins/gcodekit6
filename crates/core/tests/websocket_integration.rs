#[cfg(feature = "websocket")]
use gcodekit_core::device_manager::DeviceManager;
#[cfg(feature = "websocket")]
use gcodekit_core::streamer::Streamer;

#[cfg(feature = "websocket")]
#[test]
fn test_streamer_over_websocket() {
    // Start a local TCP listener and tungstenite acceptor to echo messages and reply 'ok'
    let listener = std::net::TcpListener::bind("127.0.0.1:0").expect("bind");
    let addr = listener.local_addr().unwrap();

    let server = std::thread::spawn(move || {
        let (stream, _) = listener.accept().expect("accept");
        let mut ws = tungstenite::accept(stream).expect("accept ws");
        // Read messages in a loop and reply with "ok" for each one.
        loop {
            match ws.read() {
                Ok(_msg) => {
                    let _ = ws.send(tungstenite::Message::Text("ok".to_string()));
                }
                Err(_) => break,
            }
        }
    });

    let url = format!("ws://{}", addr);
    // DeviceManager::connect_endpoint supports ws:// endpoints and will call into device-adapters
    let transport = DeviceManager::connect_endpoint(&url).expect("connect websocket");

    // Create a simple streamer with small window
    let streamer = Streamer::new(transport, 2);
    let lines = vec!["G0 X0 Y0", "G1 X10 Y10"];
    let res = streamer.stream(lines);
    assert!(res.is_ok());

    let _ = server.join();
}
