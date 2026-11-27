//! Tests for `gcodekit5_communication::firmware::protocol_registry`.
//!
//! Rewritten for audit T53 (P2FW-4): covers the fields with production readers
//! (`id`, `display_name`, `is_supported`, `announces_identity`, `init_commands`,
//! `detection_signatures`) and the `detect_protocol` order.

use gcodekit5_communication::firmware::ControllerProtocol;
use gcodekit5_communication::firmware::protocol_registry::{
    FALLBACK_PROFILE, PROTOCOLS, detect_protocol, protocol_profile,
};

#[test]
fn known_protocols_are_found() {
    for id in [
        "GRBL",
        "grblHAL",
        "FluidNC",
        "Repetier",
        "TinyG",
        "g2core",
        "Smoothieware",
        "Marlin",
    ] {
        let p = protocol_profile(id);
        assert_eq!(p.id, id);
        assert_eq!(p.display_name, id);
        assert!(p.announces_identity, "{id} announces its identity");
        assert!(!p.init_commands.is_empty(), "{id} has init commands");
    }
}

#[test]
fn unknown_and_empty_fall_back_to_safe_default() {
    for id in ["Siemens", ""] {
        let p = protocol_profile(id);
        assert_eq!(p.id, FALLBACK_PROFILE.id);
        assert_eq!(p.id, "Unknown");
        assert!(!p.is_supported);
        assert!(!p.announces_identity);
        assert!(p.init_commands.is_empty());
    }
}

#[test]
fn support_flags_match_wired_up_protocols() {
    assert!(protocol_profile("GRBL").is_supported);
    assert!(protocol_profile("Repetier").is_supported);
    for id in [
        "grblHAL",
        "FluidNC",
        "TinyG",
        "g2core",
        "Smoothieware",
        "Marlin",
    ] {
        assert!(!protocol_profile(id).is_supported, "{id} is unsupported");
    }
}

#[test]
fn init_commands_are_the_documented_queries() {
    assert_eq!(protocol_profile("GRBL").init_commands, ["$I", "$$"]);
    assert_eq!(protocol_profile("Repetier").init_commands, ["M110", "M115"]);
    assert_eq!(protocol_profile("Marlin").init_commands, ["M115"]);
}

#[test]
fn registry_order_puts_specific_banners_before_generic_grbl() {
    let pos = |id: &str| {
        PROTOCOLS
            .iter()
            .position(|p| p.id == id)
            .expect("in registry")
    };
    assert!(pos("Repetier") < pos("GRBL"));
    assert!(pos("grblHAL") < pos("GRBL"));
    assert!(pos("FluidNC") < pos("GRBL"));
}

#[test]
fn detect_protocol_follows_registry_order() {
    // A banner matching two entries resolves to the earlier one (Repetier
    // precedes Marlin); a lone Marlin banner maps to Unknown.
    assert_eq!(
        detect_protocol("FIRMWARE_NAME:Repetier based on Marlin"),
        Some(ControllerProtocol::Repetier)
    );
    assert_eq!(
        detect_protocol("FIRMWARE_NAME:Marlin 2.1"),
        Some(ControllerProtocol::Unknown)
    );
    assert_eq!(
        detect_protocol("Grbl 1.1h ['$' for help]"),
        Some(ControllerProtocol::Grbl)
    );
    assert_eq!(detect_protocol("ok"), None);
}
