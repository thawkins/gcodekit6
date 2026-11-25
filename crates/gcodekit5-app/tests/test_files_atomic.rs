//! The shared atomic file write: a whole new file or the old one, never half of either.

use gcodekit5_app::files::{write_atomic, write_atomic_with};
use std::path::PathBuf;

fn scratch(name: &str) -> PathBuf {
    let dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../target/temp/files_atomic")
        .join(format!("{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).expect("scratch dir");
    dir
}

fn names(dir: &std::path::Path) -> Vec<String> {
    let mut n: Vec<String> = std::fs::read_dir(dir)
        .expect("dir")
        .map(|e| e.expect("entry").file_name().to_string_lossy().into_owned())
        .collect();
    n.sort();
    n
}

#[test]
fn test_write_atomic_writes_and_replaces() {
    let dir = scratch("replace");
    let file = dir.join("a.json");
    write_atomic(&file, "one").expect("write");
    write_atomic(&file, "two").expect("replace");
    assert_eq!(std::fs::read_to_string(&file).expect("read"), "two");
    assert_eq!(names(&dir), ["a.json"], "no temporary file left");
}

#[test]
fn test_write_atomic_failure_keeps_the_old_file() {
    let dir = scratch("failure");
    let file = dir.join("a.json");
    write_atomic(&file, "kept").expect("write");
    let failed = write_atomic_with(&file, |_| Err(std::io::Error::other("disk full")));
    assert!(failed.is_err());
    assert_eq!(std::fs::read_to_string(&file).expect("read"), "kept");
    assert_eq!(names(&dir), ["a.json"], "temporary file removed");
}

#[test]
fn test_write_atomic_refuses_a_read_only_target() {
    let dir = scratch("readonly");
    let file = dir.join("a.json");
    write_atomic(&file, "ro").expect("write");
    let mut perms = std::fs::metadata(&file).expect("meta").permissions();
    perms.set_readonly(true);
    std::fs::set_permissions(&file, perms.clone()).expect("readonly");
    assert!(write_atomic(&file, "new").is_err());
    assert_eq!(std::fs::read_to_string(&file).expect("read"), "ro");
    #[allow(clippy::permissions_set_readonly_false)]
    perms.set_readonly(false);
    std::fs::set_permissions(&file, perms).expect("writable");
}
