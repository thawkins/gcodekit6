//! Colours and egui visuals of the egui front end.
//!
//! The light and dark [`Palette`]s carry the libadwaita named colours that
//! the original stylesheet referred to (`@window_bg_color`, `@accent_color`,
//! `@success_color`, ...); the status colours are the values
//! the original front end hard-coded. The fixed constants are the
//! original rendering colours and stylesheet literals.
//!
//! Which palette applies is [`resolve`]d from the user's choice in
//! [`gcodekit5_settings::config::UiSettings::theme`] and the desktop's preference.
//!
//! Dependencies: `egui`, `craft-ui` (widget palette), `gcodekit5-settings` (theme choice).

use egui::{Color32, CornerRadius, FontId, Margin, RichText, Stroke, TextStyle, Visuals};
use gcodekit5_settings::config::Theme;

/// The theme colours for one light/dark mode.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Palette {
    /// `@window_bg_color`: window background.
    pub window_bg: Color32,
    /// `@window_fg_color`: window text.
    pub window_fg: Color32,
    /// `@view_bg_color`: background of text views and lists.
    pub view_bg: Color32,
    /// `@headerbar_bg_color`: title bar background.
    pub headerbar_bg: Color32,
    /// `@card_bg_color`: background of raised cards and popups.
    pub card_bg: Color32,
    /// `@borders`: separator and frame lines.
    pub borders: Color32,
    /// `@accent_color`: accent readable as text or lines.
    pub accent: Color32,
    /// `@accent_bg_color`: accent used as a fill.
    pub accent_bg: Color32,
    /// `@success_color`.
    pub success: Color32,
    /// `@warning_color`.
    pub warning: Color32,
    /// `@error_color`.
    pub error: Color32,
    /// Widget fills for egui's interaction states.
    pub widgets: craft_ui::theme::WidgetPalette,
    /// G-code syntax colours of the editor.
    pub syntax: Syntax,
}

/// G-code syntax colours of the editor, one per highlight class.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Syntax {
    /// `G` and `M` words (`G1`, `M3`).
    pub command: Color32,
    /// Coordinates and arc parameters (`X10`, `I-2.5`, `R5`).
    pub coordinate: Color32,
    /// Line numbers (`N10`).
    pub line_number: Color32,
    /// Comments (`; ...`, `( ... )`).
    pub comment: Color32,
    /// Everything else: other words (`F`, `S`, `T`) and spaces.
    pub plain: Color32,
    /// Line numbers in the gutter.
    pub gutter: Color32,
    /// Background of the current line (the cursor's or the streaming job's).
    pub current_line_bg: Color32,
}

/// The light palette (libadwaita light).
pub const LIGHT: Palette = Palette {
    window_bg: Color32::from_rgb(0xfa, 0xfa, 0xfa),
    window_fg: Color32::from_rgb(0x32, 0x32, 0x32),
    view_bg: Color32::from_rgb(0xff, 0xff, 0xff),
    headerbar_bg: Color32::from_rgb(0xeb, 0xeb, 0xeb),
    card_bg: Color32::from_rgb(0xff, 0xff, 0xff),
    borders: Color32::from_rgb(0xd5, 0xd5, 0xd5),
    accent: Color32::from_rgb(0x1c, 0x71, 0xd8),
    accent_bg: Color32::from_rgb(0x35, 0x84, 0xe4),
    success: Color32::from_rgb(0x00, 0x7c, 0x3d),
    warning: Color32::from_rgb(0x90, 0x54, 0x00),
    error: Color32::from_rgb(0xc3, 0x00, 0x00),
    widgets: craft_ui::theme::WidgetPalette {
        noninteractive: Color32::from_rgb(0xfa, 0xfa, 0xfa),
        inactive: Color32::from_rgb(0xe8, 0xe8, 0xe8),
        hovered: Color32::from_rgb(0xdc, 0xdc, 0xdc),
        active: Color32::from_rgb(0xcc, 0xcc, 0xcc),
        open: Color32::from_rgb(0xe0, 0xe0, 0xe0),
    },
    syntax: Syntax {
        command: Color32::from_rgb(0x1c, 0x71, 0xd8),
        coordinate: Color32::from_rgb(0x26, 0xa2, 0x69),
        line_number: Color32::from_rgb(0x91, 0x41, 0xac),
        comment: Color32::from_rgb(0x77, 0x76, 0x7b),
        plain: Color32::from_rgb(0x32, 0x32, 0x32),
        gutter: Color32::from_rgb(0x9a, 0x99, 0x96),
        current_line_bg: Color32::from_rgba_unmultiplied_const(0x35, 0x84, 0xe4, 40),
    },
};

