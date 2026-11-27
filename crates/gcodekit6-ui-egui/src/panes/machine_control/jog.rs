//! The jog section: step and feed, the continuous (hold-to-jog) switch, and the X/Y pad
//! with the Z column.
//!
//! Every jog runs in the direction `axis_direction_invert` (in the `[machine]` section of
//! the settings file, `config.toml`; no Preferences row edits it) gives the axis
//! ([`JogPanel::set_inverted`]). Step and feed are mm and mm/min
//! internally and shown, and typed, in the user's measurement system and feed units; the `$J`
//! line is always G21 (mm).
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
//! ([`JogPanel::take_hold`], [`JogPanel::forget_hold`]).
//!
//! The keyboard jogs too (the keypad keys, see [`crate::commands`]): a key press jogs one
//! step, or in continuous mode starts the same hold as a button ([`JogPanel::key_hold`]),
//! which lasts while that key stays down ([`JogPanel::follow_key_hold`]); the pane also ends
//! it when a widget takes the keyboard or the pointer is pressed, and the app ends it on
//! focus loss and on a page switch, as for a button hold.
//!
//! Dependencies: `egui`, `gcodekit6-app` (jog step presets, axes, invert, translations),
//! `gcodekit6-core` (units).

use gcodekit6_app::i18n::Translator;
use gcodekit6_app::machine::Axis;
use gcodekit6_app::machine::grbl_stream::{
    LINEAR_JOG_DECIMALS, ROTARY_JOG_DECIMALS, jog_step_choices, jog_value_text, jog_writes_zero,
};
use gcodekit6_core::units::{FeedRateUnits, MeasurementSystem};

use std::time::{Duration, Instant};

use crate::commands::Command;
use crate::icons::Icon;
use crate::machine::MachineStatus;
use crate::workspace::JogKey;

/// The msgids this section shows (the button labels are the jog commands' labels).
pub(super) const MSGIDS: &[&str] = &[
    n_!("Step"),
    n_!("Jog Feed:"),
    n_!("Continuous"),
    CONTINUOUS_NOT_ON_REPETIER,
    n_!("Jog refused: the machine position is unknown."),
    n_!("Jog refused: no travel left in that direction."),
    n_!("Jog refused: the machine is not idle."),
    n_!("Jog"),
];

/// Why continuous jog is off on a Repetier connection.
const CONTINUOUS_NOT_ON_REPETIER: &str =
    n_!("Continuous jog needs GRBL's jog cancel; Repetier firmware has none.");

/// Default step, feed and travel, as `MachineSettings` defaults them.
const DEFAULT_STEP_MM: f32 = 1.0;
const DEFAULT_FEED_MM_MIN: f32 = 1000.0;
const DEFAULT_TRAVEL_MM: [f32; 3] = [200.0, 200.0, 100.0];
/// How long a keyboard hold lasts without a frame confirming its key is still down.
pub(super) const KEY_HOLD_DEAD_MAN: Duration = Duration::from_millis(250);
/// How often a frame is asked for while a keyboard hold lasts, well inside
/// [`KEY_HOLD_DEAD_MAN`].
pub(super) const KEY_HOLD_FRAME: Duration = Duration::from_millis(50);
/// The smallest rotary step a jog line can write (3 decimals).
const MIN_ROTARY_STEP_DEG: f32 = 0.001;
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
    /// The chevron beside the text.
    icon: Icon,
    axis: Axis,
    /// +1 or -1, before inversion.
    sign: f32,
    /// Index of `axis` in the travel tables.
    index: usize,
}

const fn jog_button(
    command: Command,
    text: &'static str,
    icon: Icon,
    axis: Axis,
    sign: f32,
) -> JogButton {
    let index = match axis {
        Axis::X => 0,
        Axis::Y => 1,
        Axis::Z | Axis::A | Axis::B | Axis::C => 2,
    };
    JogButton {
        command,
        text,
        icon,
        axis,
        sign,
        index,
    }
}

const Y_POS: JogButton = jog_button(Command::JogYPos, "Y+", Icon::JogUp, Axis::Y, 1.0);
const Y_NEG: JogButton = jog_button(Command::JogYNeg, "Y-", Icon::JogDown, Axis::Y, -1.0);
const X_POS: JogButton = jog_button(Command::JogXPos, "X+", Icon::JogRight, Axis::X, 1.0);
const X_NEG: JogButton = jog_button(Command::JogXNeg, "X-", Icon::JogLeft, Axis::X, -1.0);
const Z_POS: JogButton = jog_button(Command::JogZPos, "Z+", Icon::JogUp, Axis::Z, 1.0);
const Z_NEG: JogButton = jog_button(Command::JogZNeg, "Z-", Icon::JogDown, Axis::Z, -1.0);

