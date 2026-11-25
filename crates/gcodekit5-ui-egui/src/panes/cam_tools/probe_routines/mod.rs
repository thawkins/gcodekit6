//! The Probe Tools page: touch probe routines for work coordinate setup (Z-touch, edge,
//! corner, bore and boss centre, tool length).
//!
//! Generate makes the routine's program, as GTK's "Generate G-code" did, for the editor.
//! Probe runs it on the machine ([`run`]): the program streams through the shared
//! `MachineController`, the `[PRB:...]` reports are read as they arrive, and the result can
//! be written into a work coordinate system with Apply to WCS (`G10 L2`).
//!
//! Every value is in millimetres (feeds in mm/min) whatever the display units, as on the
//! GTK page: its comment explains that unit-aware fields once read imperial values as mm.
//! GTK reported a field that was not a number in a dialog at Generate; here every field
//! is checked where it is typed, with the generator's own limits, and the actions wait.
//!
//! Dependencies: `egui`, `gcodekit5-camtools` (routine engine, presets), `gcodekit5-core`
//! (routine types), `gcodekit5-app` (translations).

mod page;
pub mod run;

use gcodekit5_app::i18n::Translator;
use gcodekit5_camtools::cam_presets::ProbePreset;
use gcodekit5_camtools::probe_routines::{
    ProbeOptions, ProbeRoutineEngine, ProbeRoutineOutput, defaults,
};
use gcodekit5_core::{Corner, Position, ProbeAxis, ProbeDirection, ProbeRoutine};

use super::common::{self, CamError, FieldError, check, not_negative, parse_number, positive};
pub use page::{ProbeView, show};

/// Field label msgids (GTK's titles).
const SAFE_HEIGHT: &str = n_!("Safe Height:");
const FAST_FEED: &str = n_!("Fast Feed Rate:");
const SLOW_FEED: &str = n_!("Slow Feed Rate:");
const BACKOFF: &str = n_!("Backoff Distance:");
const MAX_DEPTH: &str = n_!("Max Probe Depth");
const EDGE_DISTANCE: &str = n_!("Probe Distance");
const DIAMETER: &str = n_!("Diameter (mm)");
const SETTER_X: &str = n_!("Setter Plate X");
const SETTER_Y: &str = n_!("Setter Plate Y");
const SETTER_Z: &str = n_!("Setter Plate Z");
const PROBE_DEPTH: &str = n_!("Probe Depth:");
const TIP_RADIUS: &str = n_!("Probe Tip Radius:");
const PLATE_THICKNESS: &str = n_!("Plate Thickness:");

/// The msgids the page shows, for the translation check.
pub fn msgids() -> Vec<&'static str> {
    let mut ids = vec![
        SAFE_HEIGHT,
        FAST_FEED,
        SLOW_FEED,
        BACKOFF,
        MAX_DEPTH,
        EDGE_DISTANCE,
        DIAMETER,
        SETTER_X,
        SETTER_Y,
        SETTER_Z,
        PROBE_DEPTH,
        TIP_RADIUS,
        PLATE_THICKNESS,
    ];
    ids.extend(RoutineKind::ALL.map(RoutineKind::msgid));
    ids.extend(page::msgids());
    ids.extend(run::msgids());
    ids
}

/// The probe routines, in GTK's order.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RoutineKind {
    /// Surface probe to set Z=0.
    ZTouch,
    /// Single-axis edge detection.
    EdgeFind,
    /// Two-axis corner location.
    CornerFind,
    /// Internal diameter centre.
    BoreCenter,
    /// External diameter centre.
    BossCenter,
    /// Tool length with a setter plate.
    ToolLength,
}

impl RoutineKind {
    /// Every routine, in GTK's order.
    pub const ALL: [RoutineKind; 6] = [
        RoutineKind::ZTouch,
        RoutineKind::EdgeFind,
        RoutineKind::CornerFind,
        RoutineKind::BoreCenter,
        RoutineKind::BossCenter,
        RoutineKind::ToolLength,
    ];

