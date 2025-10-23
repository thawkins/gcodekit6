use std::io;

use tokio::task;

use crate::serial;

/// Async wrapper around the blocking `SerialConnection` using spawn_blocking.
pub struct AsyncSerialTransport {
    // We keep the blocking serial connection in an Arc so spawn_blocking closures can access it.
    inner: std::sync::Arc<std::sync::Mutex<serial::SerialConnection>>,
}

impl AsyncSerialTransport {
    pub fn wrap(conn: serial::SerialConnection) -> Self {
        AsyncSerialTransport {
            inner: std::sync::Arc::new(std::sync::Mutex::new(conn)),
        }
    }

    pub async fn send_line(&self, line: &str) -> io::Result<()> {
        let s = line.to_string();
        let inner = std::sync::Arc::clone(&self.inner);
        task::spawn_blocking(move || {
            let mut g = inner.lock().unwrap();
            g.send_line(&s)
        })
    .await
    .map_err(|e| io::Error::other(format!("join error: {}", e)))??;
        Ok(())
    }

    pub async fn emergency_stop(&self) -> io::Result<()> {
        let inner = std::sync::Arc::clone(&self.inner);
        task::spawn_blocking(move || {
            let mut g = inner.lock().unwrap();
            g.emergency_stop()
        })
    .await
    .map_err(|e| io::Error::other(format!("join error: {}", e)))??;
        Ok(())
    }

    pub async fn flush(&self) -> io::Result<()> {
        let inner = std::sync::Arc::clone(&self.inner);
        task::spawn_blocking(move || {
            let mut g = inner.lock().unwrap();
            g.flush()
        })
    .await
    .map_err(|e| io::Error::other(format!("join error: {}", e)))??;
        Ok(())
    }

    pub async fn disconnect(&self) -> io::Result<()> {
        let inner = std::sync::Arc::clone(&self.inner);
        task::spawn_blocking(move || {
            let mut g = inner.lock().unwrap();
            g.disconnect()
        })
    .await
    .map_err(|e| io::Error::other(format!("join error: {}", e)))??;
        Ok(())
    }

    pub async fn is_alive(&self) -> io::Result<bool> {
        let inner = std::sync::Arc::clone(&self.inner);
        let res = task::spawn_blocking(move || {
            let g = inner.lock().unwrap();
            g.is_alive()
        })
    .await
    .map_err(|e| io::Error::other(format!("join error: {}", e)))?;
        res
    }

    pub async fn read_line(&self) -> io::Result<String> {
        let inner = std::sync::Arc::clone(&self.inner);
        let res = task::spawn_blocking(move || {
            let mut g = inner.lock().unwrap();
            g.read_line()
        })
    .await
    .map_err(|e| io::Error::other(format!("join error: {}", e)))?;
        res
    }
}