/// Edge of a jog pad button (56 px).
const PAD_BUTTON: f32 = 56.0;
/// Gap between the pad's buttons (5 px).
const PAD_GAP: f32 = 5.0;
/// Width of the X/Y pad, the gap, the Z column and the gap after it.
pub(super) const PAD_ROW_WIDTH: f32 = 4.0 * PAD_BUTTON + 2.0 * PAD_GAP + 48.0 + 20.0 + 24.0;

/// The slot of `axis` in [`JogPanel`]'s inversion table.
fn axis_slot(axis: Axis) -> usize {
    match axis {
        Axis::X => 0,
        Axis::Y => 1,
        Axis::Z => 2,
        Axis::A => 3,
        Axis::B => 4,
        Axis::C => 5,
    }
}

/// The travel left from `mpos_mm` in `direction` (+1 or -1), on an axis with `travel_mm`.
///
/// GRBL's machine coordinates: after homing the machine origin is at the homed end and the
/// axis runs from 0 to `-travel` (`$130`-`$132`); soft limits check exactly that range.
/// Stock GRBL keeps this range whatever the homing direction (`$23`) or step direction
/// (`$3`) masks say (only a build with `HOMING_FORCE_SET_ORIGIN` moves it), so moving `+`
/// leaves `-mpos` and moving `-` leaves `travel + mpos`. The
/// app crate implements no other convention.
pub(super) fn remaining_travel(mpos_mm: f32, travel_mm: f32, direction: f32) -> f32 {
    if direction > 0.0 {
        -mpos_mm
    } else {
        travel_mm + mpos_mm
    }
}

/// mm/min to `units`.
pub(crate) use gcodekit6_core::units::feed_rate_from_mm_per_min as feed_to_display;

/// `units` to mm/min.
use gcodekit6_core::units::feed_rate_to_mm_per_min as feed_from_display;

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
    /// Whether the connection can end a hold (GRBL's jog cancel); continuous mode is off
    /// while it cannot, see [`JogPanel::set_continuous_allowed`].
    continuous_allowed: bool,
    /// The hold in continuous mode.
    held: Option<Hold>,
    /// The rotary jog step in degrees (`jog_increment_rotary` in the settings file).
    rotary_step_deg: f32,
    /// A, B and C limits in degrees: how far a continuous rotary hold moves at most.
    rotary_limit_deg: [f32; 3],
    /// Whether this frame drew the button of the pointer hold in progress.
    hold_button_drawn: bool,
    /// X, Y, Z, A, B, C: whether `axis_direction_invert` in the settings file reverses the axis.
    inverted: [bool; 6],
}

/// A continuous jog in progress.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct Hold {
    /// The axis jogged and whether toward + (before inversion): which button, or which
    /// key's command.
    target: (Axis, bool),
    /// The key held, for a hold a key started; `None` for a button hold.
    key: Option<JogKey>,
    /// When a frame last confirmed a keyboard hold (its key still down, nothing ending it).
    confirmed: Instant,
}

/// What a key press in continuous mode did.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(super) enum KeyHold {
    /// The hold started with this move.
    Started(Action),
    /// A hold is already in progress (a key repeat, or a button held): nothing to do.
    Busy,
    /// Refused; the msgid says why.
    Refused(&'static str),
}

