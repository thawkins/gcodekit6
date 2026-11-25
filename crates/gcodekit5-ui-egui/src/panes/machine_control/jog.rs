//! The jog section: step and feed, the continuous (hold-to-jog) switch, and the X/Y pad
//! with the Z column.
//!
//! Every jog runs in the direction Preferences > Machine > Invert gives the axis
//! (`device_status::jog_direction`, as in the GTK front end). Step and feed are mm and mm/min
//! internally and shown, and typed, in the user's measurement system and feed units; the `$J`
//! line is always G21 (mm), as GTK sends it.
//!
//! In step mode a click jogs one step. In continuous mode a hold is GRBL's documented
//! continuous jog: one `$J` move toward the end of travel, cancelled with jog cancel (0x85).
//! The move is clamped to the travel left in that direction (see [`remaining_travel`]); with
//! no known machine position, or no travel left, nothing moves and the pane says why.
//!
//! Jogging (step and hold) is enabled only while the last status report says `Idle` or
//! `Jog` and was read after the last command line (`$H`, `$X`, MDI) was written.
//!
//! The hold is tracked here rather than with egui's "pointer down on" flag, which this egui
//! clears once a press can no longer be a click: it lasts while the primary button that
//! was pressed on the button stays down, the pointer stays on the button and the button
//! stays enabled. Anything else ends it with 0x85, and the pane and the app end it on
//! focus loss, a hidden pane, disconnect, stream start, soft reset and exit
//! ([`JogPanel::take_hold`], [`JogPanel::forget_hold`]). Keyboard jog keys are unbound by
//! design (see [`crate::commands`]).
//!
//! Dependencies: `egui`, `gcodekit5-app` (jog step presets, axes, invert, translations),
//! `gcodekit5-core` (units).

use gcodekit5_app::device_status::jog_direction;
use gcodekit5_app::i18n::Translator;
use gcodekit5_app::machine::Axis;
use gcodekit5_app::machine::grbl_stream::jog_step_choices;
use gcodekit5_core::units::{FeedRateUnits, MeasurementSystem};

use crate::commands::Command;
use crate::machine::MachineStatus;

/// The msgids this section shows (the button labels are the jog commands' labels).
pub(super) const MSGIDS: &[&str] = &[
    n_!("Jog"),
    n_!("Step"),
    n_!("Jog Feed:"),
    n_!("Continuous"),
    n_!("Jog refused: the machine position is unknown."),
    n_!("Jog refused: no travel left in that direction."),
    n_!("Jog refused: the machine is not idle."),
];

/// Default step, feed and travel, as `MachineSettings` defaults them.
const DEFAULT_STEP_MM: f32 = 1.0;
const DEFAULT_FEED_MM_MIN: f32 = 1000.0;
const DEFAULT_TRAVEL_MM: [f32; 3] = [200.0, 200.0, 100.0];
/// Millimetres per inch.
const MM_PER_INCH: f32 = 25.4;

/// What the section asks the controller to do.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(super) enum Action {
    /// A relative `$J` move.
    Move {
        /// The axis to move.
        axis: Axis,
        /// Signed distance in mm, inversion applied.
        distance_mm: f32,
        /// Feed in mm/min.
        feed_mm_min: f32,
    },
    /// Jog cancel (0x85).
    Cancel,
    /// A hold refused; the msgid says why.
    Refused(&'static str),
}

/// One button of the jog pad.
#[derive(Debug, Clone, Copy)]
struct JogButton {
    /// The command whose label names the button.
    command: Command,
    /// The button's text.
    text: &'static str,
    axis: Axis,
    /// +1 or -1, before inversion.
    sign: f32,
    /// Index of `axis` in the travel tables.
    index: usize,
}

const fn jog_button(command: Command, text: &'static str, axis: Axis, sign: f32) -> JogButton {
    let index = match axis {
        Axis::X => 0,
        Axis::Y => 1,
        Axis::Z | Axis::A | Axis::B | Axis::C => 2,
    };
    JogButton {
        command,
        text,
        axis,
        sign,
        index,
    }
}

