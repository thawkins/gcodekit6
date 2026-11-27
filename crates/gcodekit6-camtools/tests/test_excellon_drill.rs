//! Excellon drill files (owner decision D-4 (a)): KiCad, Eagle and Altium style files parse
//! to the right hole positions and tool diameters, and the Drill Holes layer drills one
//! plunge per hole, grouped by tool, smallest drill first.

use std::path::PathBuf;

use gcodekit6_camtools::excellon::{DrillFeature, ExcellonFile, looks_like_excellon, parse};
use gcodekit6_camtools::gerber::{
    GerberConverter, GerberLayerType, GerberParameters, detect_layer_files,
};

const TOL: f64 = 0.001;

/// KiCad 7, metric, decimal coordinates.
const KICAD_METRIC: &str = "M48\n\
; DRILL file {KiCad 7.0.1} date 2026-11-28\n\
; FORMAT={-:-/ absolute / metric / decimal}\n\
; #@! TF.FileFunction,Plated,1,2,PTH\n\
FMAT,2\n\
METRIC\n\
; #@! TA.AperFunction,Plated,PTH,ComponentDrill\n\
T1C0.800\n\
T2C1.000\n\
%\n\
G90\n\
G05\n\
T2\n\
X137.16Y-88.9\n\
X139.7Y-88.9\n\
T1\n\
X10.0Y20.0\n\
Y25.5\n\
X12.25\n\
M30\n";

/// KiCad, inch, suppressed leading zeros (TZ) with the format comment 2:4.
const KICAD_INCH_TZ: &str = "M48\n\
; FORMAT={2:4/ absolute / inch / suppress leading zeros}\n\
INCH,TZ\n\
T1C0.0394\n\
%\n\
G90\n\
T1\n\
X5000Y-12500\n\
X-250Y100\n\
M30\n";

/// Eagle, metric, trailing zeros kept (TZ), explicit 3.3 pattern.
const EAGLE_METRIC_TZ: &str = "M48\n\
;GenerationSoftware,Autodesk,EAGLE,9.6.2*%\n\
FMAT,2\n\
ICI,OFF\n\
METRIC,TZ,000.000\n\
T1C0.600\n\
T2C3.200\n\
%\n\
G90\n\
M71\n\
T1\n\
X1270Y2540\n\
X12700Y-500\n\
T2\n\
X3000Y3000\n\
M30\n";

/// Eagle, inch, leading zeros kept (LZ), explicit 2.4 pattern.
const EAGLE_INCH_LZ: &str = "M48\n\
INCH,LZ,00.0000\n\
T01C0.0320\n\
%\n\
T01\n\
X0105Y02\n\
X-01Y0015\n\
M30\n";

/// Altium, inch, leading zeros kept, FILE_FORMAT=2:5.
const ALTIUM_INCH_LZ: &str = "M48\n\
;Layer_Color=9474304\n\
;FILE_FORMAT=2:5\n\
INCH,LZ\n\
;TYPE=PLATED\n\
T1F00S00C0.03543\n\
%\n\
T01\n\
X0125Y-005\n\
M30\n";

/// The hit positions of `file`, in file order.
fn hits(file: &ExcellonFile) -> Vec<(u32, f64, f64)> {
    file.operations
        .iter()
        .filter_map(|op| match op.feature {
            DrillFeature::Hit { x, y } => Some((op.tool, x, y)),
            DrillFeature::Slot { .. } => None,
        })
        .collect()
}

/// Asserts `actual` hits equal `expected` within [`TOL`].
fn assert_hits(actual: &[(u32, f64, f64)], expected: &[(u32, f64, f64)]) {
    assert_eq!(actual.len(), expected.len(), "{actual:?}");
    for (a, e) in actual.iter().zip(expected) {
        assert_eq!(a.0, e.0, "tool of {a:?}");
        assert!(
            (a.1 - e.1).abs() < TOL && (a.2 - e.2).abs() < TOL,
            "{a:?} != {e:?}"
        );
    }
}

/// Asserts tool `n` of `file` is `mm` in diameter within [`TOL`].
fn assert_tool(file: &ExcellonFile, n: u32, mm: f64) {
    let d = file.tools.get(&n).copied().expect("tool defined");
    assert!((d - mm).abs() < TOL, "T{n} is {d} mm, not {mm}");
}

