//! The canvas's floating controls, as GTK's on-screen display: the view toolbar at the top
//! centre (zoom out, fit to content, fit to viewport, fit to the work area, GTK's "?"
//! keyboard-shortcut popover, zoom in) and the status line at the bottom left (zoom, view
//! centre, cursor, snap and constraint, units).
//!
//! While the design has no shapes, GTK's empty state sits in the middle of the canvas: "No
//! shapes yet", a hint, and Load Design, Import SVG, Import DXF, Import STL (when STL import
//! is on) and Import Image (disabled in CNC mode, as GTK).
//!
//! Not ported: GTK's scrollbar toggle (the canvas has no scrollbars).
//!
//! Dependencies: `egui`, `gcodekit5-core` (units), `gcodekit5-app` (translations).

use gcodekit5_app::i18n::Translator;
use gcodekit5_core::units::{self, MeasurementSystem};

use crate::commands::Command;
use crate::icons::{self, Icon};

/// The msgids the overlay shows, for the translation check.
pub fn msgids() -> Vec<&'static str> {
    vec![
        n_!("Zoom Out"),
        n_!("Fit to Content"),
        n_!("Fit to Viewport"),
        n_!("Fit to Device Working Area"),
        n_!("Zoom In"),
        n_!("Zoom"),
        n_!("Center"),
        n_!("Cursor"),
        n_!("Snap"),
        n_!("Constraint"),
        n_!("Keyboard Shortcuts"),
        n_!("Designer shortcuts"),
        n_!("Right click for context menu"),
        n_!("Undo"),
        n_!("Redo"),
        n_!("Copy"),
        n_!("Paste"),
        n_!("Duplicate"),
        n_!("Delete"),
        n_!("No shapes yet"),
        n_!("Use the toolbox on the left to start drawing."),
        n_!("Load Design"),
        n_!("Import SVG"),
        n_!("Import DXF"),
        n_!("Import STL"),
        n_!("Import Image"),
        n_!("Raster image import is only available in 2D mode"),
    ]
}

/// The "?" popover's lines, as GTK's: the default keys (craft-keymap chord text) and the
/// msgid of what each does on the canvas.
pub const SHORTCUTS: [(&[&str], &str); 10] = [
    (&["Cmd+Z"], n_!("Undo")),
    (&["Cmd+Y"], n_!("Redo")),
    (&["Cmd+C"], n_!("Copy")),
    (&["Cmd+V"], n_!("Paste")),
    (&["Cmd+D"], n_!("Duplicate")),
    (&["Delete"], n_!("Delete")),
    (&["+", "-"], n_!("Zoom")),
    (&["F"], n_!("Fit to Content")),
    (&["R"], n_!("Fit to Viewport")),
    (&["D"], n_!("Fit to Device Working Area")),
];

/// A view change asked for on the toolbar.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ViewAction {
    /// Zoom out one step.
    ZoomOut,
    /// Fit every shape into the view.
    FitContent,
    /// 100% with the origin at the bottom-left corner.
    FitViewport,
    /// Fit the work area into the view.
    FitWorkArea,
    /// Zoom in one step.
    ZoomIn,
}

/// The toolbar's buttons: action, icon and tooltip msgid, in GTK's order.
const BUTTONS: [(ViewAction, Icon, &str); 5] = [
    (ViewAction::ZoomOut, Icon::ZoomOut, n_!("Zoom Out")),
    (ViewAction::FitContent, Icon::Fit, n_!("Fit to Content")),
    (
        ViewAction::FitViewport,
        Icon::Restore,
        n_!("Fit to Viewport"),
    ),
    (
        ViewAction::FitWorkArea,
        Icon::Display,
        n_!("Fit to Device Working Area"),
    ),
    (ViewAction::ZoomIn, Icon::ZoomIn, n_!("Zoom In")),
];

/// Gap between the overlays and the canvas edge in points.
const MARGIN: f32 = 10.0;

/// Height the toolbar takes at the top of the canvas, margins included.
pub const TOOLBAR_HEIGHT: f32 = 48.0;

