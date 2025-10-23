//! Basic tracing initialization helpers for gcodekit6

use tracing_subscriber::util::SubscriberInitExt;
use tracing_subscriber::{fmt, layer::SubscriberExt, EnvFilter};

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
        .with(filter)
        .with(fmt_layer)
        .try_init()?;

    Ok(())
}
