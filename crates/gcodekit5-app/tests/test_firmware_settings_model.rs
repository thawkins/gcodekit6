//! The toolkit-free firmware settings model: the category msgids stay marked for
//! `scripts/update-po.sh` (xgettext `--keyword=n_!`), Repetier writes use the type the
//! controller reported, and typed values are checked before anything is sent.

use gcodekit5_app::firmware_settings::{SETTING_CATEGORIES, category_of};

/// The literals the source marks with `n_!`, which `scripts/update-po.sh` extracts.
fn marked_literals() -> Vec<String> {
    let path =
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("src/firmware_settings/mod.rs");
    let source = std::fs::read_to_string(path).expect("read firmware_settings source");
    source
        .split("n_!(\"")
        .skip(1)
        .filter_map(|rest| rest.split_once("\")").map(|(id, _)| id.to_string()))
        .collect()
}

#[test]
fn test_firmware_settings_categories_are_marked_for_extraction() {
    let marked = marked_literals();
    for category in SETTING_CATEGORIES {
        assert!(
            marked.iter().any(|m| m == category),
            "{category:?} is not marked with n_!"
        );
    }
    for number in [
        0, 22, 33, 100, 110, 120, 130, 200, 340, 370, 384, 395, 550, 600, 650, 680, 1000, 999,
    ] {
        assert!(SETTING_CATEGORIES.contains(&category_of(number)));
    }
}

#[test]
fn test_firmware_settings_epr_line_keeps_its_type() {
    let float = gcodekit5_app::firmware_settings::parse_repetier_epr_line(
        "EPR:3 11 80.0000 X-axis steps per mm [steps/mm]",
    )
    .expect("parsed");
    assert_eq!(float.number, 1011);
    assert_eq!(float.eeprom_type, Some(3));
    let byte = gcodekit5_app::firmware_settings::parse_repetier_epr_line("EPR:0 1028 1 Ena")
        .expect("parsed");
    assert_eq!(byte.eeprom_type, Some(0));
}

#[test]
fn test_firmware_settings_repetier_write_uses_the_reported_type() {
    use gcodekit5_app::firmware_settings::write_command;
    // An integer typed into a float slot is still written as a float.
    assert_eq!(
        write_command(1011, Some(3), "80").as_deref(),
        Some("M206 T3 P11 X80")
    );
    assert_eq!(
        write_command(1075, Some(2), "115200").as_deref(),
        Some("M206 T2 P75 S115200")
    );
    assert_eq!(
        write_command(1028, Some(0), "1").as_deref(),
        Some("M206 T0 P28 S1")
    );
    assert_eq!(
        write_command(1030, Some(1), "5").as_deref(),
        Some("M206 T1 P30 S5")
    );
    // A position whose type the controller never reported is not written.
    assert_eq!(write_command(1011, None, "80"), None);
    assert_eq!(
        write_command(110, None, "5000").as_deref(),
        Some("$110=5000")
    );
}
