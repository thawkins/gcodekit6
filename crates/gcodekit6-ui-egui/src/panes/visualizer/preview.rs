//! The visualizer's run preview: play, pause and stop with a speed choice, moving a
//! preview marker along the toolpath. It only animates the view; nothing is sent to the
//! machine.
//!
//! Each move is split into points about 3 mm apart
//! (2 to 24 per move), a dwell holds the marker for 8 points a second (1 to 24), and the
//! path stops at [`MAX_POINTS`]. An arc's points follow the arc (its segments as
//! the toolpath draws them) instead of the chord. Every [`TICK`] the marker moves on by the
//! speed's step (Slow 1, Normal 2, Fast 5, Turbo 20 points); at the end it stays on the last
//! point until Play starts again from the
//! beginning.
//!
//! The bar ([`show_bar`]) sits at the bottom left of the view.
//!
//! Dependencies: `egui`, `glam`, `gcodekit6-app` (translations).

use std::time::Duration;

use glam::Vec3;

use crate::icons::{Icon, icon_only_button};

/// How often the marker moves on (33 ms).
pub const TICK: Duration = Duration::from_millis(33);
/// Most points in a preview path.
pub const MAX_POINTS: usize = 40_000;
/// Distance between preview points along a move, in mm.
const POINT_SPACING: f32 = 3.0;
/// Most points per move or dwell.
const MAX_PER_MOVE: usize = 24;
/// Preview points per second of dwell.
const DWELL_POINTS_PER_SECOND: f32 = 8.0;

/// The msgids the run preview shows, for the translation check.
pub fn msgids() -> Vec<&'static str> {
    vec![
        n_!("Play preview"),
        n_!("Pause preview"),
        n_!("Stop preview"),
        n_!("Speed"),
        n_!("Preview playback speed"),
        n_!("Slow"),
        n_!("Normal"),
        n_!("Fast"),
        n_!("Turbo"),
        n_!("No G-code to run"),
        n_!("Generate or open G-code before using Run."),
        n_!("No motion commands found"),
        n_!("The loaded G-code has no G0/G1/G2/G3 movement to preview."),
    ]
}

/// The run preview's playback speed.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum PreviewSpeed {
    /// One point per tick.
    Slow,
    /// Two points per tick (the default).
    #[default]
    Normal,
    /// Five points per tick.
    Fast,
    /// Twenty points per tick.
    Turbo,
}

impl PreviewSpeed {
    /// Every speed, slowest first.
    pub const ALL: [PreviewSpeed; 4] = [
        PreviewSpeed::Slow,
        PreviewSpeed::Normal,
        PreviewSpeed::Fast,
        PreviewSpeed::Turbo,
    ];

    /// Points the marker moves on per [`TICK`].
    pub fn step(self) -> usize {
        match self {
            PreviewSpeed::Slow => 1,
            PreviewSpeed::Normal => 2,
            PreviewSpeed::Fast => 5,
            PreviewSpeed::Turbo => 20,
        }
    }

    /// The speed's name, untranslated.
    pub fn msgid(self) -> &'static str {
        match self {
            PreviewSpeed::Slow => "Slow",
            PreviewSpeed::Normal => "Normal",
            PreviewSpeed::Fast => "Fast",
            PreviewSpeed::Turbo => "Turbo",
        }
    }
}

/// Where the run preview stands.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum PreviewState {
    /// Not started, or stopped: no preview marker.
    #[default]
    Stopped,
    /// The marker moves.
    Playing,
    /// The marker holds its place.
    Paused,
    /// The marker reached the end and stays on the last point.
    Finished,
}

/// Collects a preview path, capped at [`MAX_POINTS`].
#[derive(Debug, Clone, Default, PartialEq)]
pub struct PathBuilder {
    points: Vec<Vec3>,
}

