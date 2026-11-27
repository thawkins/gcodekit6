//! Gerber input safety (audit T10): unsupported constructs are counted, a non-Excellon
//! drill layer or a file with nothing to cut makes no program, the outline layer is chosen by file name and
//! the rubout uses that same file, and a panicking ring offset is logged.

use std::io::Write;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};

use gcodekit5_camtools::gerber::{
    GerberConverter, GerberLayerType, GerberParameters, UnsupportedConstructs, detect_layer_files,
    offset_ring_or_warn, unsupported_constructs,
};

const HEADER: &str = "%FSLAX46Y46*%\n%MOMM*%\n";

/// A fresh scratch folder under the git-ignored `target/temp`.
fn scratch(name: &str) -> PathBuf {
    let dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../target/temp/test_gerber_input_safety")
        .join(name);
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).expect("scratch folder");
    dir
}

/// A square outline stroke from (0,0) to (`size`,`size`) mm.
fn square_outline(size: u32) -> String {
    let s = size * 1_000_000;
    format!(
        "{HEADER}%ADD10C,0.1*%\nD10*\nX0Y0D02*\nX{s}Y0D01*\nX{s}Y{s}D01*\nX0Y{s}D01*\nX0Y0D01*\nM02*\n"
    )
}

#[test]
fn test_gerber_unsupported_constructs_are_counted() {
    let region = format!(
        "{HEADER}%ADD10C,0.1*%\nD10*\nG36*\nX0Y0D02*\nX10000000Y0D01*\n\
         X10000000Y10000000D01*\nX0Y0D01*\nG37*\nM02*\n"
    );
    let counts = unsupported_constructs(&region);
    assert_eq!(counts.regions, 1, "{counts:?}");
    assert!(!counts.is_empty());

    let polarity = format!("{HEADER}%ADD10C,1*%\nD10*\n%LPC*%\nX0Y0D03*\n%LPD*%\nM02*\n");
    assert_eq!(unsupported_constructs(&polarity).clear_polarity, 1);

    let macro_pad = format!("{HEADER}%AMPAD*\n1,1,1.0,0,0*%\n%ADD11PAD*%\nD11*\nX0Y0D03*\nM02*\n");
    assert_eq!(unsupported_constructs(&macro_pad).aperture_macros, 1);

    // A plain stroke board has nothing to warn about.
    assert_eq!(
        unsupported_constructs(&square_outline(10)),
        UnsupportedConstructs::default()
    );
}

#[test]
fn test_gerber_drill_layer_or_empty_file_makes_no_program() {
    let excellon = "M48\nMETRIC\nT1C0.800\n%\nT1\nX10.0Y10.0\nX20.0Y10.0\nM30\n";
    let drill = GerberParameters {
        layer_type: GerberLayerType::DrillHoles,
        ..GerberParameters::default()
    };
    // The Drill Holes layer drills an Excellon file (D-4) and refuses a Gerber one.
    assert!(GerberConverter::generate(&drill, excellon).is_ok());
    assert!(GerberConverter::generate(&drill, &square_outline(10)).is_err());
    // The same file picked as another layer has nothing to cut either.
    for layer_type in [GerberLayerType::BoardOutline, GerberLayerType::TopCopper] {
        let params = GerberParameters {
            layer_type,
            ..GerberParameters::default()
        };
        assert!(GerberConverter::generate(&params, excellon).is_err());
        assert!(GerberConverter::generate(&params, "README text\n").is_err());
    }
}

#[test]
fn test_gerber_txt_file_is_not_a_drill_layer() {
    let dir = scratch("txt");
    std::fs::write(dir.join("notes.txt"), "README\n").expect("write");
    std::fs::write(dir.join("board-drill.txt"), "M48\n").expect("write");
    let layers = detect_layer_files(&dir);
    assert_eq!(
        layers.get(&GerberLayerType::DrillHoles),
        Some(&dir.join("board-drill.txt"))
    );
}

/// Two outline files: the layer shown and the rubout's board are the first by file name.
#[test]
fn test_gerber_outline_choice_is_by_name_and_shared_with_rubout() {
    let dir = scratch("two_outlines");
    std::fs::write(dir.join("a-board.gko"), square_outline(20)).expect("write");
    std::fs::write(dir.join("z-outline.gbr"), square_outline(60)).expect("write");
    let layers = detect_layer_files(&dir);
    assert_eq!(
        layers.get(&GerberLayerType::BoardOutline),
        Some(&dir.join("a-board.gko"))
    );

    let params = GerberParameters {
        rubout: true,
        use_board_outline: true,
        tool_diameter: 0.4,
        directory: Some(dir.to_string_lossy().to_string()),
        ..GerberParameters::default()
    };
    let pad = format!("{HEADER}%ADD10C,1.0*%\nD10*\nX10000000Y10000000D03*\nM02*\n");
    let gcode = GerberConverter::generate(&params, &pad).expect("gerber");
    let rubout = &gcode[gcode.find("; Rubout").expect("rubout section")..];
    let max_x = rubout
        .lines()
        .filter(|l| l.starts_with("G0 X") || l.starts_with("G1 X"))
        .filter_map(|l| {
            l.split_whitespace()
                .nth(1)?
                .strip_prefix('X')?
                .parse::<f32>()
                .ok()
        })
        .fold(f32::MIN, f32::max);
    assert!(
        max_x > 15.0 && max_x < 21.0,
        "rubout reaches x {max_x}, not the 20 mm outline"
    );
}

/// Collects formatted log output.
#[derive(Clone, Default)]
struct Captured(Arc<Mutex<Vec<u8>>>);

impl Write for Captured {
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        self.0
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .extend_from_slice(buf);
        Ok(buf.len())
    }

    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}

#[test]
fn test_gerber_rubout_ring_panic_logs_warn() {
    let captured = Captured::default();
    let writer = captured.clone();
    let subscriber = tracing_subscriber::fmt()
        .with_writer(move || writer.clone())
        .with_ansi(false)
        .with_max_level(tracing::Level::WARN)
        .finish();
    let result = tracing::subscriber::with_default(subscriber, || {
        offset_ring_or_warn("rubout", || panic!("degenerate ring"))
    });
    assert!(result.is_none());
    let log = String::from_utf8_lossy(
        &captured
            .0
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner),
    )
    .to_string();
    assert!(log.contains("WARN"), "{log}");
    assert!(log.contains("rubout"), "{log}");
}
