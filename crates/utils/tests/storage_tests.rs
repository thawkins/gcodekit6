use gcodekit_utils::storage;
use serde::{Deserialize, Serialize};
use std::fs;

#[derive(Serialize, Deserialize, Debug, PartialEq)]
struct TestData {
    name: String,
    value: u32,
}

#[test]
fn test_write_and_read_json() {
    // Use a unique filename to avoid collisions in CI
    let now = chrono::Utc::now();
    let fname = format!(
        "test-storage-{}{:09}.json",
        now.timestamp(),
        now.timestamp_subsec_nanos()
    );

    let data = TestData {
        name: "hello".into(),
        value: 42,
    };

    // Write
    storage::write_json(&fname, &data).expect("write_json failed");

    // Read back
    let read: TestData = storage::read_json(&fname).expect("read_json failed");
    assert_eq!(read, data);

    // cleanup
    if let Some(p) = storage::file_path(&fname) {
        let _ = fs::remove_file(p);
    }
}
