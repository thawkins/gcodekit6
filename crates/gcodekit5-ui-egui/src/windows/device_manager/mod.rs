//! The Device Manager window (GTK's Device Manager): the machine profiles, searched and
//! listed by name with the active one marked, and an edit form in four tabs (General,
//! Connection, Dimensions, Capabilities).
//!
//! - Profiles are kept in the GTK build's `devices.json` through `gcodekit5-devicedb`'s
//!   `DeviceManager`, so a profile written by either build loads in the other. An existing
//!   file is read when the app starts; opening the window without one creates the default
//!   profile and makes it active, as GTK does at startup.
//! - **Add Device** stores a new default profile and opens it; **Save** checks the form
//!   with [`ProfileForm::apply`] (GTK's rules and messages); **Cancel** drops the edits;
//!   **Delete** asks first; **Set Active** makes the profile the active one; **Sync from
//!   Device** fills the form from the settings the controller reported in Firmware
//!   Settings (GTK's Sync from Device, `$130`-`$132`, `$30`, `$32`).
//! - The active profile can never be deleted, nor the only one, so the app always keeps
//!   a valid active profile (GTK cleared it). While a job runs the active profile cannot
//!   be changed or replaced.
//!
//! Dependencies: `egui`, `gcodekit5-app` (form rules, translations), `gcodekit5-devicedb`
//! (profiles and their file), `gcodekit5-core` (units).

use std::collections::HashMap;
use std::path::PathBuf;

use gcodekit5_app::device_profiles::{
    self, ProfileForm, ReportedLimits, delete_refusal, matches_search,
};
use gcodekit5_app::i18n::Translator;
use gcodekit5_core::units::{FeedRateUnits, MeasurementSystem};
use gcodekit5_devicedb::{DeviceManager, DeviceProfile};

mod view;

/// The form's tabs.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Tab {
    General,
    Connection,
    Dimensions,
    Capabilities,
}

impl Tab {
    const ALL: [Self; 4] = [
        Self::General,
        Self::Connection,
        Self::Dimensions,
        Self::Capabilities,
    ];

    fn msgid(self) -> &'static str {
        match self {
            Self::General => n_!("General"),
            Self::Connection => n_!("Connection"),
            Self::Dimensions => n_!("Dimensions"),
            Self::Capabilities => n_!("Capabilities"),
        }
    }
}

/// The profile being edited.
#[derive(Debug, Clone)]
struct Edit {
    id: String,
    form: ProfileForm,
    /// The units the form's lengths and feed rate are in.
    units: MeasurementSystem,
    feed_units: FeedRateUnits,
    tab: Tab,
}

/// A delete waiting for the user's answer.
#[derive(Debug, Clone)]
struct ConfirmDelete {
    id: String,
    name: String,
}

/// The Device Manager window.
#[derive(Debug)]
pub struct DeviceManagerWindow {
    open: bool,
    /// The profiles and their file; `None` without a settings folder.
    store: Option<DeviceManager>,
    /// The file was read (or created) on this run.
    loaded: bool,
    /// Why the file could not be read, until the window shows it.
    load_error: Option<String>,
    search: String,
    edit: Option<Edit>,
    confirm_delete: Option<ConfirmDelete>,
    /// The latest notice and whether it reports a failure.
    notice: Option<(String, bool)>,
    /// The active profile or its values changed since [`Self::take_changed`].
    changed: bool,
    /// Sync from Device was clicked; the app answers with [`Self::sync_from`].
    sync_requested: bool,
}

impl DeviceManagerWindow {
    /// The window over the profiles in `file`, read now if it exists (nothing is written).
    ///
    /// # Arguments
    /// * `file` - The profiles file; `None` without a settings folder.
    pub fn new(file: Option<PathBuf>) -> Self {
        let mut loaded = false;
        let mut load_error = None;
        let store = file.map(|file| {
            let exists = file.exists();
            let store = DeviceManager::new(file);
            if exists {
                loaded = true;
                load_error = load(&store);
            }
            store
        });
        Self {
            open: false,
            store,
            loaded,
            load_error,
            search: String::new(),
            edit: None,
            confirm_delete: None,
            notice: None,
            changed: false,
            sync_requested: false,
        }
    }

    /// Opens the window; without a profiles file the default profile is created and made
    /// active (GTK's first start).
    ///
    /// # Arguments
    /// * `tr` - Translator for a notice.
    pub fn open(&mut self, tr: &Translator) {
        self.open = true;
        match &self.store {
            Some(store) if !self.loaded => {
                self.loaded = true;
                self.load_error = load(store);
                self.changed = true;
            }
            Some(_) => {}
            None => {
                let text = n_!("No settings folder: device profiles cannot be loaded or saved.");
                self.say(tr.tr(text).into_owned(), true);
            }
        }
        if let Some(error) = self.load_error.take() {
            let title = tr.tr(n_!("Device profiles could not be read:"));
            self.say(format!("{title} {error}"), true);
        }
    }

    /// Whether the window is open.
    pub fn is_open(&self) -> bool {
        self.open
    }

