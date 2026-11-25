//! Application state and root UI of the egui front end: the window frame, the light/dark
//! theme and the docking workspace.
//!
//! Who draws the title bar is decided by `craft_window::policy` before the window opens and
//! carried in [`AppState::frame`]; when the app draws it, the top strip acts as the title bar
//! (`craft_window::title_bar`) and carries the caption buttons in the desktop's order. The
//! light/dark mode follows `craft_platform::appearance` unless the user forced one in
//! [`UiSettings::theme`](gcodekit5_settings::config::UiSettings::theme).
//!
//! Dependencies: `egui`, `eframe`, `craft-window`, `craft-platform`, `gcodekit5-app`
//! (translations), `gcodekit5-settings` (configuration).

use craft_platform::appearance::SystemThemeFn;
use craft_window::policy::{Frame, MacInset};
use craft_window::{Button, Layout};
use gcodekit5_app::i18n::Translator;
use gcodekit5_settings::SettingsManager;
use gcodekit5_settings::config::Config;

use crate::theme;
use crate::workspace::{PaneContext, Workspace};

/// Height of the app-drawn title bar, in points.
const TITLE_BAR_HEIGHT: f32 = 32.0;
/// Width of the band along the window edge that resizes an undecorated window, in points.
const RESIZE_BAND: f32 = 5.0;
/// Size of one caption button, in points.
const CAPTION_SIZE: egui::Vec2 = egui::vec2(32.0, 24.0);

/// State held by the egui front end.
#[derive(Debug)]
pub struct AppState {
    /// The loaded settings.
    pub config: Config,
    /// Translator for the configured UI language.
    pub translator: Translator,
    /// Who draws the window's title bar.
    pub frame: Frame,
}

impl AppState {
    /// State over `config`, with its UI language and the desktop-drawn frame.
    ///
    /// # Arguments
    /// * `config` - The settings to use.
    pub fn from_config(config: Config) -> Self {
        let translator = Translator::new(&config.ui.language);
        Self {
            config,
            translator,
            frame: Frame::Native,
        }
    }

    /// State over the saved settings, or defaults when they cannot be read.
    pub fn load() -> Self {
        let config = match SettingsManager::config_dirs() {
            Ok(dirs) => {
                let (config, problem) = SettingsManager::load(&dirs);
                if let Some(problem) = problem {
                    tracing::warn!(%problem, "settings loaded with problems");
                }
                config
            }
            Err(error) => {
                tracing::warn!(%error, "no settings folder; using defaults");
                Config::default()
            }
        };
        Self::from_config(config)
    }
}

/// Root application of the egui front end.
pub struct GcodeKitApp {
    state: AppState,
    workspace: Workspace,
    system_theme: Option<SystemThemeFn>,
}

impl std::fmt::Debug for GcodeKitApp {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("GcodeKitApp")
            .field("state", &self.state)
            .field("workspace", &self.workspace)
            .finish_non_exhaustive()
    }
}

impl GcodeKitApp {
    /// Creates the application for a new native window, watching the desktop's light/dark
    /// preference.
    ///
    /// # Arguments
    /// * `cc` - eframe's creation context.
    /// * `state` - The front end's state.
    pub fn new(cc: &eframe::CreationContext, state: AppState) -> Self {
        theme::install(&cc.egui_ctx);
        Self {
            system_theme: craft_platform::appearance::egui_service(),
            ..Self::from_state(state)
        }
    }

    /// Creates the application without a desktop appearance watcher (headless use).
    ///
    /// # Arguments
    /// * `state` - The front end's state.
    pub fn from_state(state: AppState) -> Self {
        Self {
            state,
            workspace: Workspace::default_layout(),
            system_theme: None,
        }
    }

    /// Draws the application UI into `ui`.
    ///
    /// # Arguments
    /// * `ui` - The egui `Ui` to draw into.
    pub fn ui(&mut self, ui: &mut egui::Ui) {
        self.sync_theme(ui.ctx());
        if let Frame::Integrated(captions) = &self.state.frame {
            resize_edges(ui.ctx());
            title_bar(ui, &self.state.translator, None, Some(&captions.layout));
        } else if let Frame::Inset(inset) = &self.state.frame {
            title_bar(ui, &self.state.translator, Some(*inset), None);
        }
        let mut ctx = PaneContext::new(&self.state.translator);
        self.workspace.show(ui, &mut ctx);
    }

    /// Shows the light/dark mode the settings and the desktop ask for.
    fn sync_theme(&self, ctx: &egui::Context) {
        let observed = self.system_theme.as_ref().and_then(|watch| watch(ctx));
        let mode = theme::resolve(self.state.config.ui.theme, observed.or(ctx.system_theme()));
        if ctx.theme() != mode {
            theme::install(ctx);
            ctx.set_theme(mode);
        }
    }
}

