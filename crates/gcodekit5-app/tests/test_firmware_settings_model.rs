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