const Y_POS: JogButton = jog_button(Command::JogYPos, "Y+", Axis::Y, 1.0);
const Y_NEG: JogButton = jog_button(Command::JogYNeg, "Y-", Axis::Y, -1.0);
const X_POS: JogButton = jog_button(Command::JogXPos, "X+", Axis::X, 1.0);
const X_NEG: JogButton = jog_button(Command::JogXNeg, "X-", Axis::X, -1.0);
const Z_POS: JogButton = jog_button(Command::JogZPos, "Z+", Axis::Z, 1.0);
const Z_NEG: JogButton = jog_button(Command::JogZNeg, "Z-", Axis::Z, -1.0);

/// The signed jog distance for `axis` moved `distance_mm` in the direction the user asked
/// for: negated when Preferences inverts the axis.
pub(super) fn directed(axis: Axis, distance_mm: f32) -> f32 {
    distance_mm * jog_direction(axis.letter())
}

/// The travel left from `mpos_mm` in `direction` (+1 or -1), on an axis with `travel_mm`.
///
/// GRBL's machine coordinates: after homing the machine origin is at the homed end and the
/// axis runs from 0 to `-travel` (`$130`-`$132`); soft limits check exactly that range.
/// Stock GRBL keeps this range whatever the homing direction (`$23`) or step direction
/// (`$3`) masks say (only a build with `HOMING_FORCE_SET_ORIGIN` moves it), so moving `+`
/// leaves `-mpos` and moving `-` leaves `travel + mpos`. Neither the GTK front end nor the
/// app crate implements another convention.
pub(super) fn remaining_travel(mpos_mm: f32, travel_mm: f32, direction: f32) -> f32 {
    if direction > 0.0 {
        -mpos_mm
    } else {
        travel_mm + mpos_mm
    }
}

/// mm/min to `units`.
pub(super) fn feed_to_display(mm_min: f32, units: FeedRateUnits) -> f32 {
    match units {
        FeedRateUnits::MmPerMin => mm_min,
        FeedRateUnits::MmPerSec => mm_min / 60.0,
        FeedRateUnits::InPerMin => mm_min / MM_PER_INCH,
        FeedRateUnits::InPerSec => mm_min / MM_PER_INCH / 60.0,
    }
}

/// `units` to mm/min.
fn feed_from_display(value: f32, units: FeedRateUnits) -> f32 {
    match units {
        FeedRateUnits::MmPerMin => value,
        FeedRateUnits::MmPerSec => value * 60.0,
        FeedRateUnits::InPerMin => value * MM_PER_INCH,
        FeedRateUnits::InPerSec => value * MM_PER_INCH * 60.0,
    }
}

/// The jog settings and the hold in progress.
#[derive(Debug)]
pub(super) struct JogPanel {
    step_mm: f32,
    feed_mm_min: f32,
    /// Configured X, Y, Z travel in mm, used until the controller reports `$130`-`$132`.
    travel_mm: [f32; 3],
    units: MeasurementSystem,
    feed_units: FeedRateUnits,
    continuous: bool,
    /// The button held in continuous mode.
    held: Option<Command>,
}

impl Default for JogPanel {
    fn default() -> Self {
        Self::new(DEFAULT_STEP_MM, DEFAULT_FEED_MM_MIN, DEFAULT_TRAVEL_MM)
    }
}