/// The dark palette (libadwaita dark).
pub const DARK: Palette = Palette {
    window_bg: Color32::from_rgb(0x24, 0x24, 0x24),
    window_fg: Color32::from_rgb(0xff, 0xff, 0xff),
    view_bg: Color32::from_rgb(0x1e, 0x1e, 0x1e),
    headerbar_bg: Color32::from_rgb(0x30, 0x30, 0x30),
    card_bg: Color32::from_rgb(0x38, 0x38, 0x38),
    borders: Color32::from_rgb(0x44, 0x44, 0x44),
    accent: Color32::from_rgb(0x78, 0xae, 0xed),
    accent_bg: Color32::from_rgb(0x35, 0x84, 0xe4),
    success: Color32::from_rgb(0x78, 0xe9, 0xab),
    warning: Color32::from_rgb(0xff, 0xc2, 0x52),
    error: Color32::from_rgb(0xff, 0x93, 0x8c),
    widgets: craft_ui::theme::WidgetPalette {
        noninteractive: Color32::from_gray(32),
        inactive: Color32::from_gray(44),
        hovered: Color32::from_gray(60),
        active: Color32::from_gray(76),
        open: Color32::from_gray(52),
    },
    syntax: Syntax {
        command: Color32::from_rgb(0x78, 0xae, 0xed),
        coordinate: Color32::from_rgb(0x8f, 0xf0, 0xa4),
        line_number: Color32::from_rgb(0xdc, 0x8a, 0xdd),
        comment: Color32::from_rgb(0x9a, 0x99, 0x96),
        plain: Color32::from_rgb(0xff, 0xff, 0xff),
        gutter: Color32::from_rgb(0x77, 0x76, 0x7b),
        current_line_bg: Color32::from_rgba_unmultiplied_const(0x35, 0x84, 0xe4, 56),
    },
};

/// X axis colour (red).
pub const AXIS_X: Color32 = Color32::from_rgb(255, 0, 0);
/// Y axis colour (green).
pub const AXIS_Y: Color32 = Color32::from_rgb(0, 255, 0);
/// Z axis colour (blue).
pub const AXIS_Z: Color32 = Color32::from_rgb(0, 0, 255);
/// Device/machine bounds (blue).
pub const DEVICE_BOUNDS: Color32 = Color32::from_rgb(0, 0, 255);
/// Major grid lines (50% grey).
pub const GRID_MAJOR: Color32 = Color32::from_rgb(128, 128, 128);
/// Minor grid lines (70% grey).
pub const GRID_MINOR: Color32 = Color32::from_rgb(179, 179, 179);
/// Selected shape (red).
pub const SELECTION: Color32 = Color32::from_rgb(255, 0, 0);
/// One of several selected shapes (yellow).
pub const SELECTION_MULTI: Color32 = Color32::from_rgb(255, 255, 0);
/// Preview outline (yellow).
pub const PREVIEW: Color32 = Color32::from_rgb(255, 255, 0);
/// Rapid moves (yellow, half transparent).
pub const TOOLPATH_RAPID: Color32 = Color32::from_rgba_unmultiplied_const(255, 204, 0, 128);
/// Cutting moves (green).
pub const TOOLPATH_CUT: Color32 = Color32::from_rgb(0, 204, 0);
/// Arc moves (green, 70% opaque).
pub const TOOLPATH_ARC: Color32 = Color32::from_rgba_unmultiplied_const(0, 204, 0, 179);
/// Dark canvas background (15% grey).
pub const BACKGROUND_DARK: Color32 = Color32::from_rgb(38, 38, 38);
/// Light canvas background (white).
pub const BACKGROUND_LIGHT: Color32 = Color32::WHITE;
/// The "active" badge (`.active-badge`).
pub const ACTIVE_BADGE: Color32 = Color32::from_rgb(0x2e, 0xcc, 0x71);
/// The About dialog title (`label.gk-about-title`).
pub const ABOUT_TITLE: Color32 = Color32::from_rgb(0xff, 0xeb, 0x3b);
/// Hovered empty-state action (`.empty-actions:hover`, 30% of `#1a73e8`).
pub const EMPTY_ACTION_HOVER: Color32 = Color32::from_rgba_unmultiplied_const(0x1a, 0x73, 0xe8, 77);
/// Vector preview background (`frame.vector-preview`).
pub const VECTOR_PREVIEW_BG: Color32 = Color32::from_rgb(0x80, 0x80, 0x80);