    /// The profiles, by name.
    pub fn profiles(&self) -> Vec<DeviceProfile> {
        let mut profiles = self
            .store
            .as_ref()
            .map(DeviceManager::get_all_profiles)
            .unwrap_or_default();
        profiles.sort_by(|a, b| a.name.cmp(&b.name).then_with(|| a.id.cmp(&b.id)));
        profiles
    }

    /// The active profile, if one is set.
    pub fn active_profile(&self) -> Option<DeviceProfile> {
        self.store.as_ref()?.get_active_profile()
    }

    /// The latest notice (a refusal, a save's result), if any.
    pub fn notice(&self) -> Option<&str> {
        self.notice.as_ref().map(|(text, _)| text.as_str())
    }

    /// Whether the active profile, or which profile is active, changed since the last call.
    pub fn take_changed(&mut self) -> bool {
        std::mem::take(&mut self.changed)
    }

    /// Whether Sync from Device was clicked since the last call.
    pub fn take_sync_request(&mut self) -> bool {
        std::mem::take(&mut self.sync_requested)
    }

    fn say(&mut self, text: String, error: bool) {
        self.notice = Some((text, error));
    }

    fn active_id(&self) -> Option<String> {
        self.active_profile().map(|p| p.id)
    }

    /// Keeps a GRBL `$$` dump in the active profile (GTK's Device Config page rule).
    ///
    /// # Arguments
    /// * `settings` - The reported settings.
    pub fn store_reported_settings(&mut self, settings: &HashMap<u16, String>) {
        let Some(store) = &self.store else {
            return;
        };
        let Some(mut profile) = store.get_active_profile() else {
            return;
        };
        device_profiles::store_reported_settings(&mut profile, settings);
        match store.save_profile(profile) {
            Ok(()) => self.changed = true,
            Err(error) => tracing::warn!(%error, "reported settings not stored in the profile"),
        }
    }

    /// Opens profile `id` in the form, with lengths in `units` and feed in `feed_units`.
    fn select(&mut self, id: &str, units: MeasurementSystem, feed_units: FeedRateUnits) {
        let Some(profile) = self.store.as_ref().and_then(|s| s.get_profile(id)) else {
            return;
        };
        self.confirm_delete = None;
        self.notice = None;
        self.edit = Some(Edit {
            id: profile.id.clone(),
            form: ProfileForm::from_profile(&profile, units, feed_units),
            units,
            feed_units,
            tab: Tab::General,
        });
    }

    /// Stores a new default profile and opens it.
    fn add(&mut self, units: MeasurementSystem, feed_units: FeedRateUnits, tr: &Translator) {
        let Some(store) = &self.store else {
            return;
        };
        let profile = DeviceProfile::default();
        let id = profile.id.clone();
        match store.save_profile(profile) {
            Ok(()) => self.select(&id, units, feed_units),
            Err(error) => self.failed(&error, tr),
        }
    }

    fn failed(&mut self, error: &anyhow::Error, tr: &Translator) {
        tracing::warn!(%error, "device profiles not saved");
        let title = tr.tr(n_!("Failed to save device"));
        self.say(format!("{title}: {error}"), true);
    }

    /// The refusal for changing the active profile while a job runs.
    fn refuse_busy(&mut self, tr: &Translator) {
        let title = tr.tr(n_!("Device not changed"));
        let reason = tr.tr(n_!("a job is running"));
        self.say(format!("{title}: {reason}"), true);
    }

    /// Saves the form.
    fn save(&mut self, job_running: bool, tr: &Translator) {
        let Some(edit) = &self.edit else {
            return;
        };
        let Some(store) = &self.store else {
            return;
        };
        let is_active = self.active_id().as_deref() == Some(edit.id.as_str());
        if is_active && job_running {
            self.refuse_busy(tr);
            return;
        }
        let base = store.get_profile(&edit.id).unwrap_or_default();
        let profile = match edit.form.apply(&base, edit.units, edit.feed_units) {
            Ok(profile) => profile,
            Err(error) => {
                let detail = error.fill(&tr.tr(error.detail));
                self.say(format!("{}: {detail}", tr.tr(error.title)), true);
                return;
            }
        };
        let name = profile.name.clone();
        match store.save_profile(profile) {
            Ok(()) => {
                self.edit = None;
                self.changed |= is_active;
                self.say(format!("{} {name}", tr.tr(n_!("Device saved:"))), false);
            }
            Err(error) => self.failed(&error, tr),
        }
    }

    /// Asks before deleting the open profile, unless it must be kept.
    fn ask_delete(&mut self, tr: &Translator) {
        let Some(edit) = &self.edit else {
            return;
        };
        let count = self.profiles().len();
        let active = self.active_id();
        if let Some(refusal) = delete_refusal(&edit.id, active.as_deref(), count) {
            self.say(tr.tr(refusal.msgid()).into_owned(), true);
            return;
        }
        self.confirm_delete = Some(ConfirmDelete {
            id: edit.id.clone(),
            name: edit.form.name.clone(),
        });
    }

