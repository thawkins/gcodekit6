//! The toolkit-free Device Manager model: the edit form's checks (ported from the GTK
//! Device Manager's Save), the values a controller's `$$` dump fills in (GTK's Sync from
//! Device and the Device Config page's profile update), search, and the delete rule.

use std::collections::HashMap;

use gcodekit5_app::device_profiles::{
    DeleteRefusal, ProfileForm, ReportedLimits, delete_refusal, dump_target, ensure_active_profile,
    matches_search, store_reported_settings,
};
use gcodekit5_core::units::{FeedRateUnits, MeasurementSystem};
use gcodekit5_devicedb::DeviceManager;
use gcodekit5_devicedb::{ControllerType, DeviceProfile, DeviceType};
use gcodekit5_settings::{ConnectionSettings, ConnectionType};

const MM: MeasurementSystem = MeasurementSystem::Metric;
const MM_MIN: FeedRateUnits = FeedRateUnits::MmPerMin;

fn settings(pairs: &[(u16, &str)]) -> HashMap<u16, String> {
    pairs.iter().map(|(n, v)| (*n, v.to_string())).collect()
}

#[test]
fn test_device_profiles_form_round_trips_a_profile() {
    let profile = DeviceProfile {
        name: "Router".to_string(),
        controller_type: ControllerType::Repetier,
        device_type: DeviceType::LaserCutter,
        has_laser: true,
        laser_watts: 20.0,
        ..DeviceProfile::default()
    };
    let form = ProfileForm::from_profile(&profile, MM, MM_MIN);
    let saved = form
        .apply(&profile, MM, MM_MIN)
        .expect("unchanged form saves");
    assert_eq!(saved.id, profile.id);
    assert_eq!(saved.name, "Router");
    assert_eq!(saved.controller_type, ControllerType::Repetier);
    assert_eq!(saved.device_type, DeviceType::LaserCutter);
    assert_eq!(saved.x_axis.max, profile.x_axis.max);
    assert_eq!(saved.z_axis.max, profile.z_axis.max);
    assert_eq!(saved.max_feed_rate, profile.max_feed_rate);
    assert_eq!(saved.laser_watts, 20.0);
    assert_eq!(saved.max_spindle_speed_rpm, profile.max_spindle_speed_rpm);
}

#[test]
fn test_device_profiles_form_converts_display_units() {
    let profile = DeviceProfile::default();
    let mut form = ProfileForm::from_profile(&profile, MeasurementSystem::Imperial, MM_MIN);
    form.x_max = "10".to_string();
    let saved = form
        .apply(&profile, MeasurementSystem::Imperial, MM_MIN)
        .expect("saves");
    assert_eq!(saved.x_axis.max, 254.0);
}

/// One change to a form.
type Change = fn(&mut ProfileForm);

#[test]
fn test_device_profiles_form_rejects_bad_values() {
    let profile = DeviceProfile::default();
    let base = ProfileForm::from_profile(&profile, MM, MM_MIN);
    let cases: [(Change, &str); 6] = [
        (|f| f.tcp_port = "x".to_string(), "Invalid TCP Port"),
        (|f| f.timeout_ms = "-1".to_string(), "Invalid Timeout"),
        (|f| f.x_min = "abc".to_string(), "Invalid X Min"),
        (|f| f.num_axes = "7".to_string(), "Invalid No of Axes"),
        (
            |f| f.spindle_watts = "lots".to_string(),
            "Invalid Spindle Power",
        ),
        (
            |f| f.controller_type = ControllerType::Marlin,
            "Unsupported controller",
        ),
    ];
    for (change, title) in cases {
        let mut form = base.clone();
        change(&mut form);
        let error = form.apply(&profile, MM, MM_MIN).expect_err(title);
        assert_eq!(error.title, title);
    }
    // A disabled spindle is not checked and is saved as zero (GTK's rule).
    let mut form = base.clone();
    form.has_spindle = false;
    form.spindle_watts = "lots".to_string();
    let saved = form.apply(&profile, MM, MM_MIN).expect("saves");
    assert_eq!(saved.cnc_spindle_watts, 0.0);
    assert_eq!(saved.max_spindle_speed_rpm, 0);
    // A network connection needs a host without spaces.
    let mut form = base.clone();
    form.connection_type = "TCP/IP".to_string();
    form.tcp_host = "my host".to_string();
    let error = form.apply(&profile, MM, MM_MIN).expect_err("host");
    assert_eq!(error.title, "Invalid TCP Host");
}