/// The floating frame both overlays are drawn in.
fn osd_frame(ui: &egui::Ui) -> egui::Frame {
    let visuals = ui.visuals();
    egui::Frame::new()
        .fill(visuals.window_fill.gamma_multiply(0.92))
        .stroke(visuals.window_stroke)
        .corner_radius(egui::CornerRadius::same(6))
        .inner_margin(egui::Margin::symmetric(6, 4))
}

/// The toolbar at the top centre of the canvas `rect`.
///
/// # Arguments
/// * `ui` - The canvas's `Ui`; the toolbar floats above it in its own area.
/// * `rect` - The canvas.
/// * `tr` - Translator for the current UI language.
///
/// # Returns
/// The action clicked this frame, if any.
pub fn toolbar(ui: &mut egui::Ui, rect: egui::Rect, tr: &Translator) -> Option<ViewAction> {
    let mut clicked = None;
    // An area pivoted on its top centre is centred from its own size.
    egui::Area::new(toolbar_id())
        .order(egui::Order::Middle)
        .fixed_pos(egui::pos2(rect.center().x, rect.top() + MARGIN))
        .pivot(egui::Align2::CENTER_TOP)
        .constrain_to(rect)
        .show(ui.ctx(), |ui| {
            osd_frame(ui).show(ui, |ui| {
                ui.horizontal(|ui| {
                    for (action, icon, msgid) in BUTTONS {
                        // GTK's "?" sits just before Zoom In.
                        if action == ViewAction::ZoomIn {
                            shortcuts_button(ui, tr);
                        }
                        if icons::icon_only_button(ui, icon, &tr.tr(msgid)).clicked() {
                            clicked = Some(action);
                        }
                    }
                });
            });
        });
    clicked
}

/// The text of one popover line: the keys on this platform, then what they do.
///
/// # Arguments
/// * `keys` - craft-keymap chord texts.
/// * `msgid` - What the keys do.
/// * `tr` - Translator for the current UI language.
pub fn shortcut_line(keys: &[&str], msgid: &str, tr: &Translator) -> String {
    let platform = craft_keymap::Platform::current();
    let keys: Vec<String> = keys
        .iter()
        .map(|text| match craft_keymap::Chord::parse(text) {
            Ok(chord) => chord.display(platform),
            Err(_) => (*text).to_owned(),
        })
        .collect();
    format!("{}  -  {}", keys.join(" / "), tr.tr(msgid))
}

/// GTK's "?" button: a popover listing the canvas's keys.
fn shortcuts_button(ui: &mut egui::Ui, tr: &Translator) {
    let response = ui
        .menu_button("?", |ui| {
            ui.strong(tr.tr("Designer shortcuts"));
            for (keys, msgid) in SHORTCUTS {
                ui.label(shortcut_line(keys, msgid, tr));
            }
            ui.label(tr.tr("Right click for context menu"));
        })
        .response;
    response.on_hover_text(tr.tr("Keyboard Shortcuts"));
}

/// The id of the toolbar's area: part of the canvas, not a window or popup, so it does not
/// take the keyboard from the panes (`crate::workspace::overlay_layers`).
pub fn toolbar_id() -> egui::Id {
    egui::Id::unique("gcodekit5-designer-canvas-toolbar")
}

/// What the status line shows.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Status {
    /// The zoom factor (1.0 is 100%).
    pub zoom: f64,
    /// The world point at the canvas centre, mm.
    pub center: (f64, f64),
    /// The last world point under the pointer, mm.
    pub cursor: (f64, f64),
    /// Whether snapping is on.
    pub snap: bool,
    /// Whether Shift is held, constraining the drawing or resizing (GTK's "Constraint").
    pub constraint: bool,
    /// The display units.
    pub units: MeasurementSystem,
}

/// `mm` in the display units with two decimals.
fn length(mm: f64, system: MeasurementSystem) -> String {
    let shown = match system {
        MeasurementSystem::Metric => mm,
        MeasurementSystem::Imperial => mm / 25.4,
    };
    format!("{shown:.2}")
}

