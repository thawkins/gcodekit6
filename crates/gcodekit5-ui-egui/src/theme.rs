//! Colours and egui visuals of the egui front end, ported from the GTK build.
//!
//! The light and dark [`Palette`]s carry the libadwaita named colours that
//! `crates/gcodekit5-ui/src/ui/gtk/style.css` refers to (`@window_bg_color`, `@accent_color`,
//! `@success_color`, ...); the status colours are the values
//! `crates/gcodekit5-ui/src/ui/gtk/common/colors.rs` hard-codes. The fixed constants are the
//! rendering colours of `colors.rs` and the literal colours of `style.css`.
//!
//! Which palette applies is [`resolve`]d from the user's choice in
//! [`gcodekit5_settings::config::UiSettings::theme`] and the desktop's preference.
//!
//! Dependencies: `egui`, `craft-ui` (widget palette), `gcodekit5-settings` (theme choice).

use egui::{Color32, CornerRadius, Stroke, Visuals};
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
/// The forced mode, else the desktop's, else dark (the GTK build's default look).
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
        CornerRadius::same(3),
        Stroke::new(1.0, p.window_fg),
    );
    visuals.widgets.noninteractive.bg_stroke = Stroke::new(1.0, p.borders);
    visuals.panel_fill = p.window_bg;
    visuals.window_fill = p.card_bg;
    visuals.extreme_bg_color = p.view_bg;
    visuals.faint_bg_color = p.headerbar_bg;
    visuals.window_stroke = Stroke::new(1.0, p.borders);
    visuals.hyperlink_color = p.accent;
    visuals.selection.bg_fill = p.accent_bg;
    visuals.selection.stroke = Stroke::new(1.0, Color32::WHITE);
    visuals.warn_fg_color = p.warning;
    visuals.error_fg_color = p.error;
    visuals
}

/// Installs the light and dark visuals in `ctx`, so switching mode is one `set_theme`.
///
/// # Arguments
/// * `ctx` - The egui context.
pub fn install(ctx: &egui::Context) {
    ctx.set_visuals_of(egui::Theme::Light, visuals(egui::Theme::Light));
    ctx.set_visuals_of(egui::Theme::Dark, visuals(egui::Theme::Dark));
}
