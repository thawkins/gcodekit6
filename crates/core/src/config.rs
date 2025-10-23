use serde::Deserialize;
use std::time::Duration;

#[derive(Deserialize, Debug)]
struct ConfigFile {
    network_timeout_secs: Option<u64>,
}

/// Determine network timeout using precedence:
/// 1. Environment variable `GCK_NETWORK_TIMEOUT_SECS`
/// 2. Config file in platform data dir: `gcodekit6/config.json`
/// 3. Fallback to `gcodekit_utils::settings::network_timeout()` default
pub fn network_timeout() -> Duration {
    // 1. env var
    if let Ok(s) = std::env::var("GCK_NETWORK_TIMEOUT_SECS") {
        if let Ok(v) = s.parse::<u64>() {
            if v > 0 {
                return Duration::from_secs(v);
            }
        }
    }

    // 2. config file
    if let Some(proj_dirs) = dirs_next::data_local_dir().map(|p| p.join("gcodekit6")) {
        let cfg_path = proj_dirs.join("config.json");
        if cfg_path.exists() {
            if let Ok(contents) = std::fs::read_to_string(&cfg_path) {
                if let Ok(cf) = serde_json::from_str::<ConfigFile>(&contents) {
                    if let Some(secs) = cf.network_timeout_secs {
                        if secs > 0 {
                            return Duration::from_secs(secs);
                        }
                    }
                }
            }
        }
    }

    // 3. fallback
    gcodekit_utils::settings::network_timeout()
}
