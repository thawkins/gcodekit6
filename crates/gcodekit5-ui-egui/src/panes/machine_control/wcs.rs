//! Work coordinates: the G54-G59 selector, Zero All Axes and Go to Work Zero (with its
//! Include Z option), and keeping the active system known.
//!
//! The active system is the controller's: [`MachineStatus::active_wcs`] reads GRBL's
//! `[GC:...]` reply to `$G` and any G54-G59 line sent. The section asks `$G` after a
//! connect, a controller reset, every G54-G59 line sent (from here or the console) and the end
//! of a stream whose program may select a system (one not seen starting counts as such), and
//! holds the zero buttons until the answer is in, so a `G10 L20 P<n>` never names a system
//! other than the one shown. `$G` is sent as a query (`MachineController::send_query`): it
//! does not hold up jogging. It goes out from the pane's per-frame work, which runs whether
//! or not the Machine Control page is shown.
//! Only a controller that never reports its parser state falls back to the user's last
//! selection (G54 at first), as the GTK front end did.
//!
//! A system outside G54-G59 (grblHAL's `G59.1`, a typed `G54.1`) leaves the zero commands
//! without a `P` that names it, so they stay disabled until a G54-G59 is active again. `$G`
//! is a GRBL command: with a device profile for another controller family the work zero
//! controls stay disabled and say why.
//!
//! Ported from the GTK `machine_control` view's Work Coordinates section and zero buttons.
//!
//! Dependencies: `egui`, `gcodekit5-app` (work zero commands, controller, translations).

use gcodekit5_app::i18n::Translator;
use gcodekit5_app::machine::grbl_stream::{GrblResponseLine, classify_response};
use gcodekit5_app::machine::{MachineError, MachineEvent};
use gcodekit5_app::work_zero::{PARSER_STATE_QUERY, Wcs, WcsWord, wcs_in_line};

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
    WCS_OUTSIDE,
    NOT_GRBL,
];

/// Shown on the zero buttons while the active system is not known.
pub(super) const WAITING_FOR_WCS: &str =
    n_!("Waiting for the controller to report the active work coordinate system.");
/// Shown on the zero buttons while the active system is outside G54-G59.
pub(super) const WCS_OUTSIDE: &str = n_!("Active work coordinate system is not G54-G59.");
/// Shown on the work zero controls for a controller outside the GRBL family.
pub(super) const NOT_GRBL: &str =
    n_!("Work zero needs a GRBL-family controller (set in the device profile).");

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
    /// The last G54-G59 sent on this connection (G54 until one is): the fallback for a
    /// controller that does not report its parser state.
    selected: Wcs,
    /// Whether the controller is GRBL-family (answers `$G`); see [`WcsPanel::set_grbl`].
    not_grbl: bool,
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
        if self.not_grbl || status.wcs_outside {
            return None;
        }
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

    /// Why the zero commands (and Go to Work Zero) cannot run now, as a msgid; `None` when
    /// they can. Offline is not a reason: the controls are simply disabled.
    pub(super) fn blocked(&self, status: &MachineStatus) -> Option<&'static str> {
        if self.not_grbl {
            Some(NOT_GRBL)
        } else if status.wcs_outside {
            Some(WCS_OUTSIDE)
        } else if self.active(status).is_none() {
            Some(WAITING_FOR_WCS)
        } else {
            None
        }
    }

    /// Takes whether the controller is GRBL-family (Grbl, grblHAL, FluidNC), from the active
    /// device profile; `$G` is asked only of those.
    ///
    /// # Arguments
    /// * `grbl` - Whether it is.
    pub(super) fn set_grbl(&mut self, grbl: bool) {
        self.not_grbl = !grbl;
    }

    /// Notes, once per frame (drawn or not) while a stream runs, whether its program may select a system:
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
                    self.selected = Wcs::default();
                }
                MachineEvent::Sent(line) => match wcs_in_line(line) {
                    Some(WcsWord::Known(wcs)) => {
                        self.selected = wcs;
                        self.query = Query::Wanted;
                    }
                    // Unknown to the zero commands until the controller reports again; a
                    // reply that still names it does not ask again.
                    Some(WcsWord::Other) => self.query = Query::Wanted,
                    None => {}
                },
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
            Query::Wanted if self.not_grbl => self.query = Query::Settled,
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
    pub(super) fn show(
        &mut self,
        ui: &mut egui::Ui,
        tr: &Translator,
        status: &MachineStatus,
    ) -> Option<Action> {
        let connected = status.connected;
        let blocked = self.blocked(status);
        let zero_enabled = connected && blocked.is_none();
        let reason = blocked.map(|msgid| tr.tr(msgid));
        let selector_reason = self.not_grbl.then(|| tr.tr(NOT_GRBL));
        let shown = self.shown(status);
        let mut action = None;
        ui.strong(tr.tr("Work Coordinates"));
        egui::Grid::new("wcs-selector")
            .num_columns(3)
            .spacing([6.0, 6.0])
            .show(ui, |ui| {
                for (i, wcs) in Wcs::ALL.into_iter().enumerate() {
                    let button = egui::Button::selectable(shown == Some(wcs), wcs.code());
                    let response = ui.add_enabled(connected && !self.not_grbl, button);
                    let response = match &selector_reason {
                        Some(reason) if connected => {
                            response.on_disabled_hover_text(reason.as_ref())
                        }
                        Some(_) | None => response,
                    };
                    if response.clicked() && shown != Some(wcs) {
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
            let zero_all = match &reason {
                Some(reason) if connected => zero_all.on_disabled_hover_text(reason.as_ref()),
                Some(_) | None => zero_all,
            };
            if zero_all.clicked() {
                action = Some(Action::ZeroAll);
            }
            // It rapids to the active system's zero: only while that is the one shown.
            let go = ui
                .add_enabled(zero_enabled, egui::Button::new(tr.tr("Go to Work Zero")))
                .on_hover_text(
                    tr.tr("Rapid move to work origin (X/Y, or X/Y/Z if Include Z is checked)"),
                );
            let go = match &reason {
                Some(reason) if connected => go.on_disabled_hover_text(reason.as_ref()),
                Some(_) | None => go,
            };
            if go.clicked() {
                action = Some(Action::GoToZero {
                    include_z: self.include_z,
                });
            }
            let include_z = self.include_z;
            crate::theme::checked_scope(ui, include_z, |ui| {
                ui.add_enabled(
                    connected,
                    egui::Checkbox::new(&mut self.include_z, tr.tr("Include Z")),
                )
            })
            .on_hover_text(tr.tr("When checked, Go to Work Zero will also move to Z0"));
        });
        action
    }
}
