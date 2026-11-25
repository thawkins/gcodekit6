//! The Designer's "Array..." dialog: copies the selection in a line, a grid or around a
//! centre point, as one undo step.
//!
//! Ported from the GTK `designer_canvas/array_dialog.rs` and the canvas's
//! `create_{linear,grid,circular}_array`. The canvas's context menu opens it. The fields
//! keep GTK's defaults; lengths are in the display units. Create checks the values, then adds
//! every copy with one `PasteShapes` command; copies get fresh group ids so they form their
//! own groups, and the originals and the copies end selected, as GTK. Invalid values show an
//! inline error and keep the dialog open.
//!
//! Dependencies: `egui`, `gcodekit5-designer` (model, undo commands), `gcodekit5-core`
//! (length parsing), `gcodekit5-app` (translations).

use std::collections::HashMap;

use gcodekit5_app::i18n::Translator;
use gcodekit5_core::units::{self, MeasurementSystem};
use gcodekit5_designer::DesignerState;
use gcodekit5_designer::canvas::DrawingObject;
use gcodekit5_designer::commands::{DesignerCommand, PasteShapes};
use gcodekit5_designer::model::{DesignerShape, circular_array_step};

/// Most objects one Array operation may create; larger arrays would freeze the UI thread
/// and can exhaust memory.
pub const MAX_ARRAY_OBJECTS: usize = 10_000;

/// The arrangement the dialog makes.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum ArrayKind {
    /// Copies in a line.
    #[default]
    Linear,
    /// Copies in rows and columns.
    Grid,
    /// Copies around a centre point.
    Circular,
}

impl ArrayKind {
    /// The msgid of the kind's name.
    pub fn label_msgid(self) -> &'static str {
        match self {
            Self::Linear => n_!("Linear"),
            Self::Grid => n_!("Grid"),
            Self::Circular => n_!("Circular"),
        }
    }
}

/// The array the dialog's values describe.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum ArraySpec {
    /// `count` items, each `dx`, `dy` (mm) from the previous one.
    Linear {
        /// Items, the original included.
        count: usize,
        /// X step, mm.
        dx: f64,
        /// Y step, mm.
        dy: f64,
    },
    /// `rows` x `cols` items spaced `dx`, `dy` (mm).
    Grid {
        /// Rows.
        rows: usize,
        /// Columns.
        cols: usize,
        /// Column spacing, mm.
        dx: f64,
        /// Row spacing, mm.
        dy: f64,
    },
    /// `count` items around (`cx`, `cy`) (mm) over `angle` degrees.
    Circular {
        /// Items, the original included.
        count: usize,
        /// Centre X, mm.
        cx: f64,
        /// Centre Y, mm.
        cy: f64,
        /// Total angle, degrees.
        angle: f64,
    },
}

/// The text of the dialog's fields, before validation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ArrayInput {
    /// The arrangement.
    pub kind: ArrayKind,
    /// Items (linear, circular).
    pub count: String,
    /// Rows (grid).
    pub rows: String,
    /// Columns (grid).
    pub cols: String,
    /// X spacing (linear, grid), display units.
    pub dx: String,
    /// Y spacing (linear, grid), display units.
    pub dy: String,
    /// Centre X (circular), display units.
    pub cx: String,
    /// Centre Y (circular), display units.
    pub cy: String,
    /// Total angle (circular), degrees.
    pub angle: String,
}

impl Default for ArrayInput {
    /// GTK's defaults.
    fn default() -> Self {
        Self {
            kind: ArrayKind::Linear,
            count: "3".into(),
            rows: "2".into(),
            cols: "2".into(),
            dx: "10".into(),
            dy: "0".into(),
            cx: "0".into(),
            cy: "0".into(),
            angle: "360".into(),
        }
    }
}

/// Why a field's value was refused: the msgids of the field and of the problem.
pub type FieldError = (&'static str, &'static str);

/// Parses the dialog's fields; lengths are in `system` units.
///
/// # Errors
/// The field that is not valid and why, as msgids.
pub fn parse_array_input(
    input: &ArrayInput,
    system: MeasurementSystem,
) -> Result<ArraySpec, FieldError> {
    let count = |name: &'static str, text: &str, min: usize| {
        let problem = if min >= 2 {
            n_!("Enter a whole number of 2 or more")
        } else {
            n_!("Enter a whole number of 1 or more")
        };
        text.trim()
            .parse::<usize>()
            .ok()
            .filter(|n| *n >= min)
            .ok_or((name, problem))
    };
    let length = |name: &'static str, text: &str| {
        units::parse_length(text, system)
            .ok()
            .map(f64::from)
            .filter(|v| v.is_finite())
            .ok_or((name, n_!("Enter a number")))
    };
    match input.kind {
        ArrayKind::Linear => Ok(ArraySpec::Linear {
            count: count(n_!("Count"), &input.count, 2)?,
            dx: length(n_!("Spacing X"), &input.dx)?,
            dy: length(n_!("Spacing Y"), &input.dy)?,
        }),
        ArrayKind::Grid => Ok(ArraySpec::Grid {
            rows: count(n_!("Rows"), &input.rows, 1)?,
            cols: count(n_!("Columns"), &input.cols, 1)?,
            dx: length(n_!("Spacing X"), &input.dx)?,
            dy: length(n_!("Spacing Y"), &input.dy)?,
        }),
        ArrayKind::Circular => {
            let count = count(n_!("Count"), &input.count, 2)?;
            let cx = length(n_!("Centre X"), &input.cx)?;
            let cy = length(n_!("Centre Y"), &input.cy)?;
            let angle = input
                .angle
                .trim()
                .parse::<f64>()
                .ok()
                .filter(|a| a.is_finite() && *a != 0.0)
                .ok_or((n_!("Angle"), n_!("Enter a non-zero angle")))?;
            Ok(ArraySpec::Circular {
                count,
                cx,
                cy,
                angle,
            })
        }
    }
}

