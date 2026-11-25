//! The window frame: the app's own title bar and resize edges, as `craft_window` decides.
//!
//! Every decision comes from `craft_window`: whether the app draws the frame at all
//! ([`craft_window::policy::Decision::frame`]), which caption buttons appear and on which
//! side in which order ([`craft_window::Layout`]), which look they take
//! ([`craft_window::policy::CaptionFamily`]), and the strip's drag-to-move and double-click
//! behaviour ([`craft_window::title_bar::empty_area`]). Drawing the buttons stays with the
//! app, as craft-window documents; so does resizing an undecorated window from its edges,
//! which craft-window does not offer.
//!
//! Dependencies: `egui`, `craft-window`, `gcodekit5-app` (translations), `gcodekit5-settings`.

use craft_window::Button;
use craft_window::policy::{CaptionFamily, Captions, Frame, TitleBarPref};
use egui::{Color32, CornerRadius, Rect, Sense, Stroke, Vec2, vec2};
use gcodekit5_app::i18n::Translator;
use gcodekit5_settings::config::UiSettings;

/// Height of the app-drawn title bar, in points.
pub const TITLE_BAR_HEIGHT: f32 = 32.0;
/// Width of the band along the window edge that resizes an undecorated window, in points.
pub const RESIZE_BAND: f32 = 5.0;

/// The frame preference the user's "Use system title bar" setting asks for.
///
/// # Arguments
/// * `ui` - The UI settings.
pub fn title_bar_pref(ui: &UiSettings) -> TitleBarPref {
    if ui.use_system_title_bar {
        TitleBarPref::System
    } else {
        TitleBarPref::Integrated
    }
}

/// The English msgid naming a caption button (its accessible label).
///
/// # Arguments
/// * `button` - The caption button.
pub fn caption_msgid(button: Button) -> &'static str {
    match button {
        Button::Minimize => n_!("Minimize"),
        Button::Maximize => n_!("Maximize"),
        Button::Close => n_!("Close"),
    }
}

/// How one family's caption buttons look: the only distinctions are the ones
/// [`CaptionFamily`] names, not a desktop theme.
#[derive(Debug, Clone, Copy)]
struct CaptionStyle {
    /// Room each button takes in the strip.
    slot: Vec2,
    /// Diameter of a round button; `None` for a flat button filling its slot.
    round: Option<f32>,
    /// Whether the button shows its fill when not hovered.
    always_filled: bool,
    /// Fill of a hovered close button, where the family marks it.
    close_hover: Option<Color32>,
}

/// The caption look for a family: round Adwaita and Breeze buttons, flat Windows and
/// generic buttons filling the strip's height.
fn caption_style(family: CaptionFamily) -> CaptionStyle {
    let red = Color32::from_rgb(0xc4, 0x2b, 0x1c);
    match family {
        CaptionFamily::Adwaita => CaptionStyle {
            slot: vec2(TITLE_BAR_HEIGHT, TITLE_BAR_HEIGHT),
            round: Some(24.0),
            always_filled: true,
            close_hover: None,
        },
        CaptionFamily::Breeze => CaptionStyle {
            slot: vec2(TITLE_BAR_HEIGHT - 4.0, TITLE_BAR_HEIGHT),
            round: Some(18.0),
            always_filled: false,
            close_hover: Some(red),
        },
        CaptionFamily::Windows => CaptionStyle {
            slot: vec2(46.0, TITLE_BAR_HEIGHT),
            round: None,
            always_filled: false,
            close_hover: Some(red),
        },
        CaptionFamily::Generic => CaptionStyle {
            slot: vec2(TITLE_BAR_HEIGHT + 4.0, TITLE_BAR_HEIGHT),
            round: None,
            always_filled: false,
            close_hover: None,
        },
    }
}

/// Draws the frame `frame` asks of the app: nothing for [`Frame::Native`], the strip with
/// room for the traffic lights for [`Frame::Inset`], the strip with caption buttons and the
/// resize edges for [`Frame::Integrated`]. Call it before the rest of the window's UI.
///
/// # Arguments
/// * `ui` - The window's root `Ui`.
/// * `tr` - Translator for the button labels.
/// * `frame` - The decided frame.
pub fn show(ui: &mut egui::Ui, tr: &Translator, frame: &Frame) {
    match frame {
        Frame::Native => {}
        Frame::Inset(inset) => title_bar(ui, tr, inset.leading, None, 0.0),
        Frame::Integrated(captions) => {
            resize_edges(ui);
            title_bar(ui, tr, 0.0, Some(captions), RESIZE_BAND);
        }
    }
}

/// The title-bar strip: `leading` points left free, the application name, and the caption
/// buttons in `captions`' layout, each side in the layout's (reading) order. The top
/// `resize_band` points are left to the resize edge, so a drag there resizes rather than moves.
fn title_bar(
    ui: &mut egui::Ui,
    tr: &Translator,
    leading: f32,
    captions: Option<&Captions>,
    resize_band: f32,
) {
    let fill = ui.visuals().faint_bg_color;
    egui::Panel::top("gcodekit5-title-bar")
        .exact_size(TITLE_BAR_HEIGHT)
        .frame(egui::Frame::NONE.fill(fill))
        .show(ui, |ui| {
            let mut strip = ui.max_rect();
            strip.min.y += resize_band;
            craft_window::title_bar::empty_area(ui, strip);
            ui.spacing_mut().item_spacing.x = 0.0;
            ui.horizontal_centered(|ui| {
                ui.add_space(leading);
                if let Some(captions) = captions {
                    for button in &captions.layout.left {
                        caption_button(ui, tr, captions.family, *button);
                    }
                }
                ui.add_space(8.0);
                ui.label("GCodeKit");
                if let Some(captions) = captions {
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        // Right to left, so the last button in reading order comes first.
                        for button in captions.layout.right.iter().rev() {
                            caption_button(ui, tr, captions.family, *button);
                        }
                    });
                }
            });
        });
}

