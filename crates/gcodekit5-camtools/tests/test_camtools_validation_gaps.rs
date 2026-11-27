//! Parameter validation gaps closed by audit ticket T11 (P2CAM-4, P2CAM-5, P2CAM-6a).
//!
//! The tabbed box refuses feeds, powers, burns and play that emit `F0`/`S-n` or a
//! self-intersecting outline; the jigsaw refuses seeds whose f32 RNG state would stop
//! advancing; and the probe routines that never move to the safe height accept any.

use gcodekit5_camtools::jigsaw_puzzle::{JigsawPuzzleMaker, PuzzleParameters};
use gcodekit5_camtools::probe_routines::ProbeRoutineEngine;
use gcodekit5_camtools::tabbed_box::{BoxParameters, TabbedBoxMaker};
use gcodekit5_core::{Corner, Position, ProbeAxis, ProbeDirection, ProbeRoutine};

fn box_error(params: BoxParameters) -> String {
    match TabbedBoxMaker::new(params) {
        Ok(_) => String::new(),
        Err(e) => e,
    }
}

#[test]
fn test_tabbed_box_defaults_still_accepted() {
    assert_eq!(box_error(BoxParameters::default()), "");
}

#[test]
fn test_tabbed_box_rejects_non_positive_feed_rate() {
    for feed_rate in [0.0, -500.0] {
        let err = box_error(BoxParameters {
            feed_rate,
            ..BoxParameters::default()
        });
        assert_eq!(err, "Feed rate must be greater than 0", "{feed_rate}");
    }
}

#[test]
fn test_tabbed_box_rejects_negative_laser_power() {
    let err = box_error(BoxParameters {
        laser_power: -1,
        ..BoxParameters::default()
    });
    assert_eq!(err, "Laser power must not be negative");
}

#[test]
fn test_tabbed_box_rejects_negative_or_excessive_burn() {
    let base = BoxParameters::default();
    let narrowest = base.finger_joint.space.min(base.finger_joint.finger) * base.thickness;
    for burn in [-0.1, narrowest, narrowest + 1.0] {
        let err = box_error(BoxParameters {
            burn,
            ..BoxParameters::default()
        });
        assert!(
            err.starts_with("Burn must not be negative"),
            "{burn}: {err}"
        );
    }
    let just_under = BoxParameters {
        burn: narrowest * 0.9,
        ..BoxParameters::default()
    };
    assert_eq!(box_error(just_under), "");
}

#[test]
fn test_tabbed_box_rejects_play_that_closes_a_notch() {
    for play in [10.0, -10.0] {
        let mut p = BoxParameters::default();
        p.finger_joint.play = play;
        assert_eq!(
            box_error(p),
            "Play is too large for the finger and space widths",
            "{play}"
        );
    }
}

/// Random values a default 4 x 3 puzzle reads: 3 vertical cuts of (1 + 6 * 3) and 2
/// horizontal cuts of (1 + 6 * 4).
const DEFAULT_DRAWS: u32 = 3 * 19 + 2 * 25;

fn puzzle(seed: u32) -> Result<JigsawPuzzleMaker, String> {
    JigsawPuzzleMaker::new(PuzzleParameters {
        seed,
        ..PuzzleParameters::default()
    })
}

#[test]
fn test_jigsaw_rejects_seed_two_to_the_24() {
    let err = puzzle(1 << 24).err().unwrap_or_default();
    assert!(err.starts_with("Random seed must be at most"), "{err}");
    assert!(puzzle(u32::MAX).is_err());
}

#[test]
fn test_jigsaw_seed_limit_is_the_last_exact_state() {
    // The last state read is seed + draws - 1, which must not pass 2^24.
    let max = (1u32 << 24) + 1 - DEFAULT_DRAWS;
    assert!(puzzle(max).is_ok());
    assert_eq!(
        puzzle(max + 1).err().unwrap_or_default(),
        format!("Random seed must be at most {max} for this many pieces")
    );
}

#[test]
fn test_jigsaw_accepts_and_generates_with_a_seed_below_the_limit() {
    let mut maker = puzzle(42).expect("default seed");
    assert!(maker.generate().is_ok());
}

fn generates(routine: ProbeRoutine) -> Result<(), String> {
    ProbeRoutineEngine::new(routine)
        .generate(Position::default())
        .map(|_| ())
        .map_err(|e| e.to_string())
}

#[test]
fn test_probe_edge_corner_bore_boss_accept_zero_safe_height() {
    let routines = [
        ProbeRoutine::EdgeFind {
            axis: ProbeAxis::X,
            direction: ProbeDirection::Negative,
            safe_height: 0.0,
            probe_distance: 10.0,
            fast_feed: 100.0,
            slow_feed: 25.0,
            backoff: 2.0,
        },
        ProbeRoutine::CornerFind {
            corner: Corner::XminYmin,
            safe_height: 0.0,
            probe_distance: 10.0,
            fast_feed: 100.0,
            slow_feed: 25.0,
            backoff: 2.0,
        },
        ProbeRoutine::BoreCenter {
            diameter: 20.0,
            safe_height: 0.0,
            fast_feed: 100.0,
            slow_feed: 25.0,
        },
        ProbeRoutine::BossCenter {
            diameter: 20.0,
            safe_height: 0.0,
            fast_feed: 100.0,
            slow_feed: 25.0,
        },
    ];
    for routine in routines {
        assert_eq!(generates(routine.clone()), Ok(()), "{routine:?}");
    }
}

#[test]
fn test_probe_z_touch_still_requires_a_positive_safe_height() {
    let err = generates(ProbeRoutine::ZTouch {
        safe_height: 0.0,
        max_depth: 20.0,
        fast_feed: 100.0,
        slow_feed: 25.0,
        backoff: 2.0,
    });
    assert_eq!(err, Err("Safe height must be positive".to_string()));
}

#[test]
fn test_probe_bore_still_requires_positive_feeds() {
    let err = generates(ProbeRoutine::BoreCenter {
        diameter: 20.0,
        safe_height: 0.0,
        fast_feed: 0.0,
        slow_feed: 25.0,
    });
    assert_eq!(err, Err("Feed rates must be positive".to_string()));
}
