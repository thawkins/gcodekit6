use std::io::Write;
use std::path::PathBuf;

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
