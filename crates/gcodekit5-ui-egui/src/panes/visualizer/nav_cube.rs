//! The nav cube: the 3x3 view buttons and the zoom row in the visualizer's top-right corner,
//! ported from the GTK `ui/gtk/nav_cube.rs`.
//!
//! The layout is GTK's: isometric corners around Top, Left, Front, Right and Bottom, then
//! Zoom Out, Fit and Zoom In. Each button's accessible name is its tooltip. The one-letter
//! view labels (T, F, L, R, B) stay untranslated, as in GTK; the isometric corners are
//! labelled NW, NE, SW and SE (GTK draws arrows, which the bundled fonts lack).
//!
//! Dependencies: `egui`, `gcodekit5-app` (translations).

use gcodekit5_app::i18n::Translator;

use super::camera::StdView;

/// What a nav cube button asks for.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NavAction {
    /// Turn to a standard view.
    View(StdView),
    /// Move closer.
    ZoomIn,
    /// Move away.
    ZoomOut,
    /// Fit the toolpath into the view.
    Fit,
}

/// The msgids the nav cube shows, for the translation check.
pub fn msgids() -> Vec<&'static str> {
    vec![
        n_!("Isometric (NW)"),
        n_!("Top View"),
        n_!("Isometric (NE)"),
        n_!("Left View"),
        n_!("Front View"),
        n_!("Right View"),
        n_!("Isometric (SW)"),
        n_!("Bottom View"),
        n_!("Isometric (SE)"),
        n_!("Zoom In"),
        n_!("Zoom Out"),
        n_!("Fit to Content"),
        n_!("Fit"),
    ]
}

/// The view buttons, row by row: label, tooltip msgid and view.
const VIEWS: [[(&str, &str, StdView); 3]; 3] = [
    [
        ("NW", "Isometric (NW)", StdView::IsoNw),
        ("T", "Top View", StdView::Top),
        ("NE", "Isometric (NE)", StdView::IsoNe),
    ],
    [
        ("L", "Left View", StdView::Left),
        ("F", "Front View", StdView::Front),
        ("R", "Right View", StdView::Right),
    ],
    [
        ("SW", "Isometric (SW)", StdView::IsoSw),
        ("B", "Bottom View", StdView::Bottom),
        ("SE", "Isometric (SE)", StdView::IsoSe),
    ],
];

/// Side of a nav cube button in points (GTK's 30 px).
const BUTTON: f32 = 30.0;
/// Gap between buttons in points.
const GAP: f32 = 2.0;
/// Margin from the pane's top-right corner in points.
const MARGIN: f32 = 10.0;

/// A square button named `name` for accessibility, with `name` as its tooltip.
fn button(ui: &mut egui::Ui, label: &str, name: &str) -> bool {
    let response = ui
        .add_sized([BUTTON, BUTTON], egui::Button::new(label))
        .on_hover_text(name);
    response.widget_info(|| egui::WidgetInfo::labeled(egui::Role::Button, true, name));
    response.clicked()
}

/// Draws the nav cube in the top-right corner of `rect`.
///
/// # Arguments
/// * `ui` - The visualizer's `Ui`.
/// * `rect` - The view's rectangle.
/// * `tr` - Translator for the current UI language.
///
/// # Returns
/// The action of the button clicked this frame, if any.
pub fn show(ui: &mut egui::Ui, rect: egui::Rect, tr: &Translator) -> Option<NavAction> {
    let width = 3.0 * BUTTON + 2.0 * GAP;
    let height = 4.0 * BUTTON + 3.0 * GAP + 2.0 * MARGIN;
    let area = egui::Rect::from_min_size(
        egui::pos2(rect.right() - MARGIN - width, rect.top() + MARGIN),
        egui::vec2(width, height),
    );
    let mut action = None;
    ui.scope_builder(egui::UiBuilder::new().max_rect(area), |ui| {
        ui.spacing_mut().item_spacing = egui::vec2(GAP, GAP);
        for row in VIEWS {
            ui.horizontal(|ui| {
                for (label, msgid, view) in row {
                    if button(ui, label, &tr.tr(msgid)) {
                        action = Some(NavAction::View(view));
                    }
                }
            });
        }
        ui.horizontal(|ui| {
            let fit = tr.tr("Fit");
            let zoom = [
                ("-", "Zoom Out", NavAction::ZoomOut),
                (fit.as_ref(), "Fit to Content", NavAction::Fit),
                ("+", "Zoom In", NavAction::ZoomIn),
            ];
            for (label, msgid, act) in zoom {
                if button(ui, label, &tr.tr(msgid)) {
                    action = Some(act);
                }
            }
        });
    });
    action
}