impl JogPanel {
    /// A panel jogging by `step_mm` at `feed_mm_min` within `travel_mm`. Values that are not
    /// positive fall back to the defaults.
    pub(super) fn new(step_mm: f32, feed_mm_min: f32, travel_mm: [f32; 3]) -> Self {
        let positive = |v: f32, default: f32| if v.is_finite() && v > 0.0 { v } else { default };
        Self {
            step_mm: positive(step_mm, DEFAULT_STEP_MM),
            feed_mm_min: positive(feed_mm_min, DEFAULT_FEED_MM_MIN),
            travel_mm: [0, 1, 2].map(|i| positive(travel_mm[i], DEFAULT_TRAVEL_MM[i])),
            units: MeasurementSystem::Metric,
            feed_units: FeedRateUnits::MmPerMin,
            continuous: false,
            held: None,
        }
    }

    /// Sets the units the step and feed are shown and typed in, and the configured travel.
    pub(super) fn configure(
        &mut self,
        units: MeasurementSystem,
        feed_units: FeedRateUnits,
        travel_mm: [f32; 3],
    ) {
        self.units = units;
        self.feed_units = feed_units;
        let current = self.travel_mm;
        self.travel_mm = [0, 1, 2].map(|i| {
            if travel_mm[i] > 0.0 {
                travel_mm[i]
            } else {
                current[i]
            }
        });
    }

    /// The step in mm.
    pub(super) fn step_mm(&self) -> f32 {
        self.step_mm
    }

    /// The feed in mm/min.
    pub(super) fn feed_mm_min(&self) -> f32 {
        self.feed_mm_min
    }

    /// Whether a hold is in progress.
    pub(super) fn is_holding(&self) -> bool {
        self.held.is_some()
    }

    /// Ends the hold in progress; returns whether there was one (the caller sends 0x85).
    pub(super) fn take_hold(&mut self) -> bool {
        self.held.take().is_some()
    }

    /// Forgets the hold in progress without a cancel: the controller was reset or the
    /// connection closed, which ends the jog.
    pub(super) fn forget_hold(&mut self) {
        self.held = None;
    }

