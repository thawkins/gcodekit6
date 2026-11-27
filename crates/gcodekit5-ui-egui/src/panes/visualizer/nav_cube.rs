//! The nav cube: the 3x3 view buttons and the zoom row in the visualizer's top-right corner,
//! and under it the floating Top View and Mouse
//! Controls buttons.
//!
//! The layout: isometric corners around Top, Left, Front, Right and Bottom, then
//! Zoom Out, Fit and Zoom In. Each button's accessible name is its tooltip. The one-letter
//! view labels (T, F, L, R, B) stay untranslated; the isometric corners draw an
//! arrow toward their corner. The floating column under the cube has the
//! Top View, Fit to Device Working Area and Mouse Controls buttons; its Toggle Scrollbars is
//! left out (the egui view has no scrollbars).
//!
//! Dependencies: `egui`, `gcodekit5-app` (translations).

use gcodekit5_app::i18n::Translator;

use super::camera::StdView;
use crate::icons::Icon;

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
    /// Fit the active device's work area into the view.
    FitDevice,
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
        n_!("Fit to Device Working Area"),
        n_!("Mouse Controls"),
        n_!("Left Button+Drag: Orbit"),
        n_!("Shift+Left button+Drag: Pan"),
        n_!("Mouse wheel: Zoom+/-"),
    ]
}

/// The view buttons, row by row: label (empty for the isometric corners, which draw an
/// arrow), tooltip msgid and view.
const VIEWS: [[(&str, &str, StdView); 3]; 3] = [
    [
        ("", "Isometric (NW)", StdView::IsoNw),
        ("T", "Top View", StdView::Top),
        ("", "Isometric (NE)", StdView::IsoNe),
    ],
    [
        ("L", "Left View", StdView::Left),
        ("F", "Front View", StdView::Front),
        ("R", "Right View", StdView::Right),
    ],
    [
        ("", "Isometric (SW)", StdView::IsoSw),
        ("B", "Bottom View", StdView::Bottom),
        ("", "Isometric (SE)", StdView::IsoSe),
    ],
];

/// Side of a nav cube button in points (30 px).
const BUTTON: f32 = 30.0;
/// Gap between buttons in points.
const GAP: f32 = 2.0;
/// Gap between the grid and the column of extra buttons under it, in points.
const COLUMN_GAP: f32 = 8.0;
/// Margin from the pane's top-right corner in points.
const MARGIN: f32 = 10.0;

/// The screen direction of the arrow each isometric corner draws.
fn corner_arrow(view: StdView) -> Option<egui::Vec2> {
    match view {
        StdView::IsoNw => Some(egui::vec2(-1.0, -1.0)),
        StdView::IsoNe => Some(egui::vec2(1.0, -1.0)),
        StdView::IsoSw => Some(egui::vec2(-1.0, 1.0)),
        StdView::IsoSe => Some(egui::vec2(1.0, 1.0)),
        StdView::Top | StdView::Bottom | StdView::Front | StdView::Left | StdView::Right => None,
    }
}

/// A square button at `rect` showing `label` in small bold type, named `name` for
/// accessibility, with `name` as its tooltip.
fn button(ui: &mut egui::Ui, rect: egui::Rect, label: &str, name: &str) -> egui::Response {
    let text = egui::RichText::new(label).small().strong();
    let response = ui
        .put(rect, framed(ui, egui::Button::new(text)))
        .on_hover_text(name);
    response.widget_info(|| egui::WidgetInfo::labeled(egui::Role::Button, true, name));
    response
}

/// `button` in the OSD look: a lighter fill and a border, so it stands out on the view,
/// with its text kept on one line.
fn framed<'a>(ui: &egui::Ui, button: egui::Button<'a>) -> egui::Button<'a> {
    let visuals = &ui.visuals().widgets;
    button
        .fill(visuals.hovered.weak_bg_fill)
        .stroke(visuals.noninteractive.bg_stroke)
        .wrap_mode(egui::TextWrapMode::Extend)
}

/// Draws a small arrow pointing along `direction` in the middle of `rect`.
fn paint_arrow(ui: &egui::Ui, rect: egui::Rect, direction: egui::Vec2) {
    let dir = direction.normalized();
    let stroke = egui::Stroke::new(1.5, ui.visuals().text_color());
    ui.painter()
        .arrow(rect.center() - dir * 5.0, dir * 10.0, stroke);
}

/// The rectangle of the button in grid `row` and `col` of the nav cube placed at `origin`.
fn cell(origin: egui::Pos2, row: usize, col: usize) -> egui::Rect {
    let pitch = BUTTON + GAP;
    egui::Rect::from_min_size(
        origin + egui::vec2(col as f32 * pitch, row as f32 * pitch),
        egui::vec2(BUTTON, BUTTON),
    )
}

