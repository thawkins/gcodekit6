use anyhow::{bail, Result};
use gcodekit_device_adapters::Transport;
use std::sync::{Arc, Mutex};
use std::thread;
use tracing::{debug, info, warn};

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
    #[allow(clippy::while_immutable_condition)]
    pub fn stream<I>(&self, lines: I) -> Result<()>
    where
        I: IntoIterator,
        I::Item: AsRef<str>,
    {
    #[allow(clippy::while_immutable_condition)]
    let mut in_flight = 0usize;
        for line in lines {
            // Check stop signal
            if *self.stop_signal.lock().unwrap() {
                info!("streamer::stream: stop signal observed, exiting");
                return Ok(());
            }

            // Pause handling
            while *self.paused.lock().unwrap() {
                debug!("streamer::stream: paused, sleeping");
                thread::sleep(std::time::Duration::from_millis(10));
                if *self.stop_signal.lock().unwrap() {
                    info!("streamer::stream: stop observed while paused, exiting");
                    return Ok(());
                }
            }

            // Backpressure simple model
            while in_flight >= self.window {
                debug!(in_flight, window = self.window, "streamer::stream: backpressure applied");
                thread::sleep(std::time::Duration::from_millis(1));
            }

            // Send line and wait for an acknowledgement line (simple ACK model)
            {
                let mut t = self.transport.lock().unwrap();
                debug!(line = %line.as_ref(), "streamer::stream: sending line");
                t.send_line(line.as_ref())?;
                in_flight += 1;
                // Block waiting for a single-line ACK from device. Require
                // ACKs to start with "ok". Any other reply is treated as
                // an error to allow the streamer to stop on device errors.
                let ack = t.read_line()?;
                debug!(ack = %ack, "streamer::stream: received ack");
                if ack.to_lowercase().starts_with("ok") {
                    in_flight = in_flight.saturating_sub(1);
                } else {
                    warn!(ack = %ack, "streamer::stream: device reported error");
                    bail!("device reported error: {}", ack);
                }
            }
        }

        Ok(())
    }

    pub fn pause(&self) {
        let mut p = self.paused.lock().unwrap();
        *p = true;
        info!("streamer::pause: paused");
    }

    pub fn resume(&self) {
        let mut p = self.paused.lock().unwrap();
        *p = false;
        info!("streamer::resume: resumed");
    }

    pub fn emergency_stop(&self) -> Result<()> {
        // Signal stop and call transport emergency_stop
        {
            let mut s = self.stop_signal.lock().unwrap();
            *s = true;
        }
        let mut t = self.transport.lock().unwrap();
        info!("streamer::emergency_stop: invoking transport emergency_stop");
        t.emergency_stop()?;
        info!("streamer::emergency_stop: transport emergency_stop completed");
        Ok(())
    }
}