/// The status line's text, as GTK's `format_zoom_center_cursor` plus the snap flag and the
/// unit label.
///
/// # Arguments
/// * `status` - The values shown.
/// * `tr` - Translator for the current UI language.
pub fn status_text(status: &Status, tr: &Translator) -> String {
    let u = status.units;
    let mut text = format!(
        "{}: {:.0}%  {}: X {} Y {}  {}: X {} Y {}",
        tr.tr("Zoom"),
        status.zoom * 100.0,
        tr.tr("Center"),
        length(status.center.0, u),
        length(status.center.1, u),
        tr.tr("Cursor"),
        length(status.cursor.0, u),
        length(status.cursor.1, u),
    );
    // GTK: "Snap", "Constraint" or "Snap / Constraint".
    let flags: Vec<String> = [(status.snap, "Snap"), (status.constraint, "Constraint")]
        .into_iter()
        .filter(|(on, _)| *on)
        .map(|(_, msgid)| tr.tr(msgid).into_owned())
        .collect();
    if !flags.is_empty() {
        text.push_str(&format!("  {}", flags.join(" / ")));
    }
    text.push_str(&format!("  {}", units::get_unit_label(u)));
    text
}

/// The status line at the bottom left of the canvas `rect`; it takes no input.
///
/// # Arguments
/// * `ui` - The canvas's `Ui`.
/// * `rect` - The canvas.
/// * `status` - The values shown.
/// * `tr` - Translator for the current UI language.
pub fn status_line(ui: &mut egui::Ui, rect: egui::Rect, status: &Status, tr: &Translator) {
    let height = 28.0;
    let strip = egui::Rect::from_min_max(
        egui::pos2(rect.left() + MARGIN, rect.bottom() - height - MARGIN),
        egui::pos2(rect.right() - MARGIN, rect.bottom() - MARGIN),
    );
    ui.scope_builder(
        egui::UiBuilder::new()
            .max_rect(strip)
            .layout(egui::Layout::left_to_right(egui::Align::Center)),
        |ui| {
            osd_frame(ui).show(ui, |ui| {
                ui.add(
                    egui::Label::new(egui::RichText::new(status_text(status, tr)).small())
                        .selectable(false)
                        .truncate(),
                );
            });
        },
    );
}

/// GTK's empty state in the middle of the canvas `rect`: a title, a hint and the open and
/// import buttons. Import SVG, DXF and STL open the import dialog, which offers each of
/// those file types (GTK opened one dialog per type).
///
/// # Arguments
/// * `ui` - The canvas's `Ui`.
/// * `rect` - The canvas.
/// * `tr` - Translator for the current UI language.
/// * `stl_import` - Whether STL import is on (Preferences), which shows Import STL.
/// * `image_import` - Whether a raster image can be imported (laser mode), else Import Image
///   is disabled and says why.
///
/// # Returns
/// The command of the button clicked this frame, if any.
pub fn empty_state(
    ui: &mut egui::Ui,
    rect: egui::Rect,
    tr: &Translator,
    stl_import: bool,
    image_import: bool,
) -> Option<Command> {
    let mut clicked = None;
    let size = egui::vec2(rect.width().min(560.0), 120.0);
    let area = egui::Rect::from_center_size(rect.center(), size);
    ui.scope_builder(
        egui::UiBuilder::new()
            .max_rect(area)
            .layout(egui::Layout::top_down(egui::Align::Center)),
        |ui| {
            osd_frame(ui).show(ui, |ui| {
                ui.vertical_centered(|ui| {
                    ui.add(egui::Label::new(tr.tr("No shapes yet")).selectable(false));
                    ui.add(
                        egui::Label::new(tr.tr("Use the toolbox on the left to start drawing."))
                            .selectable(false),
                    );
                    ui.horizontal_wrapped(|ui| {
                        let buttons = [
                            ("Load Design", Command::FileOpen, true),
                            ("Import SVG", Command::FileImport, true),
                            ("Import DXF", Command::FileImport, true),
                            ("Import STL", Command::FileImport, stl_import),
                        ];
                        for (msgid, command, shown) in buttons {
                            if shown && ui.button(tr.tr(msgid)).clicked() {
                                clicked = Some(command);
                            }
                        }
                        let image = ui
                            .add_enabled(image_import, egui::Button::new(tr.tr("Import Image")))
                            .on_disabled_hover_text(
                                tr.tr("Raster image import is only available in 2D mode"),
                            );
                        if image.clicked() {
                            clicked = Some(Command::FileImportImage);
                        }
                    });
                });
            });
        },
    );
    clicked
}
