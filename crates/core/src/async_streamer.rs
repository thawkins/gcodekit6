use anyhow::Result;
#[cfg(feature = "async")]
use gcodekit_device_adapters::AsyncTransport;
use std::sync::Arc;
use std::sync::Mutex as StdMutex;
#[cfg(feature = "async")]
use tokio::sync::Mutex as AsyncMutex;
use tokio::sync::Notify;

/// AsyncStreamer wraps a blocking Transport object and performs sends/reads in
/// spawn_blocking so it can be used from async contexts. It supports pause,
/// resume, and emergency stop via a shared Notify and flags.
pub struct AsyncStreamer {
    #[cfg(not(feature = "async"))]
    transport: Arc<StdMutex<Box<dyn gcodekit_device_adapters::Transport>>>,

    #[cfg(feature = "async")]
    transport: Arc<AsyncMutex<Box<dyn AsyncTransport + Send + Sync>>>,

    window: usize,
    paused: Arc<StdMutex<bool>>,
    stop_flag: Arc<StdMutex<bool>>,
    notifier: Arc<Notify>,
}

impl AsyncStreamer {
    #[cfg(not(feature = "async"))]
    pub fn new(transport: Box<dyn gcodekit_device_adapters::Transport>, window: usize) -> Self {
        AsyncStreamer {
            transport: Arc::new(StdMutex::new(transport)),
            window,
            paused: Arc::new(StdMutex::new(false)),
            stop_flag: Arc::new(StdMutex::new(false)),
            notifier: Arc::new(Notify::new()),
        }
    }

    #[cfg(feature = "async")]
    pub fn new_async(transport: Box<dyn AsyncTransport + Send + Sync>, window: usize) -> Self {
        AsyncStreamer {
            transport: Arc::new(AsyncMutex::new(transport)),
            window,
            paused: Arc::new(StdMutex::new(false)),
            stop_flag: Arc::new(StdMutex::new(false)),
            notifier: Arc::new(Notify::new()),
        }
    }

    #[allow(clippy::while_immutable_condition)]
    pub async fn stream<I>(&self, lines: I) -> Result<()>
    where
        I: IntoIterator,
        I::Item: AsRef<str> + Send + 'static,
    {
    #[allow(clippy::while_immutable_condition)]
    let mut in_flight = 0usize;
        for line in lines {
            // Check stop
            if *self.stop_flag.lock().unwrap() {
                return Ok(());
            }

            // Pause
            while *self.paused.lock().unwrap() {
                self.notifier.notified().await;
                if *self.stop_flag.lock().unwrap() {
                    return Ok(());
                }
            }

            // Backpressure
            while in_flight >= self.window {
                // Yield to runtime
                tokio::task::yield_now().await;
            }

            #[cfg(feature = "async")]
            {
                let s = line.as_ref().to_string();
                // Lock the async transport and perform async send/read directly.
                let mut guard = self.transport.lock().await;
                guard.send_line(&s).await?;
                let ack = guard.read_line().await?;

                if ack.to_lowercase().starts_with("ok") {
                    in_flight = in_flight.saturating_sub(1);
                } else {
                    anyhow::bail!("device error: {}", ack);
                }
            }

            #[cfg(not(feature = "async"))]
            {
                // Send and wait for ack in blocking task
                let t = Arc::clone(&self.transport);
                let s = line.as_ref().to_string();
                let ack = tokio::task::spawn_blocking(move || {
                    let mut guard = t.lock().unwrap();
                    guard.send_line(&s)?;
                    let ack = guard.read_line()?;
                    Ok::<String, std::io::Error>(ack)
                })
                .await??;

                if ack.to_lowercase().starts_with("ok") {
                    in_flight = in_flight.saturating_sub(1);
                } else {
                    anyhow::bail!("device error: {}", ack);
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
        self.notifier.notify_waiters();
    }

    pub fn emergency_stop(&self) -> Result<()> {
        let mut s = self.stop_flag.lock().unwrap();
        *s = true;
        #[cfg(not(feature = "async"))]
        {
            let mut t = self.transport.lock().unwrap();
            t.emergency_stop()?;
        }
        #[cfg(feature = "async")]
        {
            let transport = Arc::clone(&self.transport);
            // If we're inside a tokio runtime, spawn a background task to call emergency_stop.
            // It's best-effort: we don't block waiting for the spawned task here.
            if tokio::runtime::Handle::try_current().is_ok() {
                tokio::spawn(async move {
                    let mut guard = transport.lock().await;
                    let _ = guard.emergency_stop().await;
                });
            } else {
                // No runtime available; create a new runtime and run the emergency_stop synchronously.
                let rt = tokio::runtime::Runtime::new()?;
                rt.block_on(async move {
                    let mut guard = transport.lock().await;
                    guard.emergency_stop().await
                })?;
            }
        }
        Ok(())
    }
}
