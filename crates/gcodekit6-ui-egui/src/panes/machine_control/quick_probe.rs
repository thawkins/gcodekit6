//! Quick probes from Machine Control (the "Probe Z" button and its "Edge:" row): a Z
//! touch down to 20 mm, or an edge find 10 mm along X or Y, both at 100 mm/min, the values
//! of the quick buttons.
//!
//! Each runs through the Probe Routines generator and run of the CAM page
//! (`cam_tools::probe_routines`): the same guards (connected, no job, no other run, Idle with
//! a fresh report, probe input not triggered), the same stop-on-error stream with its
//! backstop, the `[PRB:...]` result and the `G90` restored after a failed probe. Beyond
//! one `G21 G91 G38.2` line and a restored `G90`, the generator also lifts 1 mm off the
//! surface (Z) or moves 1 mm clear of the edge after the touch, and writes no work offset
//! (no WCS is set).
//!
//! Dependencies: `egui`, `gcodekit6-app` (translations), `gcodekit6-core` (probe axes),
//! the CAM page's probe routines.

use gcodekit6_app::i18n::Translator;
use gcodekit6_camtools::probe_routines::defaults;
use gcodekit6_core::units::MeasurementSystem;
use gcodekit6_core::{ProbeAxis, ProbeDirection};

use crate::icons::{Icon, icon_only_button};
use crate::panes::cam_tools::common::format_len;
use crate::panes::cam_tools::probe_routines::{ProbeParams, RoutineKind};

/// The msgids this section shows.
pub(super) const MSGIDS: &[&str] = &[
    n_!("Probe Z"),
    n_!("Quick Z-touch probe at current XY position"),
    n_!("Edge:"),
    n_!("Quick edge find in direction"),
    n_!("Probe X- edge"),
    n_!("Probe X+ edge"),
    n_!("Probe Y- edge"),
    n_!("Probe Y+ edge"),
];

/// The quick Z probe distance (mm, downward).
pub const Z_DISTANCE_MM: f64 = 20.0;
/// The quick edge-find distance (mm).
pub const EDGE_DISTANCE_MM: f64 = 10.0;
/// The quick probe feed (mm/min).
pub const FEED_MM_MIN: f64 = 100.0;

/// A quick probe the user asked for.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum QuickProbe {
    /// Touch down along Z.
    Z,
    /// Find the edge along `axis` (X or Y) in `direction`.
    Edge(ProbeAxis, ProbeDirection),
}

impl QuickProbe {
    /// The Probe Routines input for this probe, in mm: the quick distance and feed, no back-off
    /// and re-probe, no descent before an edge probe, no work offset written.
    pub fn params(self) -> ProbeParams {
        let units = MeasurementSystem::Metric;
        let len = |mm: f64| format_len(mm, units);
        let feed = format!("{FEED_MM_MIN:.2}");
        let mut params = ProbeParams::new(units);
        params.fast_feed = feed.clone();
        params.slow_feed = feed;
        params.backoff = len(0.0);
        params.safe_height = len(defaults::CLEARANCE);
        params.probe_depth = len(0.0);
        params.tip_radius = len(0.0);
        params.plate_thickness = len(0.0);
        params.auto_update = false;
        match self {
            QuickProbe::Z => {
                params.routine = RoutineKind::ZTouch;
                params.max_depth = len(Z_DISTANCE_MM);
            }
            QuickProbe::Edge(axis, direction) => {
                params.routine = RoutineKind::EdgeFind;
                params.edge_axis = Some(axis);
                params.edge_direction = Some(direction);
                params.edge_distance = len(EDGE_DISTANCE_MM);
            }
        }
        params
    }
}

/// The edge buttons: icon, tooltip msgid, axis and direction, in button order.
const EDGES: [(Icon, &str, ProbeAxis, ProbeDirection); 4] = [
    (
        Icon::JogLeft,
        n_!("Probe X- edge"),
        ProbeAxis::X,
        ProbeDirection::Negative,
    ),
    (
        Icon::JogRight,
        n_!("Probe X+ edge"),
        ProbeAxis::X,
        ProbeDirection::Positive,
    ),
    (
        Icon::JogDown,
        n_!("Probe Y- edge"),
        ProbeAxis::Y,
        ProbeDirection::Negative,
    ),
    (
        Icon::JogUp,
        n_!("Probe Y+ edge"),
        ProbeAxis::Y,
        ProbeDirection::Positive,
    ),
];

/// Draws Probe Z and the Edge row, enabled when `enabled`; `text` (the last run's progress
/// or result) goes under them. Returns the probe clicked.
pub(super) fn show(
    ui: &mut egui::Ui,
    tr: &Translator,
    enabled: bool,
    text: Option<&str>,
) -> Option<QuickProbe> {
    let mut clicked = None;
    ui.horizontal_wrapped(|ui| {
        let probe_z = egui::Button::image_and_text(Icon::Touchpad.themed(ui), tr.tr("Probe Z"));
        if ui
            .add_enabled(enabled, probe_z)
            .on_hover_text(tr.tr("Quick Z-touch probe at current XY position"))
            .clicked()
        {
            clicked = Some(QuickProbe::Z);
        }
        ui.add_space(8.0);
        ui.weak(tr.tr("Edge:"))
            .on_hover_text(tr.tr("Quick edge find in direction"));
        for (icon, msgid, axis, direction) in EDGES {
            let button = ui.add_enabled_ui(enabled, |ui| icon_only_button(ui, icon, &tr.tr(msgid)));
            if button.inner.clicked() {
                clicked = Some(QuickProbe::Edge(axis, direction));
            }
        }
    });
    if let Some(text) = text {
        ui.label(text);
    }
    clicked
}