    /// Deletes the profile the user agreed to delete.
    fn delete(&mut self, tr: &Translator) {
        let Some(confirm) = self.confirm_delete.take() else {
            return;
        };
        let Some(store) = &self.store else {
            return;
        };
        // Checked again: the active profile may have changed since the question.
        let count = store.get_all_profiles().len();
        let active = store.get_active_profile().map(|p| p.id);
        if let Some(refusal) = delete_refusal(&confirm.id, active.as_deref(), count) {
            self.say(tr.tr(refusal.msgid()).into_owned(), true);
            return;
        }
        match store.delete_profile(&confirm.id) {
            Ok(()) => {
                self.edit = None;
                let title = tr.tr(n_!("Device deleted:"));
                self.say(format!("{title} {}", confirm.name), false);
            }
            Err(error) => self.failed(&error, tr),
        }
    }

    /// Makes the open profile the active one.
    fn set_active(&mut self, job_running: bool, tr: &Translator) {
        if job_running {
            self.refuse_busy(tr);
            return;
        }
        let (Some(edit), Some(store)) = (&self.edit, &self.store) else {
            return;
        };
        match store.set_active_profile(&edit.id) {
            Ok(()) => {
                self.changed = true;
                let title = tr.tr(n_!("Active device:"));
                self.say(format!("{title} {}", edit.form.name), false);
                self.edit = None;
            }
            Err(error) => self.failed(&error, tr),
        }
    }

    /// Fills the open form from the settings the controller reported (Sync from Device).
    ///
    /// # Arguments
    /// * `settings` - The GRBL settings reported on this connection.
    /// * `connected` - Whether the machine is connected.
    /// * `tr` - Translator for the notice.
    pub fn sync_from(&mut self, settings: &HashMap<u16, String>, connected: bool, tr: &Translator) {
        if !connected {
            let text = n_!("Please connect to a device first.");
            self.say(tr.tr(text).into_owned(), true);
            return;
        }
        if settings.is_empty() {
            let text = n_!("No settings reported yet: Retrieve them in Firmware Settings first.");
            self.say(tr.tr(text).into_owned(), true);
            return;
        }
        let Some(edit) = self.edit.as_mut() else {
            return;
        };
        edit.form
            .sync(&ReportedLimits::from_settings(settings), edit.units);
        let text = n_!(
            "Device dimensions, spindle speed, and capabilities have been updated from the connected device. Click Save to apply changes."
        );
        self.say(tr.tr(text).into_owned(), false);
    }

    /// Whether `profile` is listed for the search text.
    fn listed(&self, profile: &DeviceProfile) -> bool {
        matches_search(profile, &self.search)
    }
}

/// Reads the profiles file (creating it with the default profile when missing).
///
/// # Returns
/// Why it could not be read, if it could not.
fn load(store: &DeviceManager) -> Option<String> {
    let error = store.load().err()?;
    tracing::warn!(%error, "device profiles not loaded");
    Some(error.to_string())
}

/// The English msgids this window shows, for the translation test.
pub fn msgids() -> Vec<&'static str> {
    let mut ids = vec![
        n_!("Device Manager"),
        n_!("Devices"),
        n_!("Search devices..."),
        n_!("Add Device"),
        n_!("Active"),
        n_!("Save"),
        n_!("Cancel"),
        n_!("Delete"),
        n_!("Sync from Device"),
        n_!("Update device information from connected device"),
        n_!("Set Active"),
        n_!("Please select or create a device entry to edit"),
        n_!("Delete device?"),
        n_!("Delete '{}'? This cannot be undone."),
        n_!("Yes"),
        n_!("No"),
        n_!("Name:"),
        n_!("Description:"),
        n_!("Device Type:"),
        n_!("Controller:"),
        n_!("Connection Type:"),
        n_!("Port:"),
        n_!("Baud Rate:"),
        n_!("TCP Host:"),
        n_!("TCP Port:"),
        n_!("Timeout (ms):"),
        n_!("Auto Reconnect"),
        n_!("X Axis:"),
        n_!("Y Axis:"),
        n_!("Z Axis:"),
        n_!("Min:"),
        n_!("Max:"),
        n_!("Limits"),
        n_!("Max Feed Rate:"),
        n_!("Max S-Value:"),
        n_!("No of Axes:"),
        n_!("Has Spindle"),
        n_!("Spindle Power (W):"),
        n_!("Max Spindle Speed (RPM):"),
        n_!("Has Laser"),
        n_!("Laser Power (W):"),
        n_!("Has Coolant"),
        n_!("Pen Mode (Repetier: M3/M5 become pen down/up Z moves)"),
        n_!("Device profiles could not be read:"),
        n_!("No settings folder: device profiles cannot be loaded or saved."),
        n_!("Failed to save device"),
        n_!("Device not changed"),
        n_!("a job is running"),
        n_!("Device saved:"),
        n_!("Device deleted:"),
        n_!("Active device:"),
        n_!("Please connect to a device first."),
        n_!("No settings reported yet: Retrieve them in Firmware Settings first."),
        n_!(
            "Device dimensions, spindle speed, and capabilities have been updated from the connected device. Click Save to apply changes."
        ),
    ];
    ids.extend(Tab::ALL.map(Tab::msgid));
    ids.extend(device_profiles::msgids());
    ids
}
