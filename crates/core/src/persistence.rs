use std::io::Write;
use std::path::PathBuf;
use crate::models::Job;

/// Save job history to the platform data directory as `jobs.json`.
pub fn save_job_history(jobs: Vec<Job>) -> std::io::Result<()> {
    let fname = "jobs.json";
    // Re-use utils storage helper which performs atomic write
    gcodekit_utils::storage::write_json(fname, &jobs).map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, e))
}

/// Load job history from platform data directory. Returns empty Vec if not found.
pub fn load_job_history() -> std::io::Result<Vec<Job>> {
    let fname = "jobs.json";
    match gcodekit_utils::storage::read_json(fname) {
        Ok(v) => Ok(v),
        Err(e) => {
            if e.kind() == std::io::ErrorKind::NotFound {
                Ok(Vec::new())
            } else {
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
    Ok(cfg_path)
}
