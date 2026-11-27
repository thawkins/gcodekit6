//! Locks the per-user log file location used by the Windows release sink.

use std::io::Write;

fn scratch(name: &str) -> std::path::PathBuf {
    let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("target/temp")
        .join(name);
    let _ = std::fs::remove_dir_all(&dir);
    dir
}

#[test]
fn test_logging_file_creates_missing_dirs_and_appends() {
    let base = scratch("test_logging_file_appends");
    let mut first = gcodekit5_design::open_log_file(&base).unwrap();
    writeln!(first, "one").unwrap();
    let mut second = gcodekit5_design::open_log_file(&base).unwrap();
    writeln!(second, "two").unwrap();
    let text = std::fs::read_to_string(base.join("gcodekit5").join("gcodekit5.log")).unwrap();
    assert_eq!(text, "one\ntwo\n");
}