impl PathBuilder {
    /// Adds the points of a move from `from` to `to`: about 3 mm apart, at least `min` of
    /// them (2 per move; an arc's short segments take 1) and at most 24.
    ///
    /// # Arguments
    /// * `from` - Where the move starts.
    /// * `to` - Where it ends.
    /// * `min` - The fewest points to add.
    pub fn line(&mut self, from: Vec3, to: Vec3, min: usize) {
        let steps = ((from.distance(to) / POINT_SPACING).ceil() as usize).clamp(min, MAX_PER_MOVE);
        for s in 1..=steps {
            if self.points.len() >= MAX_POINTS {
                return;
            }
            self.points.push(from.lerp(to, s as f32 / steps as f32));
        }
    }

    /// Holds the marker at `at` for a dwell of `seconds`.
    ///
    /// # Arguments
    /// * `at` - Where the tool dwells.
    /// * `seconds` - The dwell's length.
    pub fn dwell(&mut self, at: Vec3, seconds: f32) {
        let holds = (seconds.max(0.0) * DWELL_POINTS_PER_SECOND).round() as usize;
        for _ in 0..holds.clamp(1, MAX_PER_MOVE) {
            if self.points.len() >= MAX_POINTS {
                return;
            }
            self.points.push(at);
        }
    }

    /// The path.
    pub fn finish(self) -> Vec<Vec3> {
        self.points
    }
}

/// The run preview: its path, where the marker is, and the playback state.
#[derive(Debug, Clone, Default)]
pub struct RunPreview {
    points: Vec<Vec3>,
    /// The point the marker shows.
    shown: usize,
    state: PreviewState,
    speed: PreviewSpeed,
    /// Time played since the last tick.
    carry: Duration,
}

impl RunPreview {
    /// Replaces the path (a new program's), stopping the preview.
    ///
    /// # Arguments
    /// * `points` - The new path.
    pub fn set_points(&mut self, points: Vec<Vec3>) {
        self.stop();
        self.points = points;
    }

    /// The number of points in the path.
    pub fn len(&self) -> usize {
        self.points.len()
    }

    /// Whether the path is empty (the program does not move).
    pub fn is_empty(&self) -> bool {
        self.points.is_empty()
    }

    /// Play: resumes a paused preview, or starts from the first point.
    ///
    /// # Returns
    /// Whether the preview plays (`false` when the path is empty).
    pub fn play(&mut self) -> bool {
        match self.state {
            PreviewState::Playing => {}
            PreviewState::Paused => self.state = PreviewState::Playing,
            PreviewState::Stopped | PreviewState::Finished => {
                self.restart();
            }
        }
        self.state == PreviewState::Playing
    }

    /// Starts from the first point (File > Run), whatever the state.
    ///
    /// # Returns
    /// Whether the preview plays (`false` when the path is empty).
    pub fn restart(&mut self) -> bool {
        self.shown = 0;
        self.carry = Duration::ZERO;
        self.state = if self.points.is_empty() {
            PreviewState::Stopped
        } else {
            PreviewState::Playing
        };
        self.state == PreviewState::Playing
    }

    /// Pause: holds a playing preview.
    pub fn pause(&mut self) {
        if self.state == PreviewState::Playing {
            self.state = PreviewState::Paused;
        }
    }

    /// Stop: ends the preview and removes its marker.
    pub fn stop(&mut self) {
        self.state = PreviewState::Stopped;
        self.shown = 0;
        self.carry = Duration::ZERO;
    }

    /// The playback state.
    pub fn state(&self) -> PreviewState {
        self.state
    }

    /// The playback speed.
    pub fn speed(&self) -> PreviewSpeed {
        self.speed
    }

    /// Sets the playback speed.
    ///
    /// # Arguments
    /// * `speed` - The new speed.
    pub fn set_speed(&mut self, speed: PreviewSpeed) {
        self.speed = speed;
    }