/// Checks that `spec` applied to `selected` objects stays within [`MAX_ARRAY_OBJECTS`].
///
/// # Errors
/// The msgid of the message giving the limit.
pub fn check_array_size(spec: &ArraySpec, selected: usize) -> Result<(), &'static str> {
    let copies = match *spec {
        ArraySpec::Linear { count, .. } | ArraySpec::Circular { count, .. } => Some(count),
        ArraySpec::Grid { rows, cols, .. } => rows.checked_mul(cols),
    };
    match copies.and_then(|c| c.checked_mul(selected.max(1))) {
        Some(total) if total <= MAX_ARRAY_OBJECTS => Ok(()),
        _ => Err(n_!("The array would create more than 10000 objects")),
    }
}

/// Copies the selection as `spec` says, as one undo step; nothing happens without a
/// selection.
///
/// # Arguments
/// * `state` - The design.
/// * `spec` - The arrangement.
pub fn apply_array(state: &mut DesignerState, spec: &ArraySpec) {
    let selected: Vec<DrawingObject> = state
        .canvas
        .shapes()
        .filter(|s| s.selected)
        .cloned()
        .collect();
    if selected.is_empty() {
        return;
    }
    // One placement per copy of the selection: an offset, or a rotation about a point.
    let placements: Vec<Placement> = match *spec {
        ArraySpec::Linear { count, dx, dy } => (1..count)
            .map(|i| Placement::Move(dx * i as f64, dy * i as f64))
            .collect(),
        ArraySpec::Grid { rows, cols, dx, dy } => (0..rows)
            .flat_map(|r| (0..cols).map(move |c| (r, c)))
            .skip(1)
            .map(|(r, c)| Placement::Move(dx * c as f64, dy * r as f64))
            .collect(),
        ArraySpec::Circular {
            count,
            cx,
            cy,
            angle,
        } => {
            let step = circular_array_step(angle, count);
            (1..count)
                .map(|i| Placement::Rotate(step * i as f64, cx, cy))
                .collect()
        }
    };
    let mut new_objects = Vec::new();
    for placement in placements {
        let mut copies: Vec<DrawingObject> = selected
            .iter()
            .map(|obj| {
                let mut copy = obj.clone();
                copy.id = state.canvas.generate_id();
                match placement {
                    Placement::Move(dx, dy) => copy.shape.translate(dx, dy),
                    Placement::Rotate(angle, cx, cy) => copy.shape.rotate(angle, cx, cy),
                }
                copy.selected = true;
                copy
            })
            .collect();
        fresh_group_ids(state, &mut copies);
        new_objects.extend(copies);
    }
    if !new_objects.is_empty() {
        state.push_command(DesignerCommand::PasteShapes(PasteShapes {
            ids: new_objects.iter().map(|o| o.id).collect(),
            objects: new_objects.into_iter().map(Some).collect(),
        }));
    }
}

/// Where one copy goes.
#[derive(Clone, Copy, Debug)]
enum Placement {
    /// Moved by (dx, dy) mm.
    Move(f64, f64),
    /// Rotated by degrees about (cx, cy) mm.
    Rotate(f64, f64, f64),
}

/// Gives copied objects fresh group ids, so copies form their own groups instead of joining
/// (and being selected with) the originals' groups.
fn fresh_group_ids(state: &mut DesignerState, objects: &mut [DrawingObject]) {
    let mut map: HashMap<u64, u64> = HashMap::new();
    for obj in objects.iter_mut() {
        if let Some(gid) = obj.group_id {
            obj.group_id = Some(*map.entry(gid).or_insert_with(|| state.canvas.generate_id()));
        }
    }
}