impl eframe::App for GcodeKitApp {
    /// Draws the whole window.
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        self.ui(ui);
    }
}

/// The app-drawn title bar: the application name, and the caption buttons when `captions`
/// is given; `inset` leaves room for the macOS traffic lights.
fn title_bar(
    ui: &mut egui::Ui,
    tr: &Translator,
    inset: Option<MacInset>,
    captions: Option<&Layout>,
) {
    egui::Panel::top("gcodekit5-title-bar")
        .exact_size(TITLE_BAR_HEIGHT)
        .show(ui, |ui| {
            craft_window::title_bar::empty_area(ui, ui.max_rect());
            ui.horizontal_centered(|ui| {
                if let Some(inset) = inset {
                    ui.add_space(inset.leading);
                }
                if let Some(layout) = captions {
                    layout.left.iter().for_each(|b| caption_button(ui, tr, *b));
                }
                ui.label("GCodeKit");
                if let Some(layout) = captions {
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        // Right to left: the outermost button (listed last) goes first.
                        layout
                            .right
                            .iter()
                            .rev()
                            .for_each(|b| caption_button(ui, tr, *b));
                    });
                }
            });
        });
}

/// One caption button, drawn as a line icon, that sends its window command when clicked.
fn caption_button(ui: &mut egui::Ui, tr: &Translator, button: Button) {
    let (rect, response) = ui.allocate_exact_size(CAPTION_SIZE, egui::Sense::click());
    let maximized = ui.input(|i| i.viewport().maximized).unwrap_or(false);
    let (label, command) = match button {
        Button::Minimize => ("Minimize", egui::ViewportCommand::Minimized(true)),
        Button::Maximize => ("Maximize", egui::ViewportCommand::Maximized(!maximized)),
        Button::Close => ("Close", egui::ViewportCommand::Close),
    };
    let label = tr.tr(label);
    response.widget_info(|| egui::WidgetInfo::labeled(egui::Role::Button, true, &label));
    let visuals = ui.style().interact(&response);
    if response.hovered() {
        ui.painter().rect_filled(rect, 3.0, visuals.bg_fill);
    }
    let c = rect.center();
    let stroke = egui::Stroke::new(1.0, visuals.fg_stroke.color);
    let painter = ui.painter();
    match button {
        Button::Minimize => {
            painter.line_segment(
                [c + egui::vec2(-5.0, 4.0), c + egui::vec2(5.0, 4.0)],
                stroke,
            );
        }
        Button::Maximize => {
            let square = egui::Rect::from_center_size(c, egui::vec2(10.0, 10.0));
            painter.rect_stroke(square, 0.0, stroke, egui::StrokeKind::Inside);
        }
        Button::Close => {
            painter.line_segment(
                [c + egui::vec2(-5.0, -5.0), c + egui::vec2(5.0, 5.0)],
                stroke,
            );
            painter.line_segment(
                [c + egui::vec2(-5.0, 5.0), c + egui::vec2(5.0, -5.0)],
                stroke,
            );
        }
    }
    if response.clicked() {
        ui.ctx().send_viewport_cmd(command);
    }
}

/// Lets the user resize an undecorated window from its edges and corners.
fn resize_edges(ctx: &egui::Context) {
    let still = ctx.input(|i| {
        i.viewport().maximized.unwrap_or(false) || i.viewport().fullscreen.unwrap_or(false)
    });
    let Some(pos) = ctx.pointer_hover_pos() else {
        return;
    };
    if still {
        return;
    }
    let rect = ctx.content_rect();
    let west = pos.x - rect.left() < RESIZE_BAND;
    let east = rect.right() - pos.x < RESIZE_BAND;
    let north = pos.y - rect.top() < RESIZE_BAND;
    let south = rect.bottom() - pos.y < RESIZE_BAND;
    use egui::{CursorIcon as C, ResizeDirection as D};
    let (direction, cursor) = match (north, south, west, east) {
        (true, _, true, _) => (D::NorthWest, C::ResizeNwSe),
        (true, _, _, true) => (D::NorthEast, C::ResizeNeSw),
        (_, true, true, _) => (D::SouthWest, C::ResizeNeSw),
        (_, true, _, true) => (D::SouthEast, C::ResizeNwSe),
        (true, _, _, _) => (D::North, C::ResizeVertical),
        (_, true, _, _) => (D::South, C::ResizeVertical),
        (_, _, true, _) => (D::West, C::ResizeHorizontal),
        (_, _, _, true) => (D::East, C::ResizeHorizontal),
        (false, false, false, false) => return,
    };
    ctx.set_cursor_icon(cursor);
    if ctx.input(|i| i.pointer.primary_pressed()) {
        ctx.send_viewport_cmd(egui::ViewportCommand::BeginResize(direction));
    }
}