    /// Moves the marker on for `elapsed` of playing time: one step of the speed per whole
    /// [`TICK`], the rest carried to the next call.
    ///
    /// # Arguments
    /// * `elapsed` - Time since the last call.
    pub fn advance(&mut self, elapsed: Duration) {
        if self.state != PreviewState::Playing {
            return;
        }
        self.carry += elapsed;
        while self.carry >= TICK && self.state == PreviewState::Playing {
            self.carry -= TICK;
            let next = self.shown.saturating_add(self.speed.step());
            if next >= self.points.len() {
                self.shown = self.points.len().saturating_sub(1);
                self.state = PreviewState::Finished;
            } else {
                self.shown = next;
            }
        }
    }

    /// Where the preview marker is, unless the preview is stopped.
    pub fn position(&self) -> Option<Vec3> {
        match self.state {
            PreviewState::Stopped => None,
            PreviewState::Playing | PreviewState::Paused | PreviewState::Finished => {
                self.points.get(self.shown).copied()
            }
        }
    }
}

/// What a run preview bar button asked for.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum BarAction {
    Play,
    Pause,
    Stop,
}

/// Draws the run preview bar (Play, Pause, Stop, Speed) at the bottom left of the view
/// `rect` and runs what was clicked on `preview`.
///
/// # Arguments
/// * `ui` - The visualizer's `Ui`.
/// * `rect` - The view's rectangle.
/// * `preview` - The run preview.
/// * `tr` - Translator for the current UI language.
///
/// # Returns
/// The bar's rectangle, so other overlays can sit above it.
pub fn show_bar(
    ui: &mut egui::Ui,
    rect: egui::Rect,
    preview: &mut RunPreview,
    tr: &gcodekit6_app::i18n::Translator,
) -> egui::Rect {
    let margin = 10.0;
    let strip = egui::Rect::from_min_max(
        egui::pos2(rect.left() + margin, rect.bottom() - margin - BAR_HEIGHT),
        egui::pos2(rect.right() - margin, rect.bottom() - margin),
    );
    let mut action = None;
    let mut speed = preview.speed();
    let frame = ui
        .scope_builder(
            egui::UiBuilder::new()
                .max_rect(strip)
                .layout(egui::Layout::left_to_right(egui::Align::Center)),
            |ui| {
                egui::Frame::new()
                    .fill(ui.visuals().window_fill.gamma_multiply(0.85))
                    .corner_radius(4.0)
                    .inner_margin(egui::Margin::same(4))
                    .show(ui, |ui| {
                        ui.horizontal(|ui| {
                            let buttons = [
                                (Icon::Play, "Play preview", BarAction::Play),
                                (Icon::Pause, "Pause preview", BarAction::Pause),
                                (Icon::Stop, "Stop preview", BarAction::Stop),
                            ];
                            for (icon, msgid, act) in buttons {
                                if icon_only_button(ui, icon, &tr.tr(msgid)).clicked() {
                                    action = Some(act);
                                }
                            }
                            let label = ui.label(tr.tr("Speed"));
                            egui::ComboBox::from_id_salt("visualizer-preview-speed")
                                .selected_text(tr.tr(speed.msgid()))
                                .show_ui(ui, |ui| {
                                    for option in PreviewSpeed::ALL {
                                        ui.selectable_value(
                                            &mut speed,
                                            option,
                                            tr.tr(option.msgid()),
                                        );
                                    }
                                })
                                .response
                                .on_hover_text(tr.tr("Preview playback speed"))
                                .labelled_by(label.id);
                        });
                    })
                    .response
                    .rect
            },
        )
        .inner;
    preview.set_speed(speed);
    match action {
        Some(BarAction::Play) => {
            preview.play();
        }
        Some(BarAction::Pause) => preview.pause(),
        Some(BarAction::Stop) => preview.stop(),
        None => {}
    }
    frame
}

/// Height of the run preview bar's strip in points.
const BAR_HEIGHT: f32 = 40.0;