/// The msgids the dialog shows, for the translation check.
pub fn msgids() -> Vec<&'static str> {
    vec![
        n_!("Array..."),
        n_!("Array"),
        n_!("Linear"),
        n_!("Grid"),
        n_!("Circular"),
        n_!("Count"),
        n_!("Rows"),
        n_!("Columns"),
        n_!("Spacing X"),
        n_!("Spacing Y"),
        n_!("Centre X"),
        n_!("Centre Y"),
        n_!("Angle"),
        n_!("Cancel"),
        n_!("Create"),
        n_!("Enter a whole number of 2 or more"),
        n_!("Enter a whole number of 1 or more"),
        n_!("Enter a number"),
        n_!("Enter a non-zero angle"),
        n_!("The array would create more than 10000 objects"),
    ]
}

/// The Array dialog: open or not, its field texts and the last error.
#[derive(Clone, Debug, Default)]
pub struct ArrayDialog {
    open: bool,
    input: ArrayInput,
    /// The translated error under the fields, after a refused Create.
    error: Option<String>,
}

impl ArrayDialog {
    /// Opens the dialog with the values used last (GTK's defaults the first time).
    pub fn open(&mut self) {
        self.open = true;
        self.error = None;
    }

    /// Whether the dialog is open.
    pub fn is_open(&self) -> bool {
        self.open
    }

    /// Shows the dialog while it is open; Create copies the selection of `state`.
    ///
    /// # Arguments
    /// * `ctx` - The egui context (the dialog is modal).
    /// * `state` - The design.
    /// * `system` - The display units of the lengths.
    /// * `tr` - Translator for the current UI language.
    pub fn show(
        &mut self,
        ctx: &egui::Context,
        state: &mut DesignerState,
        system: MeasurementSystem,
        tr: &Translator,
    ) {
        if !self.open {
            return;
        }
        let (mut create, mut cancel) = (false, false);
        let response =
            egui::Modal::new(egui::Id::unique("gcodekit5-designer-array")).show(ctx, |ui| {
                ui.heading(tr.tr("Array"));
                ui.horizontal(|ui| {
                    for kind in [ArrayKind::Linear, ArrayKind::Grid, ArrayKind::Circular] {
                        ui.selectable_value(&mut self.input.kind, kind, tr.tr(kind.label_msgid()));
                    }
                });
                let enter = self.fields(ui, system, tr);
                if let Some(error) = &self.error {
                    ui.colored_label(ui.visuals().error_fg_color, error);
                }
                ui.horizontal(|ui| {
                    cancel = ui.button(tr.tr("Cancel")).clicked();
                    create = ui.button(tr.tr("Create")).clicked() || enter;
                });
            });
        if create {
            self.create(state, system, tr);
        } else if cancel || response.should_close() {
            self.open = false;
        }
    }

    /// The fields the chosen arrangement uses, as GTK shows them.
    ///
    /// # Returns
    /// Whether Enter was pressed in a field (GTK's entries activate Create).
    fn fields(&mut self, ui: &mut egui::Ui, system: MeasurementSystem, tr: &Translator) -> bool {
        let unit = units::get_unit_label(system);
        let kind = self.input.kind;
        let input = &mut self.input;
        let mut enter = false;
        egui::Grid::new("gcodekit5-array-grid")
            .num_columns(3)
            .show(ui, |ui| {
                let mut row = |ui: &mut egui::Ui, label: &str, text: &mut String, suffix: &str| {
                    let label = ui.label(tr.tr(label));
                    let field = ui
                        .add(egui::TextEdit::singleline(text).desired_width(80.0))
                        .labelled_by(label.id);
                    enter |= field.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter));
                    ui.label(suffix);
                    ui.end_row();
                };
                if kind != ArrayKind::Grid {
                    row(ui, n_!("Count"), &mut input.count, "");
                }
                if kind == ArrayKind::Grid {
                    row(ui, n_!("Rows"), &mut input.rows, "");
                    row(ui, n_!("Columns"), &mut input.cols, "");
                }
                if kind != ArrayKind::Circular {
                    row(ui, n_!("Spacing X"), &mut input.dx, unit);
                    row(ui, n_!("Spacing Y"), &mut input.dy, unit);
                } else {
                    row(ui, n_!("Centre X"), &mut input.cx, unit);
                    row(ui, n_!("Centre Y"), &mut input.cy, unit);
                    row(ui, n_!("Angle"), &mut input.angle, "°");
                }
            });
        enter
    }

    /// Checks the values and makes the array, closing the dialog; a refused value leaves it
    /// open with the error.
    fn create(&mut self, state: &mut DesignerState, system: MeasurementSystem, tr: &Translator) {
        let selected = state.selected_count();
        let checked = parse_array_input(&self.input, system)
            .map_err(|(field, problem)| format!("{}: {}", tr.tr(field), tr.tr(problem)))
            .and_then(|spec| {
                check_array_size(&spec, selected)
                    .map(|()| spec)
                    .map_err(|problem| tr.tr(problem).into_owned())
            });
        match checked {
            Ok(spec) => {
                apply_array(state, &spec);
                self.open = false;
                self.error = None;
            }
            Err(message) => self.error = Some(message),
        }
    }
}
