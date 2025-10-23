use gcodekit_core::streamer_worker::StreamerWorker;
use gcodekit_device_adapters::Transport;
use std::sync::{Arc, Mutex};

struct MockTransport {
    written: Arc<Mutex<Vec<String>>>,
    will_ack: bool,
}

impl MockTransport {
    fn new(will_ack: bool) -> Self {
        MockTransport {
            written: Arc::new(Mutex::new(Vec::new())),
            will_ack,
        }
    }
}

impl Transport for MockTransport {
    fn send_line(&mut self, line: &str) -> std::io::Result<()> {
        self.written.lock().unwrap().push(line.to_string());
        Ok(())
    }

    fn emergency_stop(&mut self) -> std::io::Result<()> {
        self.written.lock().unwrap().push("EMERGENCY".to_string());
        Ok(())
    }

    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
    fn disconnect(&mut self) -> std::io::Result<()> {
        Ok(())
    }
    fn is_alive(&self) -> std::io::Result<bool> {
        Ok(true)
    }
    fn read_line(&mut self) -> std::io::Result<String> {
        if self.will_ack {
            Ok("ok".to_string())
        } else {
                Err(std::io::Error::other("no ack"))
        }
    }
}

#[test]
fn test_streamer_sends_lines_and_ack() {
    let m = MockTransport::new(true);
    let boxed: Box<dyn Transport> = Box::new(m);
    let sw = std::sync::Arc::new(StreamerWorker::new(boxed));
    let lines = vec!["G1 X1", "G1 X2"];
    let res = sw.stream_lines(lines);
    assert!(res.is_ok());
}

#[test]
fn test_emergency_stop() {
    let m = MockTransport::new(true);
    let written = m.written.clone();
    let boxed: Box<dyn Transport> = Box::new(m);
    let sw = std::sync::Arc::new(StreamerWorker::new(boxed));
    let lines_iter = vec!["G1 X1", "G1 X2", "G1 X3"];

    // Start streaming in background and then emergency stop
    let sw_clone = sw.clone();
    let handle = std::thread::spawn(move || {
        let _ = sw_clone.stream_lines(lines_iter);
    });

    std::thread::sleep(std::time::Duration::from_millis(1));
    let _ = sw.emergency_stop();
    handle.join().unwrap();

    let wrote = written.lock().unwrap();
    assert!(wrote.iter().any(|s| s == "EMERGENCY"));
}
