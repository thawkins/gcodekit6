use std::time::Duration;

/// Return the configured network timeout duration.
/// Reads `GCK_NETWORK_TIMEOUT_SECS` from the environment (integer seconds).
/// Falls back to 30 seconds when unset or invalid.
pub fn network_timeout() -> Duration {
    match std::env::var("GCK_NETWORK_TIMEOUT_SECS") {
        Ok(s) => match s.parse::<u64>() {
            Ok(v) if v > 0 => Duration::from_secs(v),
            _ => Duration::from_secs(30),
        },
        Err(_) => Duration::from_secs(30),
    }
}