    /// The hold distance for `button`: the configured (or reported) travel, clamped to the
    /// travel left in the direction of motion; an error msgid when it must not move.
    fn hold_distance(
        &self,
        button: JogButton,
        status: &MachineStatus,
    ) -> Result<f32, &'static str> {
        let travel = status.grbl_travel[button.index].unwrap_or(self.travel_mm[button.index]);
        let mpos = status
            .machine_position()
            .ok_or(n_!("Jog refused: the machine position is unknown."))?;
        let direction = directed(button.axis, button.sign);
        let left = remaining_travel(mpos[button.index], travel, direction);
        if left <= 0.0 {
            return Err(n_!("Jog refused: no travel left in that direction."));
        }
        Ok(direction * travel.min(left))
    }

    /// Draws one jog button and adds what it asks for to `actions`.
    fn button(
        &mut self,
        ui: &mut egui::Ui,
        tr: &Translator,
        button: JogButton,
        enabled: bool,
        status: &MachineStatus,
        actions: &mut Vec<Action>,
    ) {
        let label = tr.tr(button.command.label_msgid()).into_owned();
        // Click and drag: a press that moves stays with the button, so it never
        // drag-scrolls the pane.
        let widget = egui::Button::new(egui::RichText::new(button.text).strong())
            .min_size(egui::vec2(44.0, 36.0))
            .sense(egui::Sense::click_and_drag());
        let response = ui.add_enabled(enabled, widget).on_hover_text(label.clone());
        response.widget_info(|| egui::WidgetInfo::labeled(egui::Role::Button, enabled, &label));
        if !self.continuous {
            if response.clicked() {
                actions.push(Action::Move {
                    axis: button.axis,
                    distance_mm: directed(button.axis, button.sign * self.step_mm),
                    feed_mm_min: self.feed_mm_min,
                });
            }
            return;
        }
        let (pressed, down, pos, press_origin) = ui.input(|i| {
            (
                i.pointer.primary_pressed(),
                i.pointer.primary_down(),
                i.pointer.interact_pos(),
                i.pointer.press_origin(),
            )
        });
        let on_button = |p: Option<egui::Pos2>| p.is_some_and(|p| response.rect.contains(p));
        if self.held == Some(button.command) {
            if !(enabled && down && on_button(pos)) {
                self.held = None;
                actions.push(Action::Cancel);
            }
        } else if self.held.is_none()
            && enabled
            && pressed
            && on_button(press_origin)
            && response.contains_pointer()
        {
            match self.hold_distance(button, status) {
                Ok(distance_mm) => {
                    self.held = Some(button.command);
                    actions.push(Action::Move {
                        axis: button.axis,
                        distance_mm,
                        feed_mm_min: self.feed_mm_min,
                    });
                }
                Err(msgid) => actions.push(Action::Refused(msgid)),
            }
        }
    }

    /// Draws the step chooser in the user's units.
    fn show_step(&mut self, ui: &mut egui::Ui, tr: &Translator) {
        ui.horizontal(|ui| {
            let label = ui.label(tr.tr("Step"));
            let (choices, current) = jog_step_choices(self.units, self.step_mm);
            let selected = choices
                .get(current)
                .map(|c| c.label.clone())
                .unwrap_or_default();
            egui::ComboBox::from_id_salt("machine-control-jog-step")
                .selected_text(selected)
                .show_ui(ui, |ui| {
                    for choice in &choices {
                        ui.selectable_value(&mut self.step_mm, choice.mm, &choice.label);
                    }
                })
                .response
                .labelled_by(label.id);
        });
    }

    /// Draws the feed field in the user's feed units.
    fn show_feed(&mut self, ui: &mut egui::Ui, tr: &Translator) {
        ui.horizontal(|ui| {
            let label = ui.label(tr.tr("Jog Feed:"));
            let mut shown = feed_to_display(self.feed_mm_min, self.feed_units);
            let max = feed_to_display(100_000.0, self.feed_units);
            let response = ui
                .add(
                    egui::DragValue::new(&mut shown)
                        .range(0.01..=max)
                        .max_decimals(2)
                        .suffix(format!(" {}", self.feed_units)),
                )
                .labelled_by(label.id);
            if response.changed() {
                self.feed_mm_min = feed_from_display(shown, self.feed_units);
            }
        });
    }

    /// Draws the section.
    pub(super) fn show(
        &mut self,
        ui: &mut egui::Ui,
        tr: &Translator,
        status: &MachineStatus,
        allowed: bool,
    ) -> Vec<Action> {
        let mut actions = Vec::new();
        // Disabled outside Idle and Jog, and until a report read after the last command line
        // (`allowed`, from `Machine::jog_allowed`); a hold ends when its button disables.
        let enabled = status.connected && !status.streaming && allowed;
        ui.strong(tr.tr("Jog"));
        self.show_step(ui, tr);
        self.show_feed(ui, tr);
        let was_continuous = self.continuous;
        ui.checkbox(&mut self.continuous, tr.tr("Continuous"));
        if was_continuous && !self.continuous && self.take_hold() {
            actions.push(Action::Cancel);
        }
        ui.horizontal(|ui| {
            egui::Grid::new("machine-control-jog-xy")
                .spacing([4.0, 4.0])
                .show(ui, |ui| {
                    ui.label("");
                    self.button(ui, tr, Y_POS, enabled, status, &mut actions);
                    ui.label("");
                    ui.end_row();
                    self.button(ui, tr, X_NEG, enabled, status, &mut actions);
                    ui.label("");
                    self.button(ui, tr, X_POS, enabled, status, &mut actions);
                    ui.end_row();
                    ui.label("");
                    self.button(ui, tr, Y_NEG, enabled, status, &mut actions);
                    ui.label("");
                    ui.end_row();
                });
            ui.add_space(12.0);
            ui.vertical(|ui| {
                self.button(ui, tr, Z_POS, enabled, status, &mut actions);
                self.button(ui, tr, Z_NEG, enabled, status, &mut actions);
            });
        });
        actions
    }
}
