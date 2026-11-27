//! Tests for WebSocket connections through `SerialCommunicator` (FluidNC / grblHAL boards).
//!
//! Each test runs a fake controller: a WebSocket server on 127.0.0.1 with an ephemeral
//! port, in a thread of the test. No external network is used.

use std::net::{TcpListener, TcpStream};
use std::sync::mpsc;
use std::time::{Duration, Instant};

use gcodekit5_communication::communication::websocket_url;
use gcodekit5_communication::{Communicator, ConnectionParams, SerialCommunicator};
use tungstenite::{Message, WebSocket};

/// Reads until data arrives or `timeout` elapses.
fn receive_within(comm: &mut SerialCommunicator, timeout: Duration) -> Vec<u8> {
    let deadline = Instant::now() + timeout;
    loop {
        let data = comm
            .receive()
            .expect("receive should not fail while connected");
        if !data.is_empty() || Instant::now() >= deadline {
            return data;
        }
    }
}

/// Binds a listener on an ephemeral local port and returns it with its port.
fn listener() -> (TcpListener, u16) {
    let listener = TcpListener::bind("127.0.0.1:0").expect("bind local listener");
    let port = listener.local_addr().expect("listener address").port();
    (listener, port)
}

/// Accepts one connection and completes the WebSocket handshake.
fn accept(listener: &TcpListener) -> WebSocket<TcpStream> {
    let (stream, _) = listener.accept().expect("accept connection");
    tungstenite::accept(stream).expect("websocket handshake")
}

/// Whether the next data message the server reads is text, and its bytes.
fn next_data(ws: &mut WebSocket<TcpStream>) -> (bool, Vec<u8>) {
    loop {
        match ws.read().expect("read from client") {
            Message::Text(text) => return (true, text.as_bytes().to_vec()),
            Message::Binary(data) => return (false, data.to_vec()),
            Message::Ping(_) | Message::Pong(_) | Message::Frame(_) => {}
            Message::Close(_) => panic!("client closed early"),
        }
    }
}

#[test]
fn test_websocket_url_brackets_ipv6() {
    assert_eq!(websocket_url("fluidnc.local", 81), "ws://fluidnc.local:81/");
    assert_eq!(websocket_url(" 10.0.0.5 ", 81), "ws://10.0.0.5:81/");
    assert_eq!(websocket_url("::1", 81), "ws://[::1]:81/");
}

#[test]
fn test_websocket_transport_sends_and_receives() {
    let (listener, port) = listener();
    let (done_tx, done_rx) = mpsc::channel::<()>();
    let server = std::thread::spawn(move || {
        let mut ws = accept(&listener);
        // A status query: one byte, its own message, answered like GRBL.
        assert_eq!(next_data(&mut ws), (true, b"?".to_vec()));
        ws.send(Message::text("<Idle|MPos:0.000,0.000,0.000>\r\n"))
            .expect("reply to client");
        // A G-code line keeps its framing.
        assert_eq!(next_data(&mut ws), (true, b"G0 X1\n".to_vec()));
        // A reply split over two messages, as a controller may send it.
        ws.send(Message::text("o")).expect("reply part 1");
        ws.send(Message::binary(b"k\r\n".to_vec()))
            .expect("reply part 2");
        // An extended real-time byte is not UTF-8: sent as binary, unchanged.
        assert_eq!(next_data(&mut ws), (false, vec![0x85]));
        let _ = done_rx.recv_timeout(Duration::from_secs(10));
    });

    let mut comm = SerialCommunicator::new();
    comm.connect(&ConnectionParams::websocket("127.0.0.1", port))
        .expect("WebSocket connect");
    assert!(comm.is_connected());
    comm.send(b"?").expect("send status query");
    let reply = receive_within(&mut comm, Duration::from_secs(2));
    assert!(
        String::from_utf8_lossy(&reply).starts_with("<Idle"),
        "{reply:?}"
    );
    comm.send(b"G0 X1\n").expect("send line");
    let mut ack = Vec::new();
    let deadline = Instant::now() + Duration::from_secs(2);
    while !ack.ends_with(b"ok\r\n") && Instant::now() < deadline {
        ack.extend(receive_within(&mut comm, Duration::from_millis(100)));
    }
    assert_eq!(ack, b"ok\r\n");
    comm.send(&[0x85]).expect("send jog cancel");

    done_tx.send(()).expect("signal server");
    server.join().expect("server thread");
    comm.disconnect().expect("disconnect");
    assert!(!comm.is_connected());
}

#[test]
fn test_websocket_transport_reports_closed_connection() {
    let (listener, port) = listener();
    let server = std::thread::spawn(move || {
        let mut ws = accept(&listener);
        ws.close(None).expect("close");
        // Let the close handshake finish, then drop the socket.
        let deadline = Instant::now() + Duration::from_secs(2);
        while Instant::now() < deadline && ws.read().is_ok() {}
    });

    let mut comm = SerialCommunicator::new();
    comm.connect(&ConnectionParams::websocket("127.0.0.1", port))
        .expect("WebSocket connect");

    let deadline = Instant::now() + Duration::from_secs(2);
    let mut saw_error = false;
    while Instant::now() < deadline {
        if comm.receive().is_err() {
            saw_error = true;
            break;
        }
    }
    server.join().expect("server thread");
    assert!(
        saw_error,
        "a closed WebSocket must be reported, not read as no data"
    );
    assert!(!comm.is_connected());
    assert!(comm.receive().is_err());
}

#[test]
fn test_websocket_transport_idle_receive_returns_quickly() {
    let (listener, port) = listener();
    let (done_tx, done_rx) = mpsc::channel::<()>();
    let server = std::thread::spawn(move || {
        let _ws = accept(&listener);
        let _ = done_rx.recv_timeout(Duration::from_secs(10));
    });

    let params = ConnectionParams::websocket("127.0.0.1", port);
    assert!(params.timeout_ms >= 1000);
    let mut comm = SerialCommunicator::new();
    comm.connect(&params).expect("WebSocket connect");

    let started = Instant::now();
    let data = comm.receive().expect("an idle receive reports no data");
    let elapsed = started.elapsed();
    assert!(data.is_empty(), "{data:?}");
    assert!(
        elapsed < Duration::from_millis(200),
        "idle WebSocket receive blocked for {elapsed:?}"
    );
    assert!(comm.is_connected());

    comm.disconnect().expect("disconnect");
    done_tx.send(()).expect("signal server");
    server.join().expect("server thread");
}

#[test]
fn test_websocket_connect_fails_without_server() {
    // A port that was free a moment ago: nothing listens there now.
    let (listener, port) = listener();
    drop(listener);
    let mut params = ConnectionParams::websocket("127.0.0.1", port);
    params.timeout_ms = 1000;
    let mut comm = SerialCommunicator::new();
    assert!(comm.connect(&params).is_err());
    assert!(!comm.is_connected());
}

#[test]
fn test_websocket_connect_fails_on_plain_tcp_server() {
    // A server that is not a WebSocket (a telnet port) refuses the handshake.
    let (listener, port) = listener();
    let server = std::thread::spawn(move || {
        use std::io::Write;
        let (mut stream, _) = listener.accept().expect("accept connection");
        let _ = stream.write_all(b"Grbl 1.1h ['$' for help]\r\n\r\n");
        std::thread::sleep(Duration::from_millis(200));
    });
    let mut params = ConnectionParams::websocket("127.0.0.1", port);
    params.timeout_ms = 1000;
    let mut comm = SerialCommunicator::new();
    assert!(comm.connect(&params).is_err());
    assert!(!comm.is_connected());
    server.join().expect("server thread");
}
