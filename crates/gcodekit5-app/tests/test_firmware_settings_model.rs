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

fn setting(
    number: u16,
    value: &str,
    eeprom_type: Option<u8>,
) -> gcodekit5_communication::firmware::grbl::settings::Setting {
    gcodekit5_communication::firmware::grbl::settings::Setting {
        number,
        name: String::new(),
        value: value.to_string(),
        numeric_value: value.parse().ok(),
        description: String::new(),
        range: None,
        read_only: false,
        unit: None,
        eeprom_type,
    }
}

#[test]
fn test_firmware_settings_typed_value_is_checked() {
    use gcodekit5_app::firmware_settings::{ValueError, check_value};
    let max_rate = setting(110, "500.000", None);
    assert_eq!(check_value(&max_rate, " 5000 "), Ok("5000".to_string()));
    // GRBL answers a negative value with `error:4`.
    assert_eq!(check_value(&max_rate, "-2.5"), Err(ValueError::Negative));
    assert_eq!(check_value(&max_rate, "0.5"), Ok("0.5".to_string()));
    assert_eq!(check_value(&max_rate, ""), Err(ValueError::Empty));
    assert_eq!(check_value(&max_rate, "   "), Err(ValueError::Empty));
    for bad in ["5,000", "abc", "1e", "inf", "NaN", "5 000"] {
        assert_eq!(
            check_value(&max_rate, bad),
            Err(ValueError::NotANumber),
            "{bad}"
        );
    }
    // A setting the controller reports as text (a grblHAL host name) takes text.
    let host = setting(305, "my-cnc", None);
    assert_eq!(check_value(&host, "shop-cnc"), Ok("shop-cnc".to_string()));
    assert_eq!(check_value(&host, ""), Err(ValueError::Empty));

    let float = setting(1011, "80.0000", Some(3));
    assert_eq!(check_value(&float, "80"), Ok("80".to_string()));
    assert_eq!(check_value(&float, "0,5"), Err(ValueError::NotANumber));
    let long = setting(1075, "115200", Some(2));
    assert_eq!(check_value(&long, "1.5"), Err(ValueError::NotAWholeNumber));
    assert_eq!(
        check_value(&long, "3000000000"),
        Err(ValueError::OutOfRange)
    );
    let byte = setting(1028, "1", Some(0));
    assert_eq!(check_value(&byte, "300"), Err(ValueError::OutOfRange));
    assert_eq!(check_value(&byte, "255"), Ok("255".to_string()));
    let short = setting(1030, "1", Some(1));
    assert_eq!(check_value(&short, "40000"), Err(ValueError::OutOfRange));
}

/// The GRBL masks and switches below `$100` (stored in a byte by GRBL 1.1) take a whole
/// number from 0 to 255; other GRBL settings take any number of 0 or more.
#[test]
fn test_firmware_settings_byte_settings_take_a_whole_number_up_to_255() {
    use gcodekit5_app::firmware_settings::{ValueError, check_value};
    let direction = setting(3, "0", None);
    let laser = setting(32, "0", None);
    for byte in [&direction, &laser] {
        assert_eq!(check_value(byte, "255"), Ok("255".to_string()));
        assert_eq!(check_value(byte, "0"), Ok("0".to_string()));
        assert_eq!(check_value(byte, "257"), Err(ValueError::OutOfRange));
        assert_eq!(check_value(byte, "1.5"), Err(ValueError::NotAWholeNumber));
        assert_eq!(check_value(byte, "-1"), Err(ValueError::Negative));
    }
    // Not a byte: a float below $100 and a setting above it.
    let pull_off = setting(27, "1.000", None);
    assert_eq!(check_value(&pull_off, "1.5"), Ok("1.5".to_string()));
    let steps = setting(100, "250.000", None);
    assert_eq!(check_value(&steps, "300.25"), Ok("300.25".to_string()));
    // A Repetier byte position keeps its own type rule.
    let repetier = setting(1028, "1", Some(0));
    assert_eq!(check_value(&repetier, "-1"), Err(ValueError::OutOfRange));
}
