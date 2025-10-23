use crate::models::Job;
use std::io::Write;
use std::path::PathBuf;
use tracing::{debug, info};

/// Save job history to the platform data directory as `jobs.json`.
pub fn save_job_history(jobs: Vec<Job>) -> std::io::Result<()> {
    let fname = "jobs.json";
    // Re-use utils storage helper which performs atomic write
    debug!(count = jobs.len(), "persistence::save_job_history: saving jobs");
    let res = gcodekit_utils::storage::write_json(fname, &jobs).map_err(std::io::Error::other);
    if res.is_ok() {
        info!(count = jobs.len(), "persistence::save_job_history: saved jobs successfully");
    } else {
        debug!(err = ?res.as_ref().err(), "persistence::save_job_history: failed to save jobs");
    }
    res
}

/// Load job history from platform data directory. Returns empty Vec if not found.
pub fn load_job_history() -> std::io::Result<Vec<Job>> {
    let fname = "jobs.json";
    debug!(fname = %fname, "persistence::load_job_history: loading");
    match gcodekit_utils::storage::read_json::<Vec<Job>>(fname) {
        Ok(v) => {
            info!(count = v.len(), "persistence::load_job_history: loaded jobs");
            Ok(v)
        }
        Err(e) => {
            if e.kind() == std::io::ErrorKind::NotFound {
                debug!(fname = %fname, "persistence::load_job_history: file not found, returning empty vec");
                Ok(Vec::new())
            } else {
                debug!(err = ?e, "persistence::load_job_history: error reading jobs");
                Err(e)
            }
        }
    }
}

/// Write a sample config.json into the platform data dir under gcodekit6.
/// Overwrites existing config.
pub fn write_sample_config(network_timeout_secs: u64) -> std::io::Result<PathBuf> {
    let data_home = dirs_next::data_local_dir().ok_or_else(|| {
        std::io::Error::new(std::io::ErrorKind::NotFound, "data_local_dir not found")
    })?;
    let cfg_dir = data_home.join("gcodekit6");
    std::fs::create_dir_all(&cfg_dir)?;
    let cfg_path = cfg_dir.join("config.json");
    let mut f = std::fs::File::create(&cfg_path)?;
    let contents = format!("{{ \"network_timeout_secs\": {} }}", network_timeout_secs);
    f.write_all(contents.as_bytes())?;
    info!(path = %cfg_path.display(), timeout_secs = network_timeout_secs, "persistence::write_sample_config: wrote sample config");
    Ok(cfg_path)
}
