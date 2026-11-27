//! Tests for `gcodekit6_camtools::speeds_feeds::calculator` (moved from the inline test module).

use gcodekit6_camtools::speeds_feeds::*;
use gcodekit6_core::data::tools::ToolGeometry;

#[test]
fn test_operation_type_multipliers() {
    assert!(
        OperationType::Adaptive.feed_rate_multiplier()
            > OperationType::Slotting.feed_rate_multiplier()
    );
    assert_eq!(OperationType::Slotting.radial_engagement_percent(), 100.0);
    assert_eq!(OperationType::Profiling.radial_engagement_percent(), 30.0);
}

#[test]
fn test_geometry_adjustments() {
    let high_helix = ToolGeometry {
        helix_angle: Some(50.0),
        ..Default::default()
    };
    assert!(GeometryAdjustments::helix_factor(&high_helix) > 1.0);

    let low_engagement = GeometryAdjustments::radial_engagement_factor(20.0);
    let high_engagement = GeometryAdjustments::radial_engagement_factor(100.0);
    assert!(low_engagement > high_engagement);
}

#[test]
fn test_rpm_calculation() {
    let rpm = SpeedsFeedsCalculator::rpm_from_surface_speed(200.0, 6.0);
    // RPM = (200 * 1000) / (π * 6) ≈ 10,610
    assert!(rpm > 10000.0);
    assert!(rpm < 11000.0);
}

/// Aluminium 6061 with the 1/4" flat end mill on a spindle of `max_rpm`.
fn aluminium_input(max_rpm: u32) -> CalculationInput {
    use gcodekit6_core::data::materials::{MaterialId, init_standard_library as materials};
    use gcodekit6_core::data::tools::{ToolId, init_standard_library as tools};
    CalculationInput {
        material: materials()
            .get_material(&MaterialId("metal_al_6061".to_string()))
            .expect("aluminium in the standard library")
            .clone(),
        tool: tools()
            .get_tool(&ToolId("tool_1_4_flat".to_string()))
            .expect("1/4 flat in the standard library")
            .clone(),
        device: gcodekit6_devicedb::model::DeviceProfile {
            max_spindle_speed_rpm: max_rpm,
            max_feed_rate: 5000.0,
            ..Default::default()
        },
        operation: OperationType::Slotting,
        depth_of_cut: 3.0,
        width_of_cut: Some(1.0),
        tool_stick_out: 25.0,
        coolant_enabled: false,
        conservative: true,
    }
}

/// A spindle whose maximum is below the 1000 RPM floor runs at its maximum, never above
/// it, and the feed scales with it from the calculated RPM.
#[test]
fn test_speeds_feeds_machine_max_wins_over_floor() {
    let free = SpeedsFeedsCalculator::calculate(&aluminium_input(100_000));
    assert_eq!(free.unclamped_rpm, None);
    let slow = SpeedsFeedsCalculator::calculate(&aluminium_input(500));
    assert_eq!(slow.rpm, 500);
    assert_eq!(slow.unclamped_rpm, Some(free.rpm));
    let expected_feed = free.feed_rate * 500.0 / free.rpm as f32;
    assert!(
        (slow.feed_rate - expected_feed).abs() < 0.01,
        "{} != {expected_feed}",
        slow.feed_rate
    );
}

/// A tool rated below the floor also wins over the floor.
#[test]
fn test_speeds_feeds_tool_rating_wins_over_floor() {
    let mut input = aluminium_input(24_000);
    input.tool.limits.max_rpm_rating = Some(800);
    let result = SpeedsFeedsCalculator::calculate(&input);
    assert_eq!(result.rpm, 800);
    assert!(result.unclamped_rpm.is_some_and(|rpm| rpm > 800));
}

/// Raising a slow calculated RPM to the floor is recorded like a clamp.
#[test]
fn test_speeds_feeds_floor_raise_is_recorded() {
    let mut input = aluminium_input(24_000);
    // 300 m/min on a 200 mm cutter is about 477 RPM.
    input.tool.diameter = 200.0;
    let result = SpeedsFeedsCalculator::calculate(&input);
    assert_eq!(result.rpm, 1000);
    assert!(result.unclamped_rpm.is_some_and(|rpm| rpm < 1000));
}

/// Power is `Kc * MRR / 60e6 / 0.85` kW. Worked example (the GTK default page): aluminium's
/// default Kc of 200 N/mm^2 and an MRR of 7383.3 mm^3/min give
/// 200 * 7383.3 / 60e6 / 0.85 = 0.028954 kW (29 W).
#[test]
fn test_speeds_feeds_power_is_kc_times_mrr() {
    let worked = 200.0_f32 * 7383.3 / 60_000_000.0 / 0.85;
    assert!((worked - 0.028_954).abs() < 1e-6, "{worked}");

    let result = SpeedsFeedsCalculator::calculate(&aluminium_input(24_000));
    let expected = 200.0 * result.material_removal_rate / 60_000_000.0 / 0.85;
    assert!(
        (result.power_required_kw - expected).abs() < 1e-7,
        "{} != {expected}",
        result.power_required_kw
    );
}

/// Power depends on the removal rate only, not on the flute count or the stepover ratio.
#[test]
fn test_speeds_feeds_power_ignores_flutes_and_stepover_floor() {
    for (flutes, woc) in [(2, 1.0), (4, 1.0), (2, 0.1)] {
        let mut input = aluminium_input(24_000);
        input.tool.flutes = flutes;
        input.width_of_cut = Some(woc);
        input.material = input.material.with_specific_cutting_force(1000.0);
        let result = SpeedsFeedsCalculator::calculate(&input);
        let expected = 1000.0 * result.material_removal_rate / 60_000_000.0 / 0.85;
        assert!(
            (result.power_required_kw - expected).abs() < 1e-7,
            "flutes {flutes}, woc {woc}: {} != {expected}",
            result.power_required_kw
        );
    }
}