/// Draws the nav cube in the top-right corner of `rect`: the 3x3 view grid, the zoom row
/// and, under its right column, the floating Top View and Mouse Controls buttons.
///
/// # Arguments
/// * `ui` - The visualizer's `Ui`.
/// * `rect` - The view's rectangle.
/// * `cube` - Whether to draw the view grid and the zoom row; they are hidden in the 2D view
///   mode, leaving the floating column in place.
/// * `tr` - Translator for the current UI language.
///
/// # Returns
/// The action of the button clicked this frame, if any.
pub fn show(ui: &mut egui::Ui, rect: egui::Rect, cube: bool, tr: &Translator) -> Option<NavAction> {
    ui.scope_builder(egui::UiBuilder::new().max_rect(rect), |ui| {
        // Small padding, so no label or icon makes its button outgrow the 30-point cell.
        ui.spacing_mut().button_padding = egui::vec2(2.0, 2.0);
        let width = 3.0 * BUTTON + 2.0 * GAP;
        let origin = egui::pos2(rect.right() - MARGIN - width, rect.top() + MARGIN);
        let views = cube.then(|| show_cube(ui, origin, tr)).flatten();
        show_column(ui, origin, tr).or(views)
    })
    .inner
}

/// Draws the view grid and the zoom row with its top-left corner at `origin`.
fn show_cube(ui: &mut egui::Ui, origin: egui::Pos2, tr: &Translator) -> Option<NavAction> {
    let mut action = None;
    for (row, views) in VIEWS.into_iter().enumerate() {
        for (col, (label, msgid, view)) in views.into_iter().enumerate() {
            let at = cell(origin, row, col);
            if button(ui, at, label, &tr.tr(msgid)).clicked() {
                action = Some(NavAction::View(view));
            }
            if let Some(direction) = corner_arrow(view) {
                paint_arrow(ui, at, direction);
            }
        }
    }
    let fit = tr.tr("Fit");
    let zoom = [
        ("-", "Zoom Out", NavAction::ZoomOut),
        (fit.as_ref(), "Fit to Content", NavAction::Fit),
        ("+", "Zoom In", NavAction::ZoomIn),
    ];
    for (col, (label, msgid, act)) in zoom.into_iter().enumerate() {
        if button(ui, cell(origin, 3, col), label, &tr.tr(msgid)).clicked() {
            action = Some(act);
        }
    }
    action
}

/// Draws the floating column (Top View, Fit to Device Working Area, Mouse Controls) under
/// the right column of the nav cube placed at `origin`.
fn show_column(ui: &mut egui::Ui, origin: egui::Pos2, tr: &Translator) -> Option<NavAction> {
    let mut action = None;
    let column = origin + egui::vec2(0.0, COLUMN_GAP);
    let top = tr.tr("Top View");
    let top_rect = cell(column, 4, 2);
    let image = crate::icons::app_icon(ui, "top_view.svg", BUTTON * 0.6);
    let response = match image {
        Some(image) => ui.put(top_rect, framed(ui, egui::Button::image(image))),
        None => ui.put(top_rect, framed(ui, egui::Button::new("T"))),
    }
    .on_hover_text(top.as_ref());
    response.widget_info(|| egui::WidgetInfo::labeled(egui::Role::Button, true, top.as_ref()));
    if response.clicked() {
        action = Some(NavAction::View(StdView::Top));
    }
    let device = tr.tr("Fit to Device Working Area");
    let image = Icon::Display
        .themed(ui)
        .fit_to_exact_size(egui::Vec2::splat(BUTTON * 0.6));
    let response = ui
        .put(cell(column, 5, 2), framed(ui, egui::Button::image(image)))
        .on_hover_text(device.as_ref());
    response.widget_info(|| egui::WidgetInfo::labeled(egui::Role::Button, true, device.as_ref()));
    if response.clicked() {
        action = Some(NavAction::FitDevice);
    }
    let help = [
        tr.tr("Left Button+Drag: Orbit").into_owned(),
        tr.tr("Shift+Left button+Drag: Pan").into_owned(),
        tr.tr("Mouse wheel: Zoom+/-").into_owned(),
    ]
    .join("\n");
    let name = tr.tr("Mouse Controls").into_owned();
    let text = egui::RichText::new("?").small().strong();
    let response = ui
        .put(cell(column, 6, 2), framed(ui, egui::Button::new(text)))
        .on_hover_text(format!("{name}\n{help}"));
    response.widget_info(|| egui::WidgetInfo::labeled(egui::Role::Button, true, &name));
    action
}