    /// GTK's id (its preset files hold it).
    pub fn id(self) -> &'static str {
        match self {
            RoutineKind::ZTouch => "z_touch",
            RoutineKind::EdgeFind => "edge_find",
            RoutineKind::CornerFind => "corner_find",
            RoutineKind::BoreCenter => "bore_center",
            RoutineKind::BossCenter => "boss_center",
            RoutineKind::ToolLength => "tool_length",
        }
    }

    /// The msgid of the routine's name.
    pub fn msgid(self) -> &'static str {
        match self {
            RoutineKind::ZTouch => n_!("Z-Touch"),
            RoutineKind::EdgeFind => n_!("Edge Find"),
            RoutineKind::CornerFind => n_!("Corner Find"),
            RoutineKind::BoreCenter => n_!("Bore Center"),
            RoutineKind::BossCenter => n_!("Boss Center"),
            RoutineKind::ToolLength => n_!("Tool Length"),
        }
    }

    fn from_id(id: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|r| r.id() == id)
    }
}

/// GTK's edge axis, direction and corner ids.
const AXES: [(ProbeAxis, &str); 3] = [
    (ProbeAxis::X, "X"),
    (ProbeAxis::Y, "Y"),
    (ProbeAxis::Z, "Z"),
];
const DIRECTIONS: [(ProbeDirection, &str); 2] = [
    (ProbeDirection::Negative, "negative"),
    (ProbeDirection::Positive, "positive"),
];
const CORNERS: [(Corner, &str); 4] = [
    (Corner::XminYmin, "xmin_ymin"),
    (Corner::XmaxYmin, "xmax_ymin"),
    (Corner::XminYmax, "xmin_ymax"),
    (Corner::XmaxYmax, "xmax_ymax"),
];

