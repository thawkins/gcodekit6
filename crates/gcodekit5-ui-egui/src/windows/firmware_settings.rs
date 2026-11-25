//! The Firmware Settings window (GTK's Device Config settings list): the controller's
//! settings, filtered by search text and category, read from the controller (Retrieve),
//! edited one at a time, and saved to or loaded from a JSON file.
//!
//! - **Retrieve** loads the GRBL (or, when the controller said it runs Repetier, the
//!   Repetier) definitions and sends `$$` then `$I` (`M205` then `M115`). The identity
//!   reply follows the dump, so it marks the dump complete; after 5 s the retrieval ends
//!   as incomplete.
//! - `$n=v` lines update the list whenever they arrive (a `$$` typed in the console too);
//!   the GRBL definitions are loaded first when the list is empty.
//! - Clicking a writable row opens its editor; **Apply** writes `$n=v` (or Repetier's
//!   `M206`) through [`PendingWrite`], which confirms it by reading the settings back.
//! - Every command is refused, with a notice, while disconnected, while a job runs or while
//!   the machine is not Idle ([`setting_write::blocked`]).
//! - **Save** and **Load** ask the app for a file dialog ([`FileRequest`]); the app hands
//!   the answer to [`FirmwareSettingsWindow::save_to`] or
//!   [`FirmwareSettingsWindow::load_from`]. Loading only fills the list (as in GTK, whose
//!   Restore-to-device stayed hidden).
//!
//! Dependencies: `egui`, `gcodekit5-app` (settings model, controller, translations),
//! `gcodekit5-communication` (`SettingsManager`).

use std::path::Path;
use std::time::{Duration, Instant};

use gcodekit5_app::device_identity::DeviceIdentity;
use gcodekit5_app::firmware_settings::{
    ConfigSettingRow, REPETIER_OFFSET, SETTING_CATEGORIES, SettingsBlocked, ValueError,
    apply_value, check_value, filtered_rows, grbl_definitions, load_definitions,
    parse_repetier_epr_line, repetier_definitions, setting_id, write_command,
};
use gcodekit5_app::i18n::Translator;
use gcodekit5_app::machine::MachineEvent;
use gcodekit5_communication::firmware::grbl::settings::SettingsManager;

use super::setting_write::{self, PendingWrite, WriteOutcome};
use crate::machine::Machine;

/// How long a retrieval waits for the identity reply that ends the dump.
pub const RETRIEVE_TIMEOUT: Duration = Duration::from_secs(5);

/// The file name the Save dialog suggests.
pub const EXPORT_FILE_NAME: &str = "grbl_settings.json";

/// A file dialog the window asks the app for.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FileRequest {
    /// Choose where to save the list.
    Save,
    /// Choose a file to load into the list.
    Load,
}

/// A Retrieve waiting for the controller's dump.
#[derive(Debug, Clone)]
struct Retrieval {
    started: Instant,
    repetier: bool,
    /// Settings received so far.
    count: usize,
    /// The identity reply that follows the dump was received.
    identity_seen: bool,
}

/// The row being edited.
#[derive(Debug, Clone)]
struct Edit {
    number: u16,
    value: String,
}

/// A write waiting for the controller's answer.
#[derive(Debug, Clone)]
struct Write {
    pending: PendingWrite,
    number: u16,
    value: String,
}

/// The Firmware Settings window.
#[derive(Debug)]
pub struct FirmwareSettingsWindow {
    open: bool,
    table: SettingsManager,
    search: String,
    category: &'static str,
    /// The latest notice and whether it reports a failure.
    notice: Option<(String, bool)>,
    retrieval: Option<Retrieval>,
    edit: Option<Edit>,
    write: Option<Write>,
    file_request: Option<FileRequest>,
}

impl Default for FirmwareSettingsWindow {
    fn default() -> Self {
        Self {
            open: false,
            table: SettingsManager::new(),
            search: String::new(),
            category: "All",
            notice: None,
            retrieval: None,
            edit: None,
            write: None,
            file_request: None,
        }
    }
}

impl FirmwareSettingsWindow {
    /// Opens the window.
    pub fn open(&mut self) {
        self.open = true;
    }

    /// Whether the window is open.
    pub fn is_open(&self) -> bool {
        self.open
    }

    /// The rows the list shows for the current search text and category.
    pub fn visible_rows(&self) -> Vec<ConfigSettingRow> {
        filtered_rows(&self.table, &self.search, self.category)
    }

