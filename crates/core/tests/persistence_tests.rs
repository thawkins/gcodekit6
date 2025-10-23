use chrono::Utc;
use gcodekit_core::models::Job;
use gcodekit_core::persistence::{load_job_history, save_job_history};

#[test]
fn test_save_and_load_job_history() {
    // Use a temporary XDG data home to isolate storage between tests
    let td = tempfile::tempdir().expect("tempdir");
    std::env::set_var("XDG_DATA_HOME", td.path());

    // Prepare two jobs
    let j1 = Job {
        id: "j1".into(),
        file_path: "a.gcode".into(),
        lines_total: 10,
        lines_sent: 10,
        progress: 1.0,
        status: gcodekit_core::models::JobStatus::Completed,
        created_at: Utc::now(),
    };
    let j2 = Job {
        id: "j2".into(),
        file_path: "b.gcode".into(),
        lines_total: 20,
        lines_sent: 5,
        progress: 0.25,
        status: gcodekit_core::models::JobStatus::Running,
        created_at: Utc::now(),
    };

    let jobs = vec![j1.clone(), j2.clone()];
    save_job_history(jobs).expect("save failed");

    if let Some(p) = gcodekit_utils::storage::file_path("jobs.json") {
        eprintln!("storage file: {} exists={}", p.display(), p.exists());
        if p.exists() {
            eprintln!(
                "raw contents:\n{}",
                std::fs::read_to_string(&p).unwrap_or_default()
            );
        }
    }

    // Directly use utils storage read_json to see what is deserialized in test runtime
    let direct: Result<Vec<Job>, _> = gcodekit_utils::storage::read_json("jobs.json");
    eprintln!("direct read result: {:?}", direct.as_ref().map(|v| v.len()));
    let loaded = load_job_history().expect("load failed");
    // Debug output
    eprintln!("Loaded {} jobs", loaded.len());
    for j in &loaded {
        eprintln!("job id: {}", j.id);
    }
    // Ensure at least two jobs exist and IDs match recent entries
    assert!(loaded.iter().any(|j| j.id == "j1"));
    assert!(loaded.iter().any(|j| j.id == "j2"));

    // Cleanup
    if let Some(p) = gcodekit_utils::storage::file_path("jobs.json") {
        let _ = std::fs::remove_file(p);
    }
}
use tempfile::tempdir;

#[test]
fn test_write_sample_config_creates_file() {
    let td = tempdir().expect("tempdir");
    std::env::set_var("XDG_DATA_HOME", td.path());
    let path = gcodekit_core::persistence::write_sample_config(11).expect("write");
    assert!(path.exists());
}
