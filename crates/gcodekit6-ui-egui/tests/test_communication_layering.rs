//! The egui front end reaches the communication layer only through `gcodekit6_app`
//! (its re-exports of the controller types and `list_ports`), never directly.

use std::path::Path;

/// Collects every `.rs` file under `dir`.
fn rust_files(dir: &Path, out: &mut Vec<std::path::PathBuf>) {
    for entry in std::fs::read_dir(dir).expect("read src dir") {
        let path = entry.expect("dir entry").path();
        if path.is_dir() {
            rust_files(&path, out);
        } else if path.extension().is_some_and(|e| e == "rs") {
            out.push(path);
        }
    }
}

#[test]
fn test_communication_layering_src_does_not_name_the_communication_crate() {
    let src = Path::new(env!("CARGO_MANIFEST_DIR")).join("src");
    let mut files = Vec::new();
    rust_files(&src, &mut files);
    assert!(!files.is_empty(), "found the front end's sources");

    let offenders: Vec<String> = files
        .iter()
        .filter(|path| {
            std::fs::read_to_string(path)
                .expect("read source file")
                .contains("gcodekit6_communication")
        })
        .map(|path| path.display().to_string())
        .collect();
    assert!(
        offenders.is_empty(),
        "use the gcodekit6_app re-exports instead of gcodekit6_communication in: {offenders:?}"
    );
}

#[test]
fn test_communication_layering_app_reexports_the_controller_types() {
    let _status: Option<gcodekit6_app::FullStatus> = None;
    let _overrides: Option<gcodekit6_app::OverrideState> = None;
    assert_eq!(
        gcodekit6_app::ControllerProtocol::default(),
        gcodekit6_app::ControllerProtocol::Unknown
    );
    let _settings = gcodekit6_app::SettingsManager::new();
    let _ports: fn() -> _ = gcodekit6_app::list_ports;
    let _probe = gcodekit6_app::parse_probe_report_in;
    let _wco: Option<gcodekit6_app::WorkCoordinateOffset> = None;
}