    /// The latest notice (a refusal, a write's answer, a retrieval's result), if any.
    pub fn notice(&self) -> Option<&str> {
        self.notice.as_ref().map(|(text, _)| text.as_str())
    }

    /// Takes the file dialog the window asked for, if any.
    pub fn take_file_request(&mut self) -> Option<FileRequest> {
        self.file_request.take()
    }

    fn say(&mut self, text: String, error: bool) {
        self.notice = Some((text, error));
    }

    /// Takes the controller's events, in order, whether the window is open or not.
    ///
    /// # Arguments
    /// * `events` - Events no observer has seen yet.
    /// * `tr` - Translator for the notices.
    pub fn machine_events(&mut self, events: &[MachineEvent], tr: &Translator) {
        for event in events {
            match event {
                MachineEvent::Received(line) => self.received(line.trim()),
                MachineEvent::Disconnected => {
                    self.retrieval = None;
                    self.say(tr.tr(n_!("Not connected")).into_owned(), false);
                }
                MachineEvent::Connected
                | MachineEvent::StatusReport(_)
                | MachineEvent::StreamProgress { .. }
                | MachineEvent::StreamHalted { .. }
                | MachineEvent::StreamFinished { .. }
                | MachineEvent::StreamStopped { .. }
                | MachineEvent::Sent(_)
                | MachineEvent::Error(_) => {}
            }
            if let Some(write) = self.write.as_mut()
                && let Some(outcome) = write.pending.observe(event)
            {
                self.finish_write(outcome, tr);
            }
        }
        self.tick(tr);
    }

    /// Takes one received line: a setting of the dump, or the identity reply ending it.
    fn received(&mut self, line: &str) {
        if let Some(retrieval) = self.retrieval.as_mut()
            && retrieval.repetier
        {
            if let Some(setting) = parse_repetier_epr_line(line) {
                self.table.set_setting(setting);
                retrieval.count += 1;
            }
            retrieval.identity_seen |= line.contains("FIRMWARE_NAME:");
            return;
        }
        if let Some((number, value)) = SettingsManager::parse_setting_line(line) {
            if self.table.count() == 0 {
                load_definitions(&mut self.table, grbl_definitions());
            }
            if apply_value(&mut self.table, number, &value)
                && let Some(retrieval) = self.retrieval.as_mut()
            {
                retrieval.count += 1;
            }
        } else if line.starts_with("[VER:")
            && let Some(retrieval) = self.retrieval.as_mut()
        {
            retrieval.identity_seen = true;
        }
    }

    /// Ends a retrieval whose identity reply arrived or whose time ran out, and a write
    /// whose answer is overdue.
    fn tick(&mut self, tr: &Translator) {
        if let Some(retrieval) = &self.retrieval {
            let complete = retrieval.identity_seen;
            if complete || retrieval.started.elapsed() > RETRIEVE_TIMEOUT {
                let count = retrieval.count;
                self.retrieval = None;
                self.finish_retrieval(complete, count, tr);
            }
        }
        if self.write.as_ref().is_some_and(|w| w.pending.timed_out()) {
            self.finish_write(WriteOutcome::NoReply, tr);
        }
    }

    fn finish_retrieval(&mut self, complete: bool, count: usize, tr: &Translator) {
        if count == 0 {
            self.table.clear();
            let text = n_!("Protocol mismatch: No valid parameters received from device");
            self.say(tr.tr(text).into_owned(), true);
            return;
        }
        let text = if complete {
            n_!("Settings retrieved from device:")
        } else {
            n_!("Settings retrieval incomplete (timed out), settings received:")
        };
        self.say(format!("{} {count}", tr.tr(text)), !complete);
    }

    fn finish_write(&mut self, outcome: WriteOutcome, tr: &Translator) {
        let Some(write) = self.write.take() else {
            return;
        };
        let line = write.pending.line().to_string();
        let failure = match outcome {
            WriteOutcome::Accepted => {
                // A GRBL value comes back in the `$$` dump; a Repetier write is shown as sent.
                if write.number >= REPETIER_OFFSET {
                    apply_value(&mut self.table, write.number, &write.value);
                }
                self.say(format!("{} {line}", tr.tr(n_!("Setting written:"))), false);
                return;
            }
            WriteOutcome::Rejected(reply) => match reply.strip_prefix("error:") {
                Some(_) => format!("{} {reply}", tr.tr(n_!("the controller replied"))),
                None => crate::machine::worker_message(&reply, tr),
            },
            WriteOutcome::NoReply => tr.tr(n_!("no reply from the controller")).into_owned(),
        };
        self.refuse_write(&line, &failure, tr);
    }

