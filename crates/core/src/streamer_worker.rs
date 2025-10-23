use gcodekit_device_adapters::Transport;
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::Duration;

pub struct StreamerWorker {
    transport: Arc<Mutex<Box<dyn Transport>>>,
    paused: Arc<Mutex<bool>>,
    stop_signal: Arc<Mutex<bool>>,
}

impl StreamerWorker {
    pub fn new(transport: Box<dyn Transport>) -> Self {
        StreamerWorker {
            transport: Arc::new(Mutex::new(transport)),
            paused: Arc::new(Mutex::new(false)),
            stop_signal: Arc::new(Mutex::new(false)),
        }
    }

    pub fn stream_lines<I>(&self, lines: I) -> Result<(), String>
    where
        I: IntoIterator,
        I::Item: AsRef<str>,
    {
        for line in lines {
            if *self.stop_signal.lock().unwrap() {
                return Ok(());
            }

            while *self.paused.lock().unwrap() {
                thread::sleep(Duration::from_millis(5));
                if *self.stop_signal.lock().unwrap() {
                    return Ok(());
                }
            }

            let mut t = self.transport.lock().unwrap();
            t.send_line(line.as_ref()).map_err(|e| e.to_string())?;
            // simple ack handling: try read_line with small timeout
            let _ = t.read_line().map_err(|e| e.to_string())?;
        }

        Ok(())
    }

    pub fn pause(&self) {
        let mut p = self.paused.lock().unwrap();
        *p = true;
    }

    pub fn resume(&self) {
        let mut p = self.paused.lock().unwrap();
        *p = false;
    }

    pub fn emergency_stop(&self) -> Result<(), String> {
        {
            let mut s = self.stop_signal.lock().unwrap();
            *s = true;
        }
        let mut t = self.transport.lock().unwrap();
        t.emergency_stop().map_err(|e| e.to_string())
    }
}