#[test]
fn test_device_profiles_form_swaps_reversed_limits() {
    let profile = DeviceProfile::default();
    let mut form = ProfileForm::from_profile(&profile, MM, MM_MIN);
    form.y_min = "300".to_string();
    form.y_max = "10".to_string();
    let saved = form.apply(&profile, MM, MM_MIN).expect("saves");
    assert_eq!((saved.y_axis.min, saved.y_axis.max), (10.0, 300.0));
}

#[test]
fn test_device_profiles_reported_limits_follow_gtk_sync() {
    let limits = ReportedLimits::from_settings(&settings(&[
        (130, "400.000"),
        (131, "300.5"),
        (132, "80"),
        (30, "24000"),
        (32, "1"),
    ]));
    assert_eq!(limits.x_max, Some(400.0));
    assert_eq!(limits.y_max, Some(300.5));
    assert_eq!(limits.z_max, Some(80.0));
    assert_eq!(limits.max_rpm, Some(24000));
    assert_eq!(limits.laser, Some(true));

    let profile = DeviceProfile {
        has_spindle: false,
        ..DeviceProfile::default()
    };
    let mut form = ProfileForm::from_profile(&profile, MM, MM_MIN);
    form.sync(&limits, MM);
    let saved = form.apply(&profile, MM, MM_MIN).expect("saves");
    assert_eq!(saved.x_axis.max, 400.0);
    assert_eq!(saved.max_spindle_speed_rpm, 24000);
    assert_eq!(saved.max_s_value, 24000.0);
    assert!(saved.has_spindle && saved.has_laser);
}

#[test]
fn test_device_profiles_store_reported_settings() {
    let mut profile = DeviceProfile::default();
    store_reported_settings(
        &mut profile,
        &settings(&[(30, "1000"), (32, "1"), (110, "5000")]),
    );
    assert_eq!(
        profile.grbl_settings.get(&110).map(String::as_str),
        Some("5000")
    );
    assert_eq!(profile.max_s_value, 1000.0);
    assert!(profile.has_laser);
}

/// Ported from the retired device-status module tests: a setting value is read by its leading
/// number, so trailing text after it is ignored and a value with no number is skipped.
#[test]
fn test_device_profiles_store_reported_settings_reads_numeric_prefix() {
    let mut profile = DeviceProfile {
        max_s_value: 42.0,
        ..DeviceProfile::default()
    };
    store_reported_settings(
        &mut profile,
        &settings(&[(30, " 1000.500 (rpm)"), (32, "1 (laser)")]),
    );
    assert_eq!(profile.max_s_value, 1000.5);
    assert!(profile.has_laser);

    store_reported_settings(&mut profile, &settings(&[(30, "max"), (32, "0abc")]));
    assert_eq!(profile.max_s_value, 1000.5);
    assert!(!profile.has_laser);
}

#[test]
fn test_device_profiles_search_and_delete_rule() {
    let profile = DeviceProfile {
        name: "Shop Laser".to_string(),
        controller_type: ControllerType::Repetier,
        ..DeviceProfile::default()
    };
    assert!(matches_search(&profile, ""));
    assert!(matches_search(&profile, "shop"));
    assert!(matches_search(&profile, "REPETIER"));
    assert!(matches_search(&profile, "serial"));
    assert!(!matches_search(&profile, "lathe"));

    assert_eq!(
        delete_refusal("a", Some("a"), 2),
        Some(DeleteRefusal::Active)
    );
    assert_eq!(delete_refusal("a", None, 1), Some(DeleteRefusal::Last));
    assert_eq!(delete_refusal("b", Some("a"), 2), None);
}