/// Resolves the light/dark mode to show.
///
/// # Arguments
/// * `choice` - The user's choice; `Light` and `Dark` override the desktop.
/// * `desktop` - The desktop's preference, when known.
///
/// # Returns
/// The forced mode, else the desktop's, else dark (the default look).
pub fn resolve(choice: Theme, desktop: Option<egui::Theme>) -> egui::Theme {
    match choice {
        Theme::Light => egui::Theme::Light,
        Theme::Dark => egui::Theme::Dark,
        Theme::System => desktop.unwrap_or(egui::Theme::Dark),
    }
}

/// The palette for a light/dark mode.
///
/// # Arguments
/// * `mode` - Light or dark.
pub fn palette(mode: egui::Theme) -> &'static Palette {
    match mode {
        egui::Theme::Light => &LIGHT,
        egui::Theme::Dark => &DARK,
    }
}

/// egui visuals built from the palette of `mode`.
///
/// # Arguments
/// * `mode` - Light or dark.
pub fn visuals(mode: egui::Theme) -> Visuals {
    let p = palette(mode);
    let mut visuals = match mode {
        egui::Theme::Light => Visuals::light(),
        egui::Theme::Dark => Visuals::dark(),
    };
    p.widgets.apply_to(
        &mut visuals.widgets,
        CornerRadius::same(WIDGET_RADIUS),
        Stroke::new(1.0, p.window_fg),
    );
    visuals.widgets.noninteractive.bg_stroke = Stroke::new(1.0, p.borders);
    visuals.panel_fill = p.window_bg;
    visuals.window_fill = p.card_bg;
    visuals.extreme_bg_color = match mode {
        egui::Theme::Light => Color32::from_gray(0xf0),
        egui::Theme::Dark => Color32::from_gray(0x2a),
    };
    visuals.faint_bg_color = p.headerbar_bg;
    visuals.window_stroke = Stroke::new(1.0, p.borders);
    visuals.hyperlink_color = p.accent;
    visuals.selection.bg_fill = p.accent_bg;
    visuals.selection.stroke = Stroke::new(1.0, Color32::WHITE);
    visuals.warn_fg_color = p.warning;
    visuals.error_fg_color = p.error;
    visuals
}

/// The libadwaita accent blue (`#3584e4`), the fill of primary buttons and checked boxes.
pub const ACCENT_BLUE: Color32 = Color32::from_rgb(0x35, 0x84, 0xe4);
/// Outer margin of a page, in points (12 px).
pub const PAGE_MARGIN: i8 = 12;
/// Corner radius of a button, text field and other widget (libadwaita: 6 px).
const WIDGET_RADIUS: u8 = 6;
/// Corner radius of a framed group (libadwaita cards: 8 px).
const GROUP_RADIUS: u8 = 8;

/// The fill of a destructive button (Stop, Zero All): libadwaita's red.
///
/// # Arguments
/// * `mode` - Light or dark.
pub fn destructive_fill(mode: egui::Theme) -> Color32 {
    match mode {
        egui::Theme::Light => Color32::from_rgb(0xe0, 0x1b, 0x24),
        egui::Theme::Dark => Color32::from_rgb(0xc0, 0x1c, 0x28),
    }
}

/// The egui style (visuals, spacing and text sizes) of `mode`, ported from libadwaita.
///
/// Widgets get 6 px corners and a 6 px row gap, windows a 12 px margin, text-edit fields a
/// raised fill like libadwaita entries, and the body text 14 px.
///
/// # Arguments
/// * `mode` - Light or dark.
pub fn style(mode: egui::Theme) -> egui::Style {
    let mut style = egui::Style::default();
    apply_style(&mut style, mode);
    style
}

/// Applies the theme of `mode` to `style`, keeping the settings it does not own (such as
/// the animation time a test harness zeroes).
///
/// # Arguments
/// * `style` - The style to change.
/// * `mode` - Light or dark.
pub fn apply_style(style: &mut egui::Style, mode: egui::Theme) {
    let p = palette(mode);
    style.visuals = visuals(mode);
    let spacing = &mut style.spacing;
    spacing.item_spacing = egui::vec2(8.0, 6.0);
    spacing.button_padding = egui::vec2(10.0, 5.0);
    spacing.interact_size.y = 28.0;
    spacing.window_margin = Margin::same(PAGE_MARGIN);
    spacing.menu_margin = Margin::same(6);
    spacing.indent = 18.0;
    // Rows are taller than egui's, so the default 200 px would show fewer entries.
    spacing.combo_height = 300.0;
    spacing.icon_width = 18.0;
    spacing.icon_width_inner = 10.0;
    spacing.icon_spacing = 6.0;
    let sizes = [
        (TextStyle::Heading, 18.0),
        (TextStyle::Body, 14.0),
        (TextStyle::Button, 14.0),
        (TextStyle::Small, 12.0),
        (TextStyle::Monospace, 13.0),
    ];
    for (text_style, size) in sizes {
        if let Some(font) = style.text_styles.get_mut(&text_style) {
            *font = FontId::new(size, font.family.clone());
        }
    }
    style.visuals.widgets.noninteractive.fg_stroke = Stroke::new(1.0, p.window_fg);
}