    fn refuse_write(&mut self, line: &str, reason: &str, tr: &Translator) {
        let title = tr.tr(n_!("Setting not changed"));
        self.say(format!("{title}: {line}: {reason}"), true);
    }

    /// Reads every setting from the controller.
    fn retrieve(&mut self, machine: &Machine, identity: &DeviceIdentity, tr: &Translator) {
        match setting_write::blocked(machine) {
            Some(SettingsBlocked::NotConnected) => {
                let text = n_!("Please connect the machine before retrieving parameters.");
                self.say(tr.tr(text).into_owned(), true);
                return;
            }
            Some(blocked) => {
                let reason = setting_write::blocked_text(&blocked, tr);
                let title = tr.tr(n_!("Cannot retrieve settings:"));
                self.say(format!("{title} {reason}"), true);
                return;
            }
            None => {}
        }
        let repetier = identity.is_repetier();
        let (definitions, lines, failure) = if repetier {
            (
                repetier_definitions(),
                ["M205", "M115"],
                n_!("Error sending M205:"),
            )
        } else {
            (grbl_definitions(), ["$$", "$I"], n_!("Error sending $$:"))
        };
        if let Err(reason) = setting_write::send_lines(machine, &lines, tr) {
            self.say(format!("{} {reason}", tr.tr(failure)), true);
            return;
        }
        load_definitions(&mut self.table, definitions);
        self.retrieval = Some(Retrieval {
            started: Instant::now(),
            repetier,
            count: 0,
            identity_seen: false,
        });
        let text = n_!("Retrieving settings and device info...");
        self.say(tr.tr(text).into_owned(), false);
    }

    /// Writes the edited value.
    fn apply_edit(&mut self, machine: &Machine, tr: &Translator) {
        let Some(edit) = self.edit.take() else {
            return;
        };
        let Some(setting) = self.table.get_setting(edit.number) else {
            return;
        };
        // A Repetier position is written as the type the controller reported for it.
        let eeprom_type = setting.eeprom_type;
        let value = match check_value(setting, &edit.value) {
            Ok(value) => value,
            Err(error) => {
                let reason = tr.tr(error.msgid());
                self.refuse_write(&setting_id(edit.number), &reason, tr);
                return;
            }
        };
        let Some(line) = write_command(edit.number, eeprom_type, &value) else {
            let reason = tr.tr(n_!("unknown EEPROM value type"));
            self.refuse_write(&setting_id(edit.number), &reason, tr);
            return;
        };
        if let Some(blocked) = setting_write::blocked(machine) {
            let reason = setting_write::blocked_text(&blocked, tr);
            self.refuse_write(&line, &reason, tr);
            return;
        }
        let confirm = edit.number < REPETIER_OFFSET;
        match PendingWrite::send(machine, &line, confirm, tr) {
            Ok(pending) => {
                self.notice = None;
                self.write = Some(Write {
                    pending,
                    number: edit.number,
                    value,
                });
            }
            Err(reason) => self.refuse_write(&line, &reason, tr),
        }
    }

    /// Saves the list to `path` as JSON.
    ///
    /// # Arguments
    /// * `path` - The file the Save dialog chose.
    /// * `tr` - Translator for the notice.
    pub fn save_to(&mut self, path: &Path, tr: &Translator) {
        match self.table.export_to_file(path) {
            Ok(()) => {
                let title = tr.tr(n_!("Exported settings to"));
                self.say(format!("{title} {}", path.display()), false);
            }
            Err(e) => self.say(format!("{} {e}", tr.tr(n_!("Export failed:"))), true),
        }
    }

    /// Replaces the list with the settings saved in `path`.
    ///
    /// # Arguments
    /// * `path` - The file the Load dialog chose.
    /// * `tr` - Translator for the notice.
    pub fn load_from(&mut self, path: &Path, tr: &Translator) {
        match self.table.import_from_file(path) {
            Ok(()) => {
                let title = tr.tr(n_!("Imported settings from"));
                self.say(format!("{title} {}", path.display()), false);
            }
            Err(e) => self.say(format!("{} {e}", tr.tr(n_!("Import failed:"))), true),
        }
    }