#[test]
fn test_device_profiles_grbl_family_controllers_save() {
    for controller in [ControllerType::GrblHal, ControllerType::FluidNC] {
        let profile = DeviceProfile {
            controller_type: controller.clone(),
            ..DeviceProfile::default()
        };
        let mut form = ProfileForm::from_profile(&profile, MM, MM_MIN);
        form.name = "Renamed".to_string();
        let saved = form.apply(&profile, MM, MM_MIN).expect("GRBL family saves");
        assert_eq!(saved.name, "Renamed");
        assert_eq!(saved.controller_type, controller);
    }
}

fn serial(port: &str) -> ConnectionSettings {
    ConnectionSettings {
        port: port.to_string(),
        ..ConnectionSettings::default()
    }
}

fn profile(id: &str, port: &str) -> DeviceProfile {
    DeviceProfile {
        id: id.to_string(),
        name: id.to_string(),
        port: port.to_string(),
        ..DeviceProfile::default()
    }
}

#[test]
fn test_device_profiles_dump_target_follows_gtk_port_rule() {
    let profiles = [
        profile("router", "/dev/ttyUSB0"),
        profile("laser", "/dev/ttyACM0"),
        profile("auto", "Auto"),
    ];
    let target = |active: Option<&str>, port: &str| dump_target(&profiles, active, &serial(port));
    // The active profile on the connected port, or on "Auto".
    assert_eq!(
        target(Some("router"), "/dev/ttyUSB0").as_deref(),
        Some("router")
    );
    assert_eq!(
        target(Some("auto"), "/dev/ttyUSB0").as_deref(),
        Some("auto")
    );
    // Otherwise the profile with that port.
    assert_eq!(
        target(Some("router"), "/dev/ttyACM0").as_deref(),
        Some("laser")
    );
    // Otherwise nothing.
    assert_eq!(target(Some("router"), "/dev/ttyS9"), None);
    assert_eq!(target(None, "/dev/ttyS9"), None);
    // A network connection matches a profile's host and port.
    let net = DeviceProfile {
        tcp_host: "laser.local".to_string(),
        tcp_port: 8080,
        ..profile("net", "/dev/ttyS1")
    };
    let conn = ConnectionSettings {
        connection_type: ConnectionType::Tcp,
        port: "laser.local".to_string(),
        tcp_port: 8080,
        ..ConnectionSettings::default()
    };
    let with_net = [profiles[0].clone(), net];
    assert_eq!(
        dump_target(&with_net, Some("router"), &conn).as_deref(),
        Some("net")
    );
}

/// A fresh profiles file path under `target/temp/app_device_profiles/<name>-<pid>`.
fn scratch_file(name: &str) -> std::path::PathBuf {
    let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../target/temp/app_device_profiles")
        .join(format!("{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).expect("scratch");
    dir.join("devices.json")
}

#[test]
fn test_device_profiles_ensure_active_profile() {
    // No profiles: the default one is created and made active.
    let file = scratch_file("ensure_empty");
    let manager = DeviceManager::new(file.clone());
    ensure_active_profile(&manager).expect("ensure");
    assert_eq!(manager.get_all_profiles().len(), 1);
    assert!(manager.get_active_profile().is_some());

    // Profiles but none active: the first by name becomes active, on disk too.
    let file = scratch_file("ensure_none_active");
    let manager = DeviceManager::new(file.clone());
    manager.save_profile(profile("b", "Auto")).expect("save");
    manager
        .save_profile(DeviceProfile {
            name: "a".to_string(),
            ..profile("z", "Auto")
        })
        .expect("save");
    ensure_active_profile(&manager).expect("ensure");
    assert_eq!(
        manager.get_active_profile().map(|p| p.id).as_deref(),
        Some("z")
    );
    let reread = DeviceManager::new(file);
    reread.load().expect("load");
    assert_eq!(
        reread.get_active_profile().map(|p| p.id).as_deref(),
        Some("z")
    );
}
