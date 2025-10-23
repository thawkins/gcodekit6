use gcodekit_core::async_streamer::AsyncStreamer;
#[cfg(not(feature = "async"))]
use gcodekit_core::device_manager::DeviceManager;
use std::io::{Read, Write};
use std::net::TcpListener;

#[tokio::test]
async fn test_async_streamer_basic() {
    let listener = TcpListener::bind("127.0.0.1:0").expect("bind");
    let addr = listener.local_addr().unwrap();

    // Spawn a thread to accept and ack
    std::thread::spawn(move || {
        if let Ok((mut s, _)) = listener.accept() {
            let mut buf = [0u8; 512];
            loop {
                match s.read(&mut buf) {
                    Ok(n) if n > 0 => {
                        let _ = s.write_all(b"ok\n");
                    }
                    _ => break,
                }
            }
        }
    });

    let endpoint = format!("127.0.0.1:{}", addr.port());
    #[cfg(feature = "async")]
    let transport = {
        // Use the async factory if compiled with device-adapters async feature
        let addr = endpoint.parse().unwrap();
        // create async transport via device-adapters (await directly in the tokio test)
        gcodekit_device_adapters::async_network::AsyncTcpTransport::connect(addr)
            .await
            .expect("connect")
    };

    #[cfg(not(feature = "async"))]
    let transport = DeviceManager::connect_endpoint(&endpoint).expect("connect");

    #[cfg(feature = "async")]
    let streamer = AsyncStreamer::new_async(Box::new(transport), 4);

    #[cfg(not(feature = "async"))]
    let streamer = AsyncStreamer::new(transport, 4);

    let lines = vec!["G0 X0", "G0 X1", "G0 X2"];
    let res = streamer.stream(lines).await;
    assert!(res.is_ok());
}