/// One caption button in `family`'s look; clicking it sends its window command.
fn caption_button(ui: &mut egui::Ui, tr: &Translator, family: CaptionFamily, button: Button) {
    let style = caption_style(family);
    let (slot, response) = ui.allocate_exact_size(style.slot, Sense::click());
    let label = tr.tr(caption_msgid(button));
    response.widget_info(|| egui::WidgetInfo::labeled(egui::Role::Button, true, &label));
    let visuals = ui.style().interact(&response);
    let hovered = response.hovered() || response.is_pointer_button_down_on();
    let red = style
        .close_hover
        .filter(|_| hovered && button == Button::Close);
    let painter = ui.painter_at(slot);
    if hovered || style.always_filled {
        let (shape, radius) = match style.round {
            Some(d) => (
                Rect::from_center_size(slot.center(), vec2(d, d)),
                CornerRadius::from(d / 2.0),
            ),
            None => (slot, CornerRadius::ZERO),
        };
        painter.rect_filled(shape, radius, red.unwrap_or(visuals.bg_fill));
    }
    let glyph = if red.is_some() {
        Color32::WHITE
    } else {
        visuals.fg_stroke.color
    };
    paint_glyph(&painter, slot.center(), button, Stroke::new(1.0, glyph));
    if response.clicked() {
        let maximized = ui.input(|i| i.viewport().maximized).unwrap_or(false);
        ui.ctx().send_viewport_cmd(match button {
            Button::Minimize => egui::ViewportCommand::Minimized(true),
            Button::Maximize => egui::ViewportCommand::Maximized(!maximized),
            Button::Close => egui::ViewportCommand::Close,
        });
    }
}

/// The button's line icon, 10 points across, centred on `c`.
fn paint_glyph(painter: &egui::Painter, c: egui::Pos2, button: Button, stroke: Stroke) {
    match button {
        Button::Minimize => {
            painter.line_segment([c + vec2(-5.0, 4.0), c + vec2(5.0, 4.0)], stroke);
        }
        Button::Maximize => {
            let square = Rect::from_center_size(c, vec2(10.0, 10.0));
            painter.rect_stroke(square, 0.0, stroke, egui::StrokeKind::Inside);
        }
        Button::Close => {
            painter.line_segment([c + vec2(-5.0, -5.0), c + vec2(5.0, 5.0)], stroke);
            painter.line_segment([c + vec2(-5.0, 5.0), c + vec2(5.0, -5.0)], stroke);
        }
    }
}

/// Resize areas along the edges and corners of an undecorated window.
///
/// They are registered before every other widget, so egui's hit test gives any widget drawn
/// over them (a caption button, a dock scrollbar) the pointer first; only a drag that starts
/// where no other widget takes drags resizes the window.
fn resize_edges(ui: &mut egui::Ui) {
    let still = ui.input(|i| {
        i.viewport().maximized.unwrap_or(false) || i.viewport().fullscreen.unwrap_or(false)
    });
    if still {
        return;
    }
    use egui::{CursorIcon as C, ResizeDirection as D};
    let r = ui.ctx().content_rect();
    let b = RESIZE_BAND;
    let (l, t, rt, bt) = (r.left(), r.top(), r.right(), r.bottom());
    let x = |a: f32, z: f32| egui::Rangef::new(a, z);
    let edges = [
        (D::NorthWest, C::ResizeNwSe, x(l, l + b), x(t, t + b)),
        (D::NorthEast, C::ResizeNeSw, x(rt - b, rt), x(t, t + b)),
        (D::SouthWest, C::ResizeNeSw, x(l, l + b), x(bt - b, bt)),
        (D::SouthEast, C::ResizeNwSe, x(rt - b, rt), x(bt - b, bt)),
        (D::North, C::ResizeVertical, x(l + b, rt - b), x(t, t + b)),
        (D::South, C::ResizeVertical, x(l + b, rt - b), x(bt - b, bt)),
        (D::West, C::ResizeHorizontal, x(l, l + b), x(t + b, bt - b)),
        (
            D::East,
            C::ResizeHorizontal,
            x(rt - b, rt),
            x(t + b, bt - b),
        ),
    ];
    for (index, (direction, cursor, xs, ys)) in edges.into_iter().enumerate() {
        let rect = Rect::from_x_y_ranges(xs, ys);
        let id = egui::Id::unique(("gcodekit5-resize-edge", index));
        let response = ui
            .interact(rect, id, Sense::click_and_drag())
            .on_hover_cursor(cursor);
        if response.drag_started() {
            ui.ctx()
                .send_viewport_cmd(egui::ViewportCommand::BeginResize(direction));
        }
    }
}
