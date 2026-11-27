//! Locks audit T55 (COREBUILD-1d, owner decision D-10 a): the `Config` fields no code
//! read were dropped, and a settings file that still carries them must keep loading,
//! with every remaining value intact.
//!
//! Every test works in a directory of its own under `target/temp`.

use craft_settings::{SettingsStore, Source};
use gcodekit6_settings::Config;
use std::path::PathBuf;

/// A `config.toml` as an older build wrote it: every dropped key, next to
/// non-default values of fields that remain.
const OLD_TOML: &str = r#"
recent_files = ["/home/user/a.nc", "/home/user/b.nc"]
last_opened_path = "/home/user"

[connection]
port = "/dev/ttyUSB3"
baud_rate = 250000

[ui]
window_width = 1920
window_height = 1080
font_size = 14
language = "de"
startup_tab = "designer"
visualizer_sidebar_position = 480
device_config_sidebar_position = 300
tools_sidebar_position = 505
tools_manager_search = "endmill"

[file_processing]
enabled_preprocessors = ["comment_remover", "whitespace_cleaner"]
recent_files_count = 10
max_line_length = 128

[machine]
x_limit = 420.0
default_unit = "mm"

[machine.homing_direction]
X = true
Y = false

[machine.steps_per_degree]
A = 80.0

[machine.rotary_backlash]
A = 0.5

[probe]
default_safe_height = 10.0
default_fast_feed = 100.0
default_slow_feed = 20.0
default_backoff = 2.0
default_max_depth = -10.0
auto_update_wcs = true
target_wcs = 54
use_temporary_offset = false
apply_tool_length = false
preview_before_apply = true
setter_plate_x = 1.0
setter_plate_y = 2.0
setter_plate_z = 3.0
"#;

/// A fresh `target/temp/settings_dropped_keys/<test>-<pid>` directory.
fn scratch(test: &str) -> PathBuf {
    let dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../target/temp/settings_dropped_keys")
        .join(format!("{test}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).expect("create scratch dir");
    dir
}

/// The values of [`OLD_TOML`] that survive the field removal.
fn assert_kept(config: &Config) {
    assert_eq!(config.connection.port, "/dev/ttyUSB3");
    assert_eq!(config.connection.baud_rate, 250000);
    assert_eq!(config.ui.language, "de");
    assert_eq!(config.ui.tools_manager_search, "endmill");
    assert_eq!(config.file_processing.max_line_length, 128);
    assert_eq!(config.machine.x_limit, 420.0);
}

#[test]
fn test_config_toml_with_dropped_keys_loads_from_file() {
    let dir = scratch("test_config_toml_with_dropped_keys_loads_from_file");
    let path = dir.join("config.toml");
    std::fs::write(&path, OLD_TOML).expect("write config.toml");

    let config = Config::load_from_file(&path).expect("old config.toml loads");
    assert_kept(&config);
}

#[test]
fn test_config_toml_with_dropped_keys_opens_in_store() {
    let dir = scratch("test_config_toml_with_dropped_keys_opens_in_store");
    std::fs::write(dir.join("config.toml"), OLD_TOML).expect("write config.toml");

    let (store, load) = SettingsStore::<Config>::open_at(&dir);
    assert_eq!(load.source, Source::File);
    assert!(!load.has_problems(), "{load:?}");
    assert_kept(store.get());
}

#[test]
fn test_legacy_config_json_with_dropped_keys_loads() {
    let fixture =
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/gtk_config/config.json");
    let text = std::fs::read_to_string(&fixture).expect("read fixture");
    for key in [
        "window_width",
        "font_size",
        "startup_tab",
        "recent_files",
        "last_opened_path",
        "steps_per_degree",
        "setter_plate_z",
    ] {
        assert!(text.contains(key), "fixture still carries {key}");
    }

    let config = Config::load_from_file(&fixture).expect("legacy config.json loads");
    assert_eq!(config.ui.language, "fr");
    assert_eq!(config.machine.x_limit, 420.0);
}