#[test]
fn test_excellon_kicad_metric_decimal_with_modal_coordinates() {
    let file = parse(KICAD_METRIC).expect("parse");
    assert_tool(&file, 1, 0.8);
    assert_tool(&file, 2, 1.0);
    assert_hits(
        &hits(&file),
        &[
            (2, 137.16, -88.9),
            (2, 139.7, -88.9),
            (1, 10.0, 20.0),
            (1, 10.0, 25.5),
            (1, 12.25, 25.5),
        ],
    );
}

#[test]
fn test_excellon_kicad_inch_trailing_zeros() {
    let file = parse(KICAD_INCH_TZ).expect("parse");
    assert_tool(&file, 1, 0.0394 * 25.4);
    assert_hits(
        &hits(&file),
        &[
            (1, 0.5 * 25.4, -1.25 * 25.4),
            (1, -0.025 * 25.4, 0.01 * 25.4),
        ],
    );
}

#[test]
fn test_excellon_eagle_metric_trailing_zeros() {
    let file = parse(EAGLE_METRIC_TZ).expect("parse");
    assert_tool(&file, 1, 0.6);
    assert_tool(&file, 2, 3.2);
    assert_hits(
        &hits(&file),
        &[(1, 1.27, 2.54), (1, 12.7, -0.5), (2, 3.0, 3.0)],
    );
}

#[test]
fn test_excellon_eagle_inch_leading_zeros() {
    let file = parse(EAGLE_INCH_LZ).expect("parse");
    assert_tool(&file, 1, 0.032 * 25.4);
    // LZ 2.4: "0105" is 01.05, "02" is 02.0000, "-01" is -01.0000, "0015" is 00.15.
    assert_hits(
        &hits(&file),
        &[(1, 1.05 * 25.4, 2.0 * 25.4), (1, -25.4, 0.15 * 25.4)],
    );
}

#[test]
fn test_excellon_altium_file_format_comment() {
    let file = parse(ALTIUM_INCH_LZ).expect("parse");
    assert_tool(&file, 1, 0.03543 * 25.4);
    // LZ 2:5: "0125" is 01.25000, "-005" is -00.5.
    assert_hits(&hits(&file), &[(1, 1.25 * 25.4, -0.5 * 25.4)]);
}

#[test]
fn test_excellon_slots_and_incremental_moves() {
    let content = "M48\nMETRIC\nT1C1.2\n%\nT1\nX1.0Y1.0G85X5.0Y1.0\n\
                   G00X10.0Y10.0\nM15\nG01X10.0Y14.0\nM16\nG05\n\
                   G91\nX1.0Y1.0\nX1.0\nM30\nX99.0Y99.0\n";
    let file = parse(content).expect("parse");
    let features: Vec<DrillFeature> = file.operations.iter().map(|op| op.feature).collect();
    assert_eq!(
        features,
        vec![
            DrillFeature::Slot {
                start: (1.0, 1.0),
                end: (5.0, 1.0)
            },
            DrillFeature::Slot {
                start: (10.0, 10.0),
                end: (10.0, 14.0)
            },
            DrillFeature::Hit { x: 11.0, y: 15.0 },
            DrillFeature::Hit { x: 12.0, y: 15.0 },
        ],
        "everything after M30 is ignored"
    );
}

#[test]
fn test_excellon_non_drill_content_is_refused() {
    assert!(!looks_like_excellon("%FSLAX46Y46*%\nX0Y0D02*\n"));
    assert!(!looks_like_excellon("README\n"));
    assert!(looks_like_excellon("; drill\n\nM48\n"));
    assert!(parse("%FSLAX46Y46*%\n%MOMM*%\nX0Y0D02*\nM02*\n").is_err());
    assert!(parse("M48\nMETRIC\nT1C0.8\n%\nM30\n").is_err(), "no holes");
}

fn drill_params() -> GerberParameters {
    GerberParameters {
        layer_type: GerberLayerType::DrillHoles,
        offset_x: 2.0,
        offset_y: -1.0,
        ..GerberParameters::default()
    }
}