/// The pad button of a jog command.
fn button_of(command: Command) -> Option<JogButton> {
    [Y_POS, Y_NEG, X_POS, X_NEG, Z_POS, Z_NEG]
        .into_iter()
        .find(|b| b.command == command)
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
            continuous_allowed: true,
            held: None,
            rotary_step_deg: 1.0,
            rotary_limit_deg: [360.0; 3],
            hold_button_drawn: false,
            inverted: [false; 6],
        }
    }

    /// Sets which axes jog in reverse (`axis_direction_invert` in the settings file).
    ///
    /// # Arguments
    /// * `inverted` - X, Y, Z, A, B, C: whether the axis is reversed.
    pub(super) fn set_inverted(&mut self, inverted: [bool; 6]) {
        self.inverted = inverted;
    }

    /// The axes [`JogPanel::set_inverted`] reversed.
    pub(super) fn inverted(&self) -> [bool; 6] {
        self.inverted
    }

    /// The signed jog distance for `axis` moved `distance_mm` in the direction the user
    /// asked for: negated when the settings file inverts the axis.
    pub(super) fn directed(&self, axis: Axis, distance_mm: f32) -> f32 {
        if self.inverted[axis_slot(axis)] {
            -distance_mm
        } else {
            distance_mm
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

    /// Starts a frame of the jog buttons: no hold button drawn yet.
    pub(super) fn begin_frame(&mut self) {
        self.hold_button_drawn = false;
    }

    /// Ends a frame of the jog buttons: a pointer hold whose button was not drawn (a rotary
    /// axis the machine stopped reporting) ends.
    ///
    /// # Returns
    /// Whether a hold ended (the caller sends 0x85).
    pub(super) fn end_frame(&mut self) -> bool {
        let orphan = self
            .held
            .is_some_and(|h| h.key.is_none() && !self.hold_button_drawn);
        if orphan {
            self.held = None;
        }
        orphan
    }

    /// Sets the rotary jog step and the A, B and C limits, in degrees; values that are not
    /// positive keep the current ones. A step below the 0.001 degree a jog line can write
    /// is raised to it, so a click never asks for a zero-length move.
    pub(super) fn configure_rotary(&mut self, step_deg: f32, limits_deg: [f32; 3]) {
        if step_deg.is_finite() && step_deg > 0.0 {
            if step_deg < MIN_ROTARY_STEP_DEG {
                tracing::warn!(
                    step_deg,
                    used = MIN_ROTARY_STEP_DEG,
                    "rotary jog step below what a jog line writes; raised"
                );
            }
            self.rotary_step_deg = step_deg.max(MIN_ROTARY_STEP_DEG);
        }
        for (limit, new) in self.rotary_limit_deg.iter_mut().zip(limits_deg) {
            if new.is_finite() && new > 0.0 {
                *limit = new;
            }
        }
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

    /// Whether continuous (hold-to-jog) mode is on.
    pub(super) fn continuous(&self) -> bool {
        self.continuous && self.continuous_allowed
    }

    /// Allows continuous mode, or turns it off (keeping the user's choice for later) on a
    /// connection that cannot cancel a jog (Repetier). Returns whether a hold was in
    /// progress, which the caller then cancels.
    pub(super) fn set_continuous_allowed(&mut self, allowed: bool) -> bool {
        self.continuous_allowed = allowed;
        !allowed && self.take_hold()
    }

    /// The key a keyboard hold lasts while, if one is in progress.
    pub(super) fn held_key(&self) -> Option<JogKey> {
        self.held.and_then(|h| h.key)
    }

    /// Starts a continuous jog for `command`, run by `key`, lasting while the key is down.
    ///
    /// # Arguments
    /// * `command` - The jog command the key ran.
    /// * `key` - The key held.
    /// * `status` - The machine status (the travel left).
    pub(super) fn key_hold(
        &mut self,
        command: Command,
        key: JogKey,
        status: &MachineStatus,
    ) -> KeyHold {
        if self.held.is_some() {
            return KeyHold::Busy;
        }
        let Some(button) = button_of(command) else {
            return KeyHold::Busy;
        };
        match self.hold_distance(button, status) {
            Ok(distance_mm) => {
                self.held = Some(Hold {
                    target: (button.axis, button.sign > 0.0),
                    key: Some(key),
                    confirmed: Instant::now(),
                });
                KeyHold::Started(Action::Move {
                    axis: button.axis,
                    distance_mm,
                    feed_mm_min: self.feed_mm_min,
                })
            }
            Err(msgid) => KeyHold::Refused(msgid),
        }
    }

    /// Ends a keyboard hold unless it may go on: its key is still down and `may_continue`
    /// (the machine still allows jogging, no widget took the keyboard, no pointer press).
    ///
    /// # Arguments
    /// * `key_down` - Whether the hold's key is down this frame.
    /// * `may_continue` - Whether nothing else ends the hold.
    ///
    /// # Returns
    /// Whether the hold ended (the caller sends 0x85).
    pub(super) fn follow_key_hold(&mut self, key_down: bool, may_continue: bool) -> bool {
        let Some(hold) = self.held.as_mut().filter(|h| h.key.is_some()) else {
            return false;
        };
        if key_down && may_continue {
            hold.confirmed = Instant::now();
            return false;
        }
        self.held = None;
        true
    }

    /// The dead man's switch of a keyboard hold: ends it when no frame has confirmed it for
    /// [`KEY_HOLD_DEAD_MAN`] (frames stopped, or the pane stopped following it).
    ///
    /// # Returns
    /// Whether the hold ended (the caller sends 0x85).
    pub(super) fn expire_key_hold(&mut self) -> bool {
        let expired = self
            .held
            .is_some_and(|h| h.key.is_some() && h.confirmed.elapsed() > KEY_HOLD_DEAD_MAN);
        if expired {
            self.held = None;
        }
        expired
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
        let direction = self.directed(button.axis, button.sign);
        let left = remaining_travel(mpos[button.index], travel, direction);
        let distance = direction * travel.min(left);
        // Less travel left than a jog line can write (it would be sent as `X0.00`).
        if left <= 0.0 || jog_writes_zero(distance, LINEAR_JOG_DECIMALS) {
            return Err(n_!("Jog refused: no travel left in that direction."));
        }
        Ok(distance)
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
        let widget = egui::Button::image_and_text(
            button.icon.themed(ui),
            egui::RichText::new(button.text).strong(),
        )
        .min_size(egui::vec2(PAD_BUTTON, PAD_BUTTON))
        .sense(egui::Sense::click_and_drag());
        let response = ui.add_enabled(enabled, widget).on_hover_text(label.clone());
        response.widget_info(|| egui::WidgetInfo::labeled(egui::Role::Button, enabled, &label));
        let step = self.step_mm;
        self.drive_button(
            ui,
            &response,
            (button.axis, button.sign),
            (enabled, step),
            actions,
            |panel| panel.hold_distance(button, status),
        );
    }

    /// Runs a jog button's step click or press-and-hold, for a pad button or a rotary one.
    ///
    /// # Arguments
    /// * `ui` - The button's `Ui` (for the pointer).
    /// * `response` - The button.
    /// * `(axis, sign)` - The axis and +1 or -1, before inversion.
    /// * `(enabled, step)` - Whether the button is enabled, and the step (mm or degrees).
    /// * `actions` - Where what it asks for goes.
    /// * `hold` - The distance of a hold, or why it is refused.
    fn drive_button(
        &mut self,
        ui: &egui::Ui,
        response: &egui::Response,
        (axis, sign): (Axis, f32),
        (enabled, step): (bool, f32),
        actions: &mut Vec<Action>,
        hold: impl FnOnce(&Self) -> Result<f32, &'static str>,
    ) {
        if !self.continuous() {
            if response.clicked() {
                actions.push(Action::Move {
                    axis,
                    distance_mm: self.directed(axis, sign * step),
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
        let target = (axis, sign > 0.0);
        let pointer_hold =
            |held: Option<Hold>| held.is_some_and(|h| h.key.is_none() && h.target == target);
        if pointer_hold(self.held) {
            self.hold_button_drawn = true;
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
            match hold(self) {
                Ok(distance_mm) => {
                    self.held = Some(Hold {
                        target,
                        key: None,
                        confirmed: Instant::now(),
                    });
                    self.hold_button_drawn = true;
                    actions.push(Action::Move {
                        axis,
                        distance_mm,
                        feed_mm_min: self.feed_mm_min,
                    });
                }
                Err(msgid) => actions.push(Action::Refused(msgid)),
            }
        }
    }

    /// Draws the `-` and `+` jog buttons of the rotary `axis` (the buttons in the axis's
    /// readout row), stepping by the rotary step in degrees, or in continuous mode holding
    /// a move of the axis's configured limit, under the pad's rules and guards.
    ///
    /// # Arguments
    /// * `ui` - Where to draw.
    /// * `tr` - Translator.
    /// * `axis` - A, B or C.
    /// * `enabled` - As the pad's buttons: connected, no stream, jogging allowed.
    /// * `actions` - Where what they ask for goes.
    pub(super) fn rotary_buttons(
        &mut self,
        ui: &mut egui::Ui,
        tr: &Translator,
        axis: Axis,
        enabled: bool,
        actions: &mut Vec<Action>,
    ) {
        let index = match axis {
            Axis::A => 0,
            Axis::B => 1,
            Axis::C => 2,
            Axis::X | Axis::Y | Axis::Z => return,
        };
        let limit = self.rotary_limit_deg[index];
        let step = self.rotary_step_deg;
        for (text, sign) in [("-", -1.0_f32), ("+", 1.0)] {
            let name = format!("{} {}{text}", tr.tr("Jog"), axis.letter());
            let widget = egui::Button::new(egui::RichText::new(text).strong())
                .min_size(egui::vec2(32.0, 32.0))
                .sense(egui::Sense::click_and_drag());
            let response = ui.add_enabled(enabled, widget).on_hover_text(&name);
            response.widget_info(|| egui::WidgetInfo::labeled(egui::Role::Button, enabled, &name));
            self.drive_button(
                ui,
                &response,
                (axis, sign),
                (enabled, step),
                actions,
                |panel| {
                    if limit > 0.0 {
                        Ok(panel.directed(axis, sign * limit))
                    } else {
                        Err(n_!("Jog refused: no travel left in that direction."))
                    }
                },
            );
        }
        // The step these buttons move, as used: a configured step too small for a jog line
        // is raised to 0.001 degree, and shown so.
        let step = jog_value_text(step, ROTARY_JOG_DECIMALS);
        ui.weak(format!("{} {step}\u{b0}", tr.tr("Step")));
    }

    /// Draws the step chooser in the user's units.
    fn show_step(&mut self, ui: &mut egui::Ui, tr: &Translator) {
        let label = ui.label(tr.tr("Step"));
        let (choices, current) = jog_step_choices(self.units, self.step_mm);
        let selected = choices
            .get(current)
            .map(|c| c.label.clone())
            .unwrap_or_default();
        egui::ComboBox::from_id_salt("machine-control-jog-step")
            .selected_text(selected)
            .width(110.0)
            .show_ui(ui, |ui| {
                for choice in &choices {
                    ui.selectable_value(&mut self.step_mm, choice.mm, &choice.label);
                }
            })
            .response
            .labelled_by(label.id);
    }

    /// Draws the feed field in the user's feed units.
    fn show_feed(&mut self, ui: &mut egui::Ui, tr: &Translator) {
        let label = ui.label(tr.tr("Jog Feed:"));
        let mut shown = feed_to_display(f64::from(self.feed_mm_min), self.feed_units);
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
            self.feed_mm_min = feed_from_display(shown, self.feed_units) as f32;
        }
    }

    /// Draws the step, feed and continuous switch in one row, then the X/Y pad and the Z
    /// column, with `beside` drawn to the right of the Z column (the emergency stop).
    ///
    /// # Arguments
    /// * `ui` - Where to draw.
    /// * `tr` - Translator.
    /// * `status` - The machine status.
    /// * `allowed` - [`crate::machine::Machine::jog_allowed`], read after this frame's
    ///   command buttons.
    /// * `beside` - Draws what sits right of the pad.
    pub(super) fn show(
        &mut self,
        ui: &mut egui::Ui,
        tr: &Translator,
        status: &MachineStatus,
        allowed: bool,
        beside: impl FnOnce(&mut egui::Ui),
    ) -> Vec<Action> {
        let mut actions = Vec::new();
        // Disabled outside Idle and Jog, and until a report read after the last command line
        // (`allowed`, from `Machine::jog_allowed`); a hold ends when its button disables.
        let enabled = status.connected && !status.streaming && allowed;
        ui.horizontal_wrapped(|ui| {
            self.show_step(ui, tr);
            ui.add_space(8.0);
            self.show_feed(ui, tr);
            ui.add_space(8.0);
            if self.continuous_allowed {
                let was_continuous = self.continuous;
                crate::theme::checkbox(ui, &mut self.continuous, tr.tr("Continuous"));
                if was_continuous && !self.continuous && self.take_hold() {
                    actions.push(Action::Cancel);
                }
            } else {
                let mut off = false;
                ui.add_enabled_ui(false, |ui| {
                    crate::theme::checkbox(ui, &mut off, tr.tr("Continuous"))
                })
                .inner
                .on_disabled_hover_text(tr.tr(CONTINUOUS_NOT_ON_REPETIER));
            }
        });
        ui.add_space(6.0);
        let cell = egui::vec2(PAD_BUTTON, PAD_BUTTON);
        ui.horizontal(|ui| {
            egui::Grid::new("machine-control-jog-xy")
                .spacing([PAD_GAP, PAD_GAP])
                .min_col_width(PAD_BUTTON)
                .min_row_height(PAD_BUTTON)
                .show(ui, |ui| {
                    ui.label("");
                    self.button(ui, tr, Y_POS, enabled, status, &mut actions);
                    ui.label("");
                    ui.end_row();
                    self.button(ui, tr, X_NEG, enabled, status, &mut actions);
                    ui.add_sized(cell, egui::Label::new("XY"));
                    self.button(ui, tr, X_POS, enabled, status, &mut actions);
                    ui.end_row();
                    ui.label("");
                    self.button(ui, tr, Y_NEG, enabled, status, &mut actions);
                    ui.label("");
                    ui.end_row();
                });
            ui.add_space(48.0);
            ui.vertical(|ui| {
                ui.spacing_mut().item_spacing.y = PAD_GAP;
                self.button(ui, tr, Z_POS, enabled, status, &mut actions);
                ui.add_sized(cell, egui::Label::new("Z"));
                self.button(ui, tr, Z_NEG, enabled, status, &mut actions);
            });
            ui.add_space(20.0);
            beside(ui);
        });
        actions
    }
}
