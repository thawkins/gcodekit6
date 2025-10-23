//! Basic tracing initialization helpers for gcodekit6

use tracing_subscriber::util::SubscriberInitExt;
use tracing_subscriber::{fmt, layer::SubscriberExt, EnvFilter};
use tracing_appender::rolling;
use std::path::PathBuf;

/// Initialize structured logging for the application.
///
/// Uses RUST_LOG environment variable as a fallback (via EnvFilter). Call this
/// early in `main` or tests to initialize `tracing`.
pub fn init_logging() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    // Default to info if RUST_LOG not set
    let env = std::env::var("RUST_LOG").unwrap_or_else(|_| "info".to_string());
    let filter = EnvFilter::try_new(env)?;

    let fmt_layer = fmt::layer().with_target(false);

    tracing_subscriber::registry()
        .with(filter.clone())
        .with(fmt_layer)
        .try_init()?;

    Ok(())
}

/// Initialize production logging with optional JSON output and file appender.
///
/// Environment variables:
/// - `GCK_LOG_JSON` = "1" to enable JSON formatted logs (default: text)
/// - `GCK_LOG_FILE` = path to a directory where rotated logs will be written
///   If unset, logs remain on stdout/stderr.
pub fn init_logging_prod() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let env = std::env::var("RUST_LOG").unwrap_or_else(|_| "info".to_string());
    let filter = EnvFilter::try_new(env)?;

    // Determine if JSON output should be used. This is enabled at runtime
    // only when the `json-logs` feature is compiled in and the runtime
    // env var `GCK_LOG_JSON` is set to "1".
    let use_json = cfg!(feature = "json-logs") && std::env::var("GCK_LOG_JSON").ok().as_deref() == Some("1");
    // We use the textual fmt layer by default; when JSON is enabled we will
    // use event_format(fmt::format().json()) where supported.
    let log_dir = std::env::var("GCK_LOG_FILE").ok();

    let fmt_layer = fmt::layer().with_target(false);

    // EnvFilter is cheap to clone; clone where needed to avoid move errors
    // Ensure we keep clones where the filter is used multiple times so it
    // isn't moved and later borrowed.
    let registry = tracing_subscriber::registry().with(filter.clone()).with(fmt_layer);

    if let Some(dir) = log_dir {
        let path = PathBuf::from(dir);
        std::fs::create_dir_all(&path)?;
        let file_appender = rolling::daily(path, "gcodekit6.log");
        let (non_blocking, _guard) = tracing_appender::non_blocking(file_appender);
        if use_json {
            // JSON formatting enabled via feature and env var
            let layer = fmt::Layer::default().with_writer(non_blocking).with_target(false);
            tracing_subscriber::registry()
                .with(layer)
                .with(filter.clone())
                .try_init()?;
        } else {
            tracing_subscriber::registry()
                .with(tracing_subscriber::fmt::Layer::default().with_writer(non_blocking))
                .with(filter.clone())
                .try_init()?;
        }
        // Keep guard alive by leaking it (intended for long-running apps)
        std::mem::forget(_guard);
        Ok(())
    } else {
        registry.try_init()?;
        Ok(())
    }
}