#[test]
fn test_excellon_drill_program_one_plunge_per_hit_grouped_by_tool() {
    let params = drill_params();
    let gcode = GerberConverter::generate(&params, KICAD_METRIC).expect("drill program");
    let plunges: Vec<&str> = gcode
        .lines()
        .filter(|l| l.starts_with("G1 Z-1.800"))
        .collect();
    assert_eq!(plunges.len(), 5, "{gcode}");

    // Smallest drill first: T1 (0.8 mm) then T2 (1.0 mm), each after a tool-change pause.
    let t1 = gcode
        .find("; Drill T1: 0.800 mm, 3 feature(s)")
        .expect("T1 group");
    let t2 = gcode
        .find("; Drill T2: 1.000 mm, 2 feature(s)")
        .expect("T2 group");
    assert!(t1 < t2, "{gcode}");
    assert_eq!(gcode.matches("\nM0 ; Tool change").count(), 2, "{gcode}");

    // Hole positions carry the board offset; each plunge follows its rapid.
    let lines: Vec<&str> = gcode.lines().collect();
    let rapids: Vec<&str> = lines
        .windows(2)
        .filter(|w| w[1].starts_with("G1 Z-1.800"))
        .map(|w| w[0])
        .collect();
    assert_eq!(
        rapids,
        vec![
            "G0 X12.000 Y19.000",
            "G0 X12.000 Y24.500",
            "G0 X14.250 Y24.500",
            "G0 X139.160 Y-89.900",
            "G0 X141.700 Y-89.900",
        ]
    );
    assert!(gcode.trim_end().ends_with("M5\nG0 X0 Y0"), "{gcode}");
}

#[test]
fn test_excellon_drill_program_routes_slots() {
    let content = "M48\nMETRIC\nT1C1.2\n%\nT1\nX1.0Y1.0G85X5.0Y1.0\nM30\n";
    let params = GerberParameters {
        offset_x: 0.0,
        offset_y: 0.0,
        ..drill_params()
    };
    let gcode = GerberConverter::generate(&params, content).expect("drill program");
    assert!(
        gcode.contains("G0 X1.000 Y1.000\nG1 Z-1.800 F100.0\nG1 X5.000 Y1.000\nG0 Z5.000\n"),
        "{gcode}"
    );
}

#[test]
fn test_excellon_drill_program_refuses_unsafe_parameters() {
    let shallow = GerberParameters {
        alignment_hole_depth: 0.0,
        ..drill_params()
    };
    assert!(GerberConverter::generate(&shallow, KICAD_METRIC).is_err());
    let no_z = GerberParameters {
        num_axes: 2,
        ..drill_params()
    };
    assert!(GerberConverter::generate(&no_z, KICAD_METRIC).is_err());
}

/// A fresh scratch folder under the git-ignored `target/temp`.
fn scratch(name: &str) -> PathBuf {
    let dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../target/temp/test_excellon_drill")
        .join(name);
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).expect("scratch folder");
    dir
}

#[test]
fn test_excellon_drill_file_detection() {
    let dir = scratch("xln");
    std::fs::write(dir.join("board.xln"), KICAD_METRIC).expect("write");
    assert_eq!(
        detect_layer_files(&dir).get(&GerberLayerType::DrillHoles),
        Some(&dir.join("board.xln"))
    );

    let dir = scratch("exc");
    std::fs::write(dir.join("board.exc"), KICAD_METRIC).expect("write");
    assert_eq!(
        detect_layer_files(&dir).get(&GerberLayerType::DrillHoles),
        Some(&dir.join("board.exc"))
    );

    // A `.txt` is a drill file only when its content starts like Excellon (Altium's
    // `Board-RoundHoles.TXT`); a README is not.
    let dir = scratch("txt_sniff");
    std::fs::write(dir.join("a-readme.txt"), "README\n").expect("write");
    std::fs::write(dir.join("board-roundholes.txt"), ALTIUM_INCH_LZ).expect("write");
    assert_eq!(
        detect_layer_files(&dir).get(&GerberLayerType::DrillHoles),
        Some(&dir.join("board-roundholes.txt"))
    );
}
