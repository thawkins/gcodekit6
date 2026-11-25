//! The jog section: step and feed, the continuous (hold-to-jog) switch, and the X/Y pad
//! with the Z column.
//!
//! In step mode a click jogs one step. In continuous mode pressing a button starts one `$J`
//! move toward the far end of the axis' travel and releasing it sends jog cancel (0x85),
//! which stops the motion and flushes the move; a button that becomes disabled or a soft
//! reset while held also ends the hold. Keyboard jog keys are unbound by design (see
//! [`crate::commands`]).
//!
//! Dependencies: `egui`, `gcodekit5-app` (jog step presets, axes, translations).

use gcodekit5_app::i18n::Translator;
use gcodekit5_app::machine::Axis;
use gcodekit5_app::machine::grbl_stream::jog_step_choices;
use gcodekit5_core::units::MeasurementSystem;

use crate::commands::Command;
use crate::machine::MachineStatus;

/// The msgids this section shows (the button labels are the jog commands' labels).
pub(super) const MSGIDS: &[&str] = &[n_!("Jog"), n_!("Step"), n_!("Jog Feed:"), n_!("Continuous")];

/// Default step, feed and travel, as `MachineSettings` defaults them.
const DEFAULT_STEP_MM: f32 = 1.0;
const DEFAULT_FEED_MM_MIN: f32 = 1000.0;
const DEFAULT_TRAVEL_MM: [f32; 3] = [200.0, 200.0, 100.0];

/// What the section asks the controller to do.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(super) enum Action {
    /// A relative `$J` move.
    Move {
        /// The axis to move.
        axis: Axis,
        /// Signed distance in mm.
        distance_mm: f32,
        /// Feed in mm/min.
        feed_mm_min: f32,
    },
    /// Jog cancel (0x85).
    Cancel,
}

/// The jog settings and the hold in progress.
#[derive(Debug)]
pub(super) struct JogPanel {
    step_mm: f32,
    feed_mm_min: f32,
    travel_mm: [f32; 3],
    continuous: bool,
    /// The button held in continuous mode.
    held: Option<Command>,
}

impl Default for JogPanel {
    fn default() -> Self {
        Self::new(DEFAULT_STEP_MM, DEFAULT_FEED_MM_MIN, DEFAULT_TRAVEL_MM)
    }
}

/// One button of the jog pad.
#[derive(Debug, Clone, Copy)]
struct JogButton {
    /// The command whose label names the button.
    command: Command,
    /// The button's text.
    text: &'static str,
    axis: Axis,
    /// +1 or -1.
    sign: f32,
    /// Index of `axis` in the travel table.
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

impl JogPanel {
    /// A panel jogging by `step_mm` at `feed_mm_min` within `travel_mm`. Values that are not
    /// positive fall back to the defaults.
    pub(super) fn new(step_mm: f32, feed_mm_min: f32, travel_mm: [f32; 3]) -> Self {
        let positive = |v: f32, default: f32| if v.is_finite() && v > 0.0 { v } else { default };
        Self {
            step_mm: positive(step_mm, DEFAULT_STEP_MM),
            feed_mm_min: positive(feed_mm_min, DEFAULT_FEED_MM_MIN),
            travel_mm: [0, 1, 2].map(|i| positive(travel_mm[i], DEFAULT_TRAVEL_MM[i])),
            continuous: false,
            held: None,
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

    /// Forgets the hold in progress (the controller was reset, which ends the jog).
    pub(super) fn forget_hold(&mut self) {
        self.held = None;
    }

    /// Draws one jog button and returns what it asks for.
    fn button(
        &mut self,
        ui: &mut egui::Ui,
        tr: &Translator,
        button: JogButton,
        enabled: bool,
        actions: &mut Vec<Action>,
    ) {
        let JogButton {
            command,
            text,
            axis,
            sign,
            index,
        } = button;
        let label = tr.tr(command.label_msgid()).into_owned();
        let response = ui
            .add_enabled(
                enabled,
                egui::Button::new(egui::RichText::new(text).strong())
                    .min_size(egui::vec2(44.0, 36.0)),
            )
            .on_hover_text(label.clone());
        response.widget_info(|| egui::WidgetInfo::labeled(egui::Role::Button, enabled, &label));
        if self.continuous {
            let down = response.is_pointer_button_down_on();
            if down && self.held.is_none() {
                self.held = Some(command);
                actions.push(Action::Move {
                    axis,
                    distance_mm: sign * self.travel_mm[index],
                    feed_mm_min: self.feed_mm_min,
                });
            } else if !down && self.held == Some(command) {
                self.held = None;
                actions.push(Action::Cancel);
            }
        } else if response.clicked() {
            actions.push(Action::Move {
                axis,
                distance_mm: sign * self.step_mm,
                feed_mm_min: self.feed_mm_min,
            });
        }
    }

    /// Draws the section.
    pub(super) fn show(
        &mut self,
        ui: &mut egui::Ui,
        tr: &Translator,
        status: &MachineStatus,
    ) -> Vec<Action> {
        let mut actions = Vec::new();
        let enabled = status.connected && !status.streaming;
        ui.strong(tr.tr("Jog"));
        ui.horizontal(|ui| {
            let label = ui.label(tr.tr("Step"));
            let (choices, current) = jog_step_choices(MeasurementSystem::Metric, self.step_mm);
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
        ui.horizontal(|ui| {
            let label = ui.label(tr.tr("Jog Feed:"));
            ui.add(
                egui::DragValue::new(&mut self.feed_mm_min)
                    .range(1.0..=100_000.0)
                    .suffix(" mm/min"),
            )
            .labelled_by(label.id);
        });
        let was_continuous = self.continuous;
        ui.checkbox(&mut self.continuous, tr.tr("Continuous"));
        if was_continuous && !self.continuous && self.held.take().is_some() {
            actions.push(Action::Cancel);
        }
        ui.horizontal(|ui| {
            egui::Grid::new("machine-control-jog-xy")
                .spacing([4.0, 4.0])
                .show(ui, |ui| {
                    ui.label("");
                    self.button(ui, tr, Y_POS, enabled, &mut actions);
                    ui.label("");
                    ui.end_row();
                    self.button(ui, tr, X_NEG, enabled, &mut actions);
                    ui.label("");
                    self.button(ui, tr, X_POS, enabled, &mut actions);
                    ui.end_row();
                    ui.label("");
                    self.button(ui, tr, Y_NEG, enabled, &mut actions);
                    ui.label("");
                    ui.end_row();
                });
            ui.add_space(12.0);
            ui.vertical(|ui| {
                self.button(ui, tr, Z_POS, enabled, &mut actions);
                self.button(ui, tr, Z_NEG, enabled, &mut actions);
            });
        });
        actions
    }
}
