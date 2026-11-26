//! Work coordinates: the G54-G59 selector, Zero All Axes and Go to Work Zero (with its
//! Include Z option), and keeping the active system known.
//!
//! The active system is the controller's: [`MachineStatus::active_wcs`] reads GRBL's
//! `[GC:...]` reply to `$G` and any G54-G59 line sent. The section asks `$G` after a
//! connect, a controller reset, every G54-G59 line sent (from here or the console) and the end
//! of a stream whose program may select a system (one not seen starting counts as such), and
//! holds the zero buttons until the answer is in, so a `G10 L20 P<n>` never names a system
//! other than the one shown. `$G` is sent as a query (`MachineController::send_query`): it
//! does not hold up jogging. It goes out while the pane is drawn, where its buttons are. Only a controller that never reports its parser
//! state falls back to the user's last selection (G54 at first), as the GTK front end did.
//!
//! Ported from the GTK `machine_control` view's Work Coordinates section and zero buttons.
//!
//! Dependencies: `egui`, `gcodekit5-app` (work zero commands, controller, translations).

use gcodekit5_app::i18n::Translator;
use gcodekit5_app::machine::grbl_stream::{GrblResponseLine, classify_response};
use gcodekit5_app::machine::{MachineError, MachineEvent};
use gcodekit5_app::work_zero::{PARSER_STATE_QUERY, Wcs, wcs_in_line};

use crate::machine::{Machine, MachineStatus, lock};

/// The msgids this section shows.
pub(super) const MSGIDS: &[&str] = &[
    n_!("Work Coordinates"),
    n_!("Zero All Axes"),
    n_!("Set active work position to 0 for X/Y/Z"),
    n_!("Go to Work Zero"),
    n_!("Rapid move to work origin (X/Y, or X/Y/Z if Include Z is checked)"),
    n_!("Include Z"),
    n_!("When checked, Go to Work Zero will also move to Z0"),
    WAITING_FOR_WCS,
];

/// Shown on the zero buttons while the active system is not known.
pub(super) const WAITING_FOR_WCS: &str =
    n_!("Waiting for the controller to report the active work coordinate system.");

/// What the user asked for in the section.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum Action {
    /// Select a work coordinate system.
    Select(Wcs),
    /// Zero X, Y and Z in the active system (after confirmation).
    ZeroAll,
    /// Rapid to work zero; Z too when `include_z`.
    GoToZero {
        /// Whether Z moves to 0 too.
        include_z: bool,
    },
}

/// Where the `$G` that refreshes the active system stands.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
enum Query {
    /// Nothing to ask: the status's system is current.
    #[default]
    Settled,
    /// The system may have changed; `$G` goes out once every earlier line is answered.
    Wanted,
    /// `$G` is out, sent after the machine's poll number `after`; settled once a later
    /// poll finds it answered (its `[GC:...]` reply, read before the `ok`, is then among the
    /// events that poll took).
    Sent {
        /// [`Machine::polls`] when it was sent.
        after: u64,
    },
}

/// The section's state.
#[derive(Debug, Default)]
pub(super) struct WcsPanel {
    /// The system the user last selected (or the controller last reported): the fallback
    /// for a controller that does not report its parser state.
    selected: Wcs,
    /// Whether Go to Work Zero moves Z too; off by default, a Z0 rapid can crash a tool.
    include_z: bool,
    query: Query,
    /// For the stream in progress: whether its program may select a system (`None` when no
    /// stream was seen starting).
    stream_selects_wcs: Option<bool>,
}

impl WcsPanel {
    /// The system the section shows as active.
    pub(super) fn shown(&self, status: &MachineStatus) -> Option<Wcs> {
        match status.active_wcs {
            Some(wcs) => Some(wcs),
            None if !status.wcs_reported => Some(self.selected),
            None => None,
        }
    }

    /// The system a zero command acts on: the one shown, once no refresh is outstanding.
    pub(super) fn active(&self, status: &MachineStatus) -> Option<Wcs> {
        if self.query == Query::Settled {
            self.shown(status)
        } else {
            None
        }
    }

    /// Notes, once per frame while a stream runs, whether its program may select a system:
    /// the loaded program (the one Start streams) holds a G54-G59 word.
    ///
    /// # Arguments
    /// * `machine` - The app's machine connection.
    pub(super) fn watch_stream(&mut self, machine: &Machine) {
        if machine.job_running() && self.stream_selects_wcs.is_none() {
            let selects = machine
                .program()
                .is_some_and(|lines| lines.iter().any(|l| wcs_in_line(l).is_some()));
            self.stream_selects_wcs = Some(selects);
        }
    }