    /// Draws the window (and the open editor) while it is open.
    ///
    /// # Arguments
    /// * `ctx` - The egui context.
    /// * `machine` - The app's machine connection, for reading and writing settings.
    /// * `identity` - What the controller reported about itself (Repetier or GRBL).
    /// * `tr` - Translates the labels.
    pub fn show(
        &mut self,
        ctx: &egui::Context,
        machine: &Machine,
        identity: &DeviceIdentity,
        tr: &Translator,
    ) {
        self.tick(tr);
        if let Some(write) = &self.write {
            ctx.request_repaint_after(write.pending.time_left());
        }
        if self.retrieval.is_some() {
            ctx.request_repaint_after(Duration::from_millis(250));
        }
        if !self.open {
            return;
        }
        let mut open = self.open;
        egui::Window::new(tr.tr(n_!("Firmware Settings")))
            .id(egui::Id::unique("gcodekit5-firmware-settings"))
            .open(&mut open)
            .default_size([900.0, 560.0])
            .show(ctx, |ui| self.contents(ui, machine, identity, tr));
        self.open = open;
        if !self.open {
            self.edit = None;
        }
        self.show_editor(ctx, machine, tr);
    }

    fn contents(
        &mut self,
        ui: &mut egui::Ui,
        machine: &Machine,
        identity: &DeviceIdentity,
        tr: &Translator,
    ) {
        let busy = self.retrieval.is_some() || self.write.is_some();
        ui.horizontal(|ui| {
            let retrieve = ui
                .add_enabled(!busy, egui::Button::new(tr.tr(n_!("Retrieve"))))
                .on_hover_text(tr.tr(n_!("Retrieve Settings from Device ($$)")));
            if retrieve.clicked() {
                self.retrieve(machine, identity, tr);
            }
            let save = ui
                .add_enabled(
                    self.table.count() > 0,
                    egui::Button::new(tr.tr(n_!("Save"))),
                )
                .on_hover_text(tr.tr(n_!("Save Settings to File")));
            if save.clicked() {
                self.file_request = Some(FileRequest::Save);
            }
            let load = ui
                .button(tr.tr(n_!("Load")))
                .on_hover_text(tr.tr(n_!("Load Settings from File")));
            if load.clicked() {
                self.file_request = Some(FileRequest::Load);
            }
        });
        ui.horizontal(|ui| {
            let label = ui.label(tr.tr(n_!("Filter:")));
            ui.add(
                egui::TextEdit::singleline(&mut self.search)
                    .hint_text(tr.tr(n_!("Search settings...")))
                    .desired_width(240.0),
            )
            .labelled_by(label.id);
            let label = ui.label(tr.tr(n_!("Category:")));
            egui::ComboBox::from_id_salt("gcodekit5-firmware-settings-category")
                .selected_text(tr.tr(self.category))
                .show_ui(ui, |ui| {
                    for &category in SETTING_CATEGORIES {
                        ui.selectable_value(&mut self.category, category, tr.tr(category));
                    }
                })
                .response
                .labelled_by(label.id);
        });
        let rows = self.visible_rows();
        ui.horizontal(|ui| {
            match &self.notice {
                Some((text, true)) => ui.colored_label(ui.visuals().error_fg_color, text),
                Some((text, false)) => ui.label(text),
                None if !machine.status().connected => ui.label(tr.tr(n_!("Not connected"))),
                None => ui.label(""),
            };
            let count = format!(
                "{} / {} {}",
                rows.len(),
                self.table.count(),
                tr.tr(n_!("settings"))
            );
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                ui.weak(count);
            });
        });
        ui.separator();
        egui::ScrollArea::both()
            .auto_shrink([false, false])
            .show(ui, |ui| self.list(ui, &rows, tr));
    }

    fn list(&mut self, ui: &mut egui::Ui, rows: &[ConfigSettingRow], tr: &Translator) {
        egui::Grid::new("gcodekit5-firmware-settings-list")
            .striped(true)
            .num_columns(6)
            .show(ui, |ui| {
                for heading in [
                    n_!("ID"),
                    n_!("Name"),
                    n_!("Value"),
                    n_!("Unit"),
                    n_!("Category"),
                    n_!("Description"),
                ] {
                    ui.strong(tr.tr(heading));
                }
                ui.end_row();
                for row in rows {
                    if row.read_only {
                        ui.weak(row.id());
                    } else if ui.selectable_label(false, row.id()).clicked() {
                        self.edit = Some(Edit {
                            number: row.number,
                            value: row.value.clone(),
                        });
                    }
                    ui.label(&row.name);
                    ui.label(&row.value);
                    ui.weak(&row.unit);
                    ui.weak(tr.tr(&row.category));
                    ui.add(
                        egui::Label::new(egui::RichText::new(&row.description).weak()).truncate(),
                    );
                    ui.end_row();
                }
            });
    }

    /// Draws the editor of the clicked row while it is open.
    fn show_editor(&mut self, ctx: &egui::Context, machine: &Machine, tr: &Translator) {
        // One command at a time: a write's answer is told by the order of the replies.
        let can_apply = self.write.is_none() && self.retrieval.is_none();
        let Some(edit) = self.edit.as_mut() else {
            return;
        };
        let Some(setting) = self.table.get_setting(edit.number).cloned() else {
            self.edit = None;
            return;
        };
        let title = if edit.number >= REPETIER_OFFSET {
            format!(
                "{} {}",
                tr.tr(n_!("Edit EEPROM Pos")),
                edit.number - REPETIER_OFFSET
            )
        } else {
            format!("{} {}", tr.tr(n_!("Edit Setting")), setting_id(edit.number))
        };
        let (mut apply, mut cancel) = (false, false);
        egui::Window::new(title)
            .id(egui::Id::unique("gcodekit5-firmware-settings-edit"))
            // Above the list it edits, which a click would otherwise raise over it.
            .order(egui::Order::Foreground)
            .collapsible(false)
            .resizable(false)
            .show(ctx, |ui| {
                egui::Grid::new("gcodekit5-firmware-settings-edit-grid")
                    .num_columns(3)
                    .show(ui, |ui| {
                        ui.strong(tr.tr(n_!("Name:")));
                        ui.label(&setting.name);
                        ui.end_row();
                        ui.strong(tr.tr(n_!("Description:")));
                        ui.weak(&setting.description);
                        ui.end_row();
                        let label = ui.strong(tr.tr(n_!("Value:")));
                        ui.add(egui::TextEdit::singleline(&mut edit.value))
                            .labelled_by(label.id);
                        ui.weak(setting.unit.as_deref().unwrap_or(""));
                        ui.end_row();
                    });
                ui.horizontal(|ui| {
                    cancel = ui.button(tr.tr(n_!("Cancel"))).clicked();
                    apply = ui
                        .add_enabled(can_apply, egui::Button::new(tr.tr(n_!("Apply"))))
                        .clicked();
                });
            });
        if apply {
            self.apply_edit(machine, tr);
        } else if cancel {
            self.edit = None;
        }
    }
}

