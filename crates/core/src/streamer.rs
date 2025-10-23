use anyhow::{bail, Result};
use gcodekit_device_adapters::Transport;
use std::sync::{Arc, Mutex};
use std::thread;

/// Simple synchronous streamer that sends lines from an iterator to a Transport
/// with a window size controlling how many in-flight lines are allowed.
pub struct Streamer {
    transport: Arc<Mutex<Box<dyn Transport>>>,
    window: usize,
    paused: Arc<Mutex<bool>>,
    stop_signal: Arc<Mutex<bool>>,
}

impl Streamer {
    pub fn new(transport: Box<dyn Transport>, window: usize) -> Self {
        Streamer {
            transport: Arc::new(Mutex::new(transport)),
            window,
            paused: Arc::new(Mutex::new(false)),
            stop_signal: Arc::new(Mutex::new(false)),
        }
    }

    /// Stream lines from an iterator. This will block until the iterator is
    /// exhausted, pause is invoked, or emergency_stop is called.
    pub fn stream<I>(&self, lines: I) -> Result<()>
    where
        I: IntoIterator,
        I::Item: AsRef<str>,
    {
        let mut in_flight = 0usize;
        for line in lines {
            // Check stop signal
            if *self.stop_signal.lock().unwrap() {
                return Ok(());
            }

            // Pause handling
            while *self.paused.lock().unwrap() {
                thread::sleep(std::time::Duration::from_millis(10));
                if *self.stop_signal.lock().unwrap() {
                    return Ok(());
                }
            }

            // Backpressure simple model
            while in_flight >= self.window {
                thread::sleep(std::time::Duration::from_millis(1));
            }

            // Send line and wait for an acknowledgement line (simple ACK model)
            {
                let mut t = self.transport.lock().unwrap();
                t.send_line(line.as_ref())?;
                in_flight += 1;
                // Block waiting for a single-line ACK from device. Require
                // ACKs to start with "ok". Any other reply is treated as
                // an error to allow the streamer to stop on device errors.
                let ack = t.read_line()?;
                if ack.to_lowercase().starts_with("ok") {
                    in_flight = in_flight.saturating_sub(1);
                } else {
                    bail!("device reported error: {}", ack);
                }
            }
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

    pub fn emergency_stop(&self) -> Result<()> {
        // Signal stop and call transport emergency_stop
        {
            let mut s = self.stop_signal.lock().unwrap();
            *s = true;
        }
        let mut t = self.transport.lock().unwrap();
        t.emergency_stop()?;
        Ok(())
    }
}