    /// Notes the events that may change the active system.
    ///
    /// # Arguments
    /// * `events` - Machine events, oldest first.
    pub(super) fn follow(&mut self, events: &[MachineEvent]) {
        for event in events {
            match event {
                MachineEvent::Connected => self.query = Query::Wanted,
                MachineEvent::StreamFinished { .. } | MachineEvent::StreamStopped { .. } => {
                    // A stream not seen starting may have selected one: ask.
                    if self.stream_selects_wcs.take() != Some(false) {
                        self.query = Query::Wanted;
                    }
                }
                MachineEvent::Disconnected => {
                    self.query = Query::Settled;
                    self.stream_selects_wcs = None;
                }
                MachineEvent::Sent(line) => {
                    if let Some(wcs) = wcs_in_line(line) {
                        self.selected = wcs;
                        self.query = Query::Wanted;
                    }
                }
                MachineEvent::Received(line) => {
                    if matches!(classify_response(line), GrblResponseLine::Reset(_)) {
                        self.query = Query::Wanted;
                    }
                }
                MachineEvent::StatusReport(_)
                | MachineEvent::StreamProgress { .. }
                | MachineEvent::StreamHalted { .. }
                | MachineEvent::Error(_) => {}
            }
        }
    }

    /// Sends `$G` when a refresh is wanted and the controller is free (no stream, every
    /// earlier line answered, so the reply cannot be taken for another line's), and settles
    /// once a later poll finds it answered.
    ///
    /// # Arguments
    /// * `machine` - The app's machine connection.
    pub(super) fn drive(&mut self, machine: &Machine) {
        match self.query {
            Query::Settled => {}
            Query::Wanted => {
                let mut controller = lock(machine.controller());
                if controller.is_streaming() || !controller.lines_answered() {
                    return;
                }
                // A query: it leaves the latest report fresh, so jogging is not held up.
                match controller.send_query(PARSER_STATE_QUERY) {
                    Ok(()) => {
                        self.query = Query::Sent {
                            after: machine.polls(),
                        };
                    }
                    Err(MachineError::NotConnected) => self.query = Query::Settled,
                    Err(error) => tracing::debug!(%error, "parser state query deferred"),
                }
            }
            Query::Sent { after } => {
                if machine.polls() > after && machine.lines_answered_at_poll() {
                    self.query = Query::Settled;
                }
            }
        }
    }

    /// Draws the section; returns what the user asked for.
    ///
    /// # Arguments
    /// * `ui` - The pane's `Ui`.
    /// * `tr` - Translator for the current UI language.
    /// * `status` - The machine status.
    /// * `zero_enabled` - Whether Zero All Axes is enabled.
    pub(super) fn show(
        &mut self,
        ui: &mut egui::Ui,
        tr: &Translator,
        status: &MachineStatus,
        zero_enabled: bool,
    ) -> Option<Action> {
        let connected = status.connected;
        let shown = self.shown(status);
        let mut action = None;
        ui.strong(tr.tr("Work Coordinates"));
        egui::Grid::new("wcs-selector")
            .num_columns(3)
            .spacing([6.0, 6.0])
            .show(ui, |ui| {
                for (i, wcs) in Wcs::ALL.into_iter().enumerate() {
                    let button = egui::Button::selectable(shown == Some(wcs), wcs.code());
                    if ui.add_enabled(connected, button).clicked() && shown != Some(wcs) {
                        action = Some(Action::Select(wcs));
                    }
                    if i % 3 == 2 {
                        ui.end_row();
                    }
                }
            });
        ui.horizontal_wrapped(|ui| {
            let zero_all = ui
                .add_enabled(zero_enabled, egui::Button::new(tr.tr("Zero All Axes")))
                .on_hover_text(tr.tr("Set active work position to 0 for X/Y/Z"));
            let zero_all = if connected {
                zero_all.on_disabled_hover_text(tr.tr(WAITING_FOR_WCS))
            } else {
                zero_all
            };
            if zero_all.clicked() {
                action = Some(Action::ZeroAll);
            }
            if ui
                .add_enabled(connected, egui::Button::new(tr.tr("Go to Work Zero")))
                .on_hover_text(
                    tr.tr("Rapid move to work origin (X/Y, or X/Y/Z if Include Z is checked)"),
                )
                .clicked()
            {
                action = Some(Action::GoToZero {
                    include_z: self.include_z,
                });
            }
            ui.add_enabled(
                connected,
                egui::Checkbox::new(&mut self.include_z, tr.tr("Include Z")),
            )
            .on_hover_text(tr.tr("When checked, Go to Work Zero will also move to Z0"));
        });
        action
    }
}