/// A bold-looking section heading (Adwaita `heading` labels: "Connection", "Size").
///
/// No bold font is bundled, so the weight is faked by painting the text twice, a
/// fraction of a point apart. The label is a normal accessible label.
///
/// # Arguments
/// * `ui` - Where to put the heading.
/// * `text` - The heading text, already translated.
pub fn section_heading(ui: &mut egui::Ui, text: &str) -> egui::Response {
    let font = TextStyle::Body.resolve(ui.style());
    let color = ui.visuals().strong_text_color();
    let response = ui.label(RichText::new(text).font(font.clone()).color(color));
    let galley = ui.painter().layout_no_wrap(text.to_owned(), font, color);
    let pos = response.rect.min + egui::vec2(0.6, 0.0);
    ui.painter().galley(pos, galley, color);
    response
}

/// Adwaita's title sizes in points: `title-1` (page titles such as "CAM Tools"), `title-2`
/// (a CAM tool's title), `title-3` (a placeholder) and `title-4` (a list row's name).
pub const TITLE_1: f32 = 22.0;
/// See [`TITLE_1`].
pub const TITLE_2: f32 = 18.0;
/// See [`TITLE_1`].
pub const TITLE_3: f32 = 16.0;
/// See [`TITLE_1`].
pub const TITLE_4: f32 = 15.0;

/// A bold-looking title of `size` points (Adwaita's `title-1` to `title-4` labels), drawn
/// like [`section_heading`]. The label is a normal accessible label.
///
/// # Arguments
/// * `ui` - Where to put the title.
/// * `text` - The title text, already translated.
/// * `size` - The font size in points, such as [`TITLE_1`].
pub fn title(ui: &mut egui::Ui, text: &str, size: f32) -> egui::Response {
    let font = FontId::proportional(size);
    let color = ui.visuals().strong_text_color();
    let response = ui.label(RichText::new(text).font(font.clone()).color(color));
    let galley = ui.painter().layout_no_wrap(text.to_owned(), font, color);
    let pos = response.rect.min + egui::vec2(0.7, 0.0);
    ui.painter().galley(pos, galley, color);
    response
}

/// A small green badge with bold white text, like libadwaita's `active-badge` ("Active",
/// "Connected"). The text is a normal accessible label.
///
/// # Arguments
/// * `ui` - Where to put the badge.
/// * `text` - The badge text, already translated.
pub fn badge(ui: &mut egui::Ui, text: &str) -> egui::Response {
    egui::Frame::new()
        .fill(ACTIVE_BADGE)
        .corner_radius(CornerRadius::same(4))
        .inner_margin(Margin::symmetric(4, 1))
        .show(ui, |ui| {
            ui.label(
                RichText::new(text)
                    .color(Color32::WHITE)
                    .strong()
                    .size(11.0),
            )
        })
        .inner
}

/// A rounded, bordered card for a group of related controls, like the Designer inspector's
/// "Size" and "Machine Type" groups.
///
/// Use it as `theme::group_frame(ui).show(ui, |ui| { ... })`.
///
/// # Arguments
/// * `ui` - The ui the group goes into; its theme decides the colours.
pub fn group_frame(ui: &egui::Ui) -> egui::Frame {
    let p = palette(mode_of(ui));
    egui::Frame::new()
        .fill(p.card_bg)
        .stroke(Stroke::new(1.0, p.borders))
        .corner_radius(CornerRadius::same(GROUP_RADIUS))
        .inner_margin(Margin::same(10))
}

/// A bold white label on `fill`, shaped like an Adwaita suggested or destructive button.
fn filled_button(text: impl Into<String>, fill: Color32) -> egui::Button<'static> {
    egui::Button::new(RichText::new(text.into()).color(Color32::WHITE).strong())
        .fill(fill)
        .corner_radius(CornerRadius::same(WIDGET_RADIUS))
}