/// The id of `value` in `table`.
fn id_of<T: PartialEq + Copy>(table: &[(T, &'static str)], value: Option<T>) -> Option<String> {
    let value = value?;
    table
        .iter()
        .find(|(v, _)| *v == value)
        .map(|(_, id)| (*id).to_string())
}

/// The value of `id` in `table`.
fn from_id<T: Copy>(table: &[(T, &'static str)], id: Option<&str>) -> Option<T> {
    let id = id?;
    table.iter().find(|(_, i)| *i == id).map(|(v, _)| *v)
}

/// The page's input: every length in mm and every feed in mm/min, as typed.
///
/// The edge axis, direction and corner start unchosen (`None`), as GTK's drop-downs did;
/// unchosen they mean X, negative and X-min / Y-min, and a preset saves them as `null`.
#[derive(Clone, Debug, PartialEq)]
pub struct ProbeParams {
    /// The routine.
    pub routine: RoutineKind,
    /// Safe height.
    pub safe_height: String,
    /// Fast feed rate.
    pub fast_feed: String,
    /// Slow feed rate.
    pub slow_feed: String,
    /// Backoff distance.
    pub backoff: String,
    /// Max probe depth.
    pub max_depth: String,
    /// Target work coordinate system, 54 to 59.
    pub wcs: u8,
    /// Write the result into the WCS from the program (G10 L20).
    pub auto_update: bool,
    /// Edge probe axis.
    pub edge_axis: Option<ProbeAxis>,
    /// Edge probe direction.
    pub edge_direction: Option<ProbeDirection>,
    /// Edge and corner probe distance.
    pub edge_distance: String,
    /// Corner to find.
    pub corner: Option<Corner>,
    /// Bore / boss diameter.
    pub diameter: String,
    /// Setter plate X.
    pub setter_x: String,
    /// Setter plate Y.
    pub setter_y: String,
    /// Setter plate Z.
    pub setter_z: String,
    /// How far below the start to lower before probing sideways.
    pub probe_depth: String,
    /// Stylus ball radius.
    pub tip_radius: String,
    /// Touch plate thickness.
    pub plate_thickness: String,
}

impl Default for ProbeParams {
    /// GTK's initial values.
    fn default() -> Self {
        let mm = |v: f64| format!("{v:.2}");
        Self {
            routine: RoutineKind::ZTouch,
            safe_height: mm(defaults::SAFE_HEIGHT),
            fast_feed: mm(defaults::FAST_FEED),
            slow_feed: mm(defaults::SLOW_FEED),
            backoff: mm(defaults::BACKOFF),
            max_depth: format!("{:.1}", defaults::MAX_PROBE_DEPTH),
            wcs: 54,
            auto_update: false,
            edge_axis: None,
            edge_direction: None,
            edge_distance: format!("{:.1}", defaults::PROBE_DISTANCE),
            corner: None,
            diameter: "20.0".into(),
            setter_x: "100.0".into(),
            setter_y: "100.0".into(),
            setter_z: "-50.0".into(),
            probe_depth: mm(defaults::PROBE_DEPTH),
            tip_radius: mm(0.0),
            plate_thickness: mm(0.0),
        }
    }
}

/// Rule: any value.
fn any(_v: f64) -> Option<&'static str> {
    None
}

impl ProbeParams {
    /// The routine and its options, or every problem with the fields the routine uses.
    ///
    /// # Errors
    /// The fields that are not numbers or that the routine engine would refuse: a safe
    /// height, feed, depth, distance or diameter that is not above 0, a negative backoff,
    /// probe depth, tip radius or plate thickness.
    pub fn parameters(&self) -> Result<(ProbeRoutine, ProbeOptions), Vec<FieldError>> {
        let mut errors = Vec::new();
        let mut num = |label, text: &str, rule: fn(f64) -> Option<&'static str>| {
            check(&mut errors, label, parse_number(text), rule)
        };
        let safe_height = num(SAFE_HEIGHT, &self.safe_height, positive);
        let fast_feed = num(FAST_FEED, &self.fast_feed, positive);
        let slow_feed = num(SLOW_FEED, &self.slow_feed, positive);
        let backoff = num(BACKOFF, &self.backoff, not_negative);
        let probe_depth = num(PROBE_DEPTH, &self.probe_depth, not_negative);
        let tip_radius = num(TIP_RADIUS, &self.tip_radius, not_negative);
        let plate_thickness = num(PLATE_THICKNESS, &self.plate_thickness, not_negative);
        let routine = match self.routine {
            RoutineKind::ZTouch => ProbeRoutine::ZTouch {
                safe_height,
                max_depth: num(MAX_DEPTH, &self.max_depth, positive),
                fast_feed,
                slow_feed,
                backoff,
            },
            RoutineKind::EdgeFind => ProbeRoutine::EdgeFind {
                axis: self.edge_axis.unwrap_or(ProbeAxis::X),
                direction: self.edge_direction.unwrap_or(ProbeDirection::Negative),
                safe_height,
                probe_distance: num(EDGE_DISTANCE, &self.edge_distance, positive),
                fast_feed,
                slow_feed,
                backoff,
            },
            RoutineKind::CornerFind => ProbeRoutine::CornerFind {
                corner: self.corner.unwrap_or(Corner::XminYmin),
                safe_height,
                probe_distance: num(EDGE_DISTANCE, &self.edge_distance, positive),
                fast_feed,
                slow_feed,
                backoff,
            },
            RoutineKind::BoreCenter => ProbeRoutine::BoreCenter {
                diameter: num(DIAMETER, &self.diameter, positive),
                safe_height,
                fast_feed,
                slow_feed,
            },
            RoutineKind::BossCenter => ProbeRoutine::BossCenter {
                diameter: num(DIAMETER, &self.diameter, positive),
                safe_height,
                fast_feed,
                slow_feed,
            },
            RoutineKind::ToolLength => ProbeRoutine::ToolLength {
                plate_xy: (
                    num(SETTER_X, &self.setter_x, any),
                    num(SETTER_Y, &self.setter_y, any),
                ),
                plate_z: num(SETTER_Z, &self.setter_z, any),
                safe_height,
                max_depth: num(MAX_DEPTH, &self.max_depth, positive),
                feed_rate: fast_feed,
            },
        };
        if !errors.is_empty() {
            return Err(errors);
        }
        let options = ProbeOptions {
            probe_depth,
            tip_radius,
            plate_thickness,
            set_wcs: self.auto_update.then_some(self.wcs),
        };
        Ok((routine, options))
    }

    /// The routine's program (GTK's Generate G-code), with `start` written into its header.
    ///
    /// # Arguments
    /// * `start` - The machine position in mm (the routines move relative to the probe).
    ///
    /// # Errors
    /// [`CamError::Invalid`] while the input has problems, else the engine's refusal.
    pub fn generate(&self, start: Position) -> Result<ProbeRoutineOutput, CamError> {
        let (routine, options) = self.parameters().map_err(|_| CamError::Invalid)?;
        ProbeRoutineEngine::with_options(routine, options)
            .generate(start)
            .map_err(CamError::generator)
    }

    /// The lines a run streams: the program without comments or empty lines.
    ///
    /// # Arguments
    /// * `start` - The machine position in mm.
    ///
    /// # Errors
    /// As [`ProbeParams::generate`], or a line too long for the controller.
    pub fn run_lines(&self, start: Position) -> Result<Vec<String>, CamError> {
        let output = self.generate(start)?;
        run::program_lines(&output.gcode).map_err(CamError::Generator)
    }

    /// The page's input as GTK's preset file (`ProbePreset`, values in mm).
    ///
    /// # Arguments
    /// * `tr` - Translator for the current UI language.
    ///
    /// # Errors
    /// The first field that is not a number (GTK refused to save then too).
    pub fn preset_json(&self, tr: &Translator) -> Result<String, String> {
        let preset = (|| {
            let n = |label, text: &str| common::preset_number(label, text);
            Ok(ProbePreset {
                routine: Some(self.routine.id().to_string()),
                safe_height: n(SAFE_HEIGHT, &self.safe_height)?,
                fast_feed: n(FAST_FEED, &self.fast_feed)?,
                slow_feed: n(SLOW_FEED, &self.slow_feed)?,
                backoff: n(BACKOFF, &self.backoff)?,
                max_depth: n(MAX_DEPTH, &self.max_depth)?,
                edge_axis: id_of(&AXES, self.edge_axis),
                edge_direction: id_of(&DIRECTIONS, self.edge_direction),
                edge_distance: n(EDGE_DISTANCE, &self.edge_distance)?,
                corner: id_of(&CORNERS, self.corner),
                diameter: n(DIAMETER, &self.diameter)?,
                setter_x: n(SETTER_X, &self.setter_x)?,
                setter_y: n(SETTER_Y, &self.setter_y)?,
                setter_z: n(SETTER_Z, &self.setter_z)?,
                probe_depth: n(PROBE_DEPTH, &self.probe_depth)?,
                tip_radius: n(TIP_RADIUS, &self.tip_radius)?,
                plate_thickness: n(PLATE_THICKNESS, &self.plate_thickness)?,
                target_wcs: Some(format!("G{}", self.wcs)),
                auto_update_wcs: self.auto_update,
                ..ProbePreset::default()
            })
        })();
        let preset = preset.map_err(|e: FieldError| {
            let field = e.field.map_or_else(String::new, |f| tr.tr(f).into_owned());
            format!("{field} {}", tr.tr(e.message))
        })?;
        preset.to_json().map_err(|e| e.to_string())
    }

    /// Sets the page from a preset file in GTK's format (values written with three
    /// decimals, as GTK did); ids GTK did not know leave their choice as it is.
    ///
    /// # Arguments
    /// * `json` - The file's text.
    ///
    /// # Errors
    /// The text is not a probe preset; the page is left as it was.
    pub fn apply_preset(&mut self, json: &str) -> Result<(), String> {
        // GTK read any JSON value as an old preset of defaults; only an object can be one.
        common::preset_object(json)?;
        let loaded = ProbePreset::from_json(json).map_err(|e| e.to_string())?;
        for warning in &loaded.warnings {
            tracing::warn!(%warning, "probe preset loaded with a warning");
        }
        let p = loaded.preset;
        let mm = |v: f64| format!("{v:.3}");
        if let Some(kind) = p.routine.as_deref().and_then(RoutineKind::from_id) {
            self.routine = kind;
        }
        self.safe_height = mm(p.safe_height);
        self.fast_feed = mm(p.fast_feed);
        self.slow_feed = mm(p.slow_feed);
        self.backoff = mm(p.backoff);
        self.max_depth = mm(p.max_depth);
        if let Some(v) = from_id(&AXES, p.edge_axis.as_deref()) {
            self.edge_axis = Some(v);
        }
        if let Some(v) = from_id(&DIRECTIONS, p.edge_direction.as_deref()) {
            self.edge_direction = Some(v);
        }
        self.edge_distance = mm(p.edge_distance);
        if let Some(v) = from_id(&CORNERS, p.corner.as_deref()) {
            self.corner = Some(v);
        }
        self.diameter = mm(p.diameter);
        self.setter_x = mm(p.setter_x);
        self.setter_y = mm(p.setter_y);
        self.setter_z = mm(p.setter_z);
        self.probe_depth = mm(p.probe_depth);
        self.tip_radius = mm(p.tip_radius);
        self.plate_thickness = mm(p.plate_thickness);
        if let Some(wcs) = p
            .target_wcs
            .as_deref()
            .and_then(|id| id.trim_start_matches('G').parse::<u8>().ok())
            .filter(|w| (54..=59).contains(w))
        {
            self.wcs = wcs;
        }
        self.auto_update = p.auto_update_wcs;
        Ok(())
    }
}