/// The English msgids this window shows, for the translation test.
pub fn msgids() -> Vec<&'static str> {
    let mut ids = vec![
        n_!("Firmware Settings"),
        n_!("Retrieve"),
        n_!("Retrieve Settings from Device ($$)"),
        n_!("Save"),
        n_!("Save Settings to File"),
        n_!("Load"),
        n_!("Load Settings from File"),
        n_!("Filter:"),
        n_!("Search settings..."),
        n_!("Category:"),
        n_!("Not connected"),
        n_!("settings"),
        n_!("ID"),
        n_!("Name"),
        n_!("Value"),
        n_!("Unit"),
        n_!("Category"),
        n_!("Description"),
        n_!("Protocol mismatch: No valid parameters received from device"),
        n_!("Settings retrieved from device:"),
        n_!("Settings retrieval incomplete (timed out), settings received:"),
        n_!("Setting written:"),
        n_!("the controller replied"),
        n_!("no reply from the controller"),
        n_!("Setting not changed"),
        n_!("Please connect the machine before retrieving parameters."),
        n_!("Cannot retrieve settings:"),
        n_!("Error sending M205:"),
        n_!("Error sending $$:"),
        n_!("Retrieving settings and device info..."),
        n_!("Exported settings to"),
        n_!("Export failed:"),
        n_!("Imported settings from"),
        n_!("Import failed:"),
        n_!("Edit EEPROM Pos"),
        n_!("Edit Setting"),
        n_!("Name:"),
        n_!("Description:"),
        n_!("Value:"),
        n_!("Cancel"),
        n_!("Apply"),
        n_!("unknown EEPROM value type"),
    ];
    ids.extend(
        [
            ValueError::Empty,
            ValueError::NotANumber,
            ValueError::NotAWholeNumber,
            ValueError::OutOfRange,
        ]
        .map(ValueError::msgid),
    );
    ids.extend(SETTING_CATEGORIES);
    ids.extend(setting_write::msgids());
    ids
}
