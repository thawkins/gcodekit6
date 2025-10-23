use tempfile::tempdir;

#[test]
fn test_write_sample_config_creates_file() {
    let td = tempdir().expect("tempdir");
    std::env::set_var("XDG_DATA_HOME", td.path());
    let path = gcodekit_core::persistence::write_sample_config(11).expect("write");
    assert!(path.exists());
}