/// A primary (Adwaita "suggested-action") button: white text on the accent blue. Used for
/// Connect.
///
/// # Arguments
/// * `text` - The button label, already translated.
pub fn primary_button(text: impl Into<String>) -> egui::Button<'static> {
    filled_button(text, ACCENT_BLUE)
}

/// A destructive button (Adwaita "destructive-action"): white text on red. Used for Stop
/// and Zero All.
///
/// The fill suits both themes; it is the dark red when the app is dark and still readable
/// on light.
///
/// # Arguments
/// * `text` - The button label, already translated.
pub fn destructive_button(ui: &egui::Ui, text: impl Into<String>) -> egui::Button<'static> {
    filled_button(text, destructive_fill(mode_of(ui)))
}

/// An icon and a bold white label on `fill`, shaped like an Adwaita suggested or
/// destructive button with an icon.
fn filled_icon_button(
    image: egui::Image<'static>,
    text: impl Into<String>,
    fill: Color32,
) -> egui::Button<'static> {
    egui::Button::image_and_text(
        image.tint(Color32::WHITE),
        RichText::new(text.into()).color(Color32::WHITE).strong(),
    )
    .fill(fill)
    .corner_radius(CornerRadius::same(WIDGET_RADIUS))
}

/// A [`primary_button`] with an icon before the label (Connect, Send). The label is the
/// accessible name.
///
/// # Arguments
/// * `image` - The icon; it is drawn white.
/// * `text` - The button label, already translated.
pub fn primary_icon_button(
    image: egui::Image<'static>,
    text: impl Into<String>,
) -> egui::Button<'static> {
    filled_icon_button(image, text, ACCENT_BLUE)
}

/// A [`destructive_button`] with an icon before the label (Stop, Zero All Axes). The label
/// is the accessible name.
///
/// # Arguments
/// * `ui` - The ui the button goes into; its theme decides the red.
/// * `image` - The icon; it is drawn white.
/// * `text` - The button label, already translated.
pub fn destructive_icon_button(
    ui: &egui::Ui,
    image: egui::Image<'static>,
    text: impl Into<String>,
) -> egui::Button<'static> {
    filled_icon_button(image, text, destructive_fill(mode_of(ui)))
}

/// The light/dark mode `ui` is drawn in.
pub(crate) fn mode_of(ui: &egui::Ui) -> egui::Theme {
    if ui.visuals().dark_mode {
        egui::Theme::Dark
    } else {
        egui::Theme::Light
    }
}

/// Runs `add` (which adds one checkbox) with the box drawn accent blue and its tick white
/// when `checked`, as libadwaita draws a checked check button. The label keeps its colour.
///
/// egui's checkbox does not know its own state when it picks its fill, so the colour is set
/// by the caller from the value it is about to show.
///
/// # Arguments
/// * `ui` - Where the checkbox goes.
/// * `checked` - The value the checkbox shows.
/// * `add` - Adds the checkbox and returns its result.
pub fn checked_scope<R>(
    ui: &mut egui::Ui,
    checked: bool,
    add: impl FnOnce(&mut egui::Ui) -> R,
) -> R {
    ui.scope(|ui| {
        if checked {
            let text = ui.visuals().widgets.noninteractive.fg_stroke.color;
            let visuals = ui.visuals_mut();
            visuals.override_text_color = Some(text);
            for state in [
                &mut visuals.widgets.inactive,
                &mut visuals.widgets.hovered,
                &mut visuals.widgets.active,
            ] {
                state.bg_fill = ACCENT_BLUE;
                state.bg_stroke = Stroke::NONE;
                state.fg_stroke = Stroke::new(1.5, Color32::WHITE);
            }
        }
        add(ui)
    })
    .inner
}

/// A checkbox that is accent blue with a white tick when checked, in both themes. Use it in
/// place of `ui.checkbox`.
///
/// # Arguments
/// * `ui` - Where the checkbox goes.
/// * `checked` - The value to edit.
/// * `atoms` - The label.
pub fn checkbox<'a>(
    ui: &mut egui::Ui,
    checked: &'a mut bool,
    atoms: impl egui::IntoAtoms<'a>,
) -> egui::Response {
    let on = *checked;
    checked_scope(ui, on, |ui| ui.checkbox(checked, atoms))
}

/// Installs the light and dark styles in `ctx`, so switching mode is one `set_theme`.
///
/// # Arguments
/// * `ctx` - The egui context.
pub fn install(ctx: &egui::Context) {
    for mode in [egui::Theme::Light, egui::Theme::Dark] {
        ctx.style_mut_of(mode, |style| apply_style(style, mode));
    }
}
