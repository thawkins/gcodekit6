//! The CNC Tools page (GTK's Tools Manager): the tool library searched and filtered by
//! type, material and diameter, an edit form in four tabs (Basic Info, Geometry,
//! Manufacturer, Notes), and the library actions (New Tool, GTC import, export, reset).
//!
//! - Custom tools are kept in the GTK build's `custom_tools.json` (legacy settings folder)
//!   through `gcodekit5-app`'s `ToolsManagerBackend`, so a library written by either build
//!   loads in the other. Saves are atomic; an unreadable file is reported when the window
//!   opens and backed up (`custom_tools.json.bak-<time>`, named in the notice) before the
//!   first save replaces it.
//! - The form's rules and messages are GTK's ([`ToolForm`]); lengths are millimetres, as in
//!   GTK. Switching away from an edited tool asks Save / Discard / Cancel, as GTK does.
//! - Only custom tools can be deleted (a standard tool would come back on the next start);
//!   deleting a custom tool that replaced a standard one brings the standard one back.
//! - Every change is reported through [`ToolsManagerWindow::take_changed`], so the app hands
//!   the library to the speeds and feeds calculator at once.
//!
//! Dependencies: `egui`, `gcodekit5-app` (form rules, back end, translations),
//! `gcodekit5-core` (tool types), `gcodekit5-settings` (the remembered filters).

use std::path::{Path, PathBuf};

use gcodekit5_app::i18n::Translator;
use gcodekit5_app::tool_form::{
    self, ToolFilter, ToolForm, tool_material_filter_id, tool_material_from_filter_id,
    tool_type_filter_id, tool_type_from_filter_id,
};
use gcodekit5_app::tools_manager_backend::ToolsManagerBackend;
use gcodekit5_core::data::tools::{Tool, ToolId};
use gcodekit5_settings::config::Config;

mod view;

/// The form's tabs.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Tab {
    Basic,
    Geometry,
    Manufacturer,
    Notes,
}

impl Tab {
    const ALL: [Self; 4] = [Self::Basic, Self::Geometry, Self::Manufacturer, Self::Notes];

    fn msgid(self) -> &'static str {
        match self {
            Self::Basic => n_!("Basic Info"),
            Self::Geometry => n_!("Geometry"),
            Self::Manufacturer => n_!("Manufacturer"),
            Self::Notes => n_!("Notes"),
        }
    }
}

/// The tool being edited.
#[derive(Debug, Clone)]
struct Edit {
    /// The tool as stored, or `None` for a new tool.
    original: Option<Tool>,
    form: ToolForm,
    tab: Tab,
}

/// Where the user wants to go from the open form.
#[derive(Debug, Clone, PartialEq, Eq)]
enum Next {
    /// A new tool's form.
    New,
    /// The stored tool with this id.
    Select(String),
    /// No form (Cancel).
    Close,
}

/// A question waiting for the user's answer.
#[derive(Debug, Clone, PartialEq, Eq)]
enum Question {
    /// Delete the tool (id, name)?
    Delete(String, String),
    /// Remove every custom tool?
    Reset,
    /// Save the edited tool before going on?
    Unsaved(Next),
}

/// The kind of GTC catalog file to import.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GtcFile {
    /// A `.zip` package.
    Package,
    /// A `.json` catalog.
    Json,
}

/// A file dialog the window asks the app to show.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ToolsFileRequest {
    /// Choose a GTC catalog to import.
    Import(GtcFile),
    /// Choose where to export the custom tools.
    Export,
}

/// The CNC Tools settings GTK remembered (`config.ui.tools_manager_*`).
#[derive(Debug, Clone, PartialEq)]
struct UiState {
    search: String,
    type_filter: String,
    material_filter: String,
    dia_min: Option<f32>,
    dia_max: Option<f32>,
    selected: Option<String>,
}

impl UiState {
    /// The values in `config`; an empty filter is GTK's "all", so untouched settings are
    /// not rewritten at startup.
    fn of(config: &Config) -> Self {
        let ui = &config.ui;
        let or_all = |id: &str| if id.is_empty() { "all" } else { id }.to_string();
        Self {
            search: ui.tools_manager_search.clone(),
            type_filter: or_all(&ui.tools_manager_type_filter),
            material_filter: or_all(&ui.tools_manager_material_filter),
            dia_min: ui.tools_manager_dia_min,
            dia_max: ui.tools_manager_dia_max,
            selected: ui.tools_manager_selected_tool.clone(),
        }
    }
}

/// The CNC Tools page.
pub struct ToolsManagerWindow {
    /// The library and its file; `None` without a settings folder.
    backend: Option<ToolsManagerBackend>,
    /// Why the file could not be read, until the window shows it.
    load_error: Option<String>,
    /// The backup of an unreadable file the notice last named.
    backup_named: Option<PathBuf>,
    filter: ToolFilter,
    /// The tool last opened (remembered in the settings, as GTK does).
    last_selected: Option<String>,
    edit: Option<Edit>,
    question: Option<Question>,
    /// The latest notice and whether it reports a failure.
    notice: Option<(String, bool)>,
    /// The library changed since [`Self::take_changed`].
    changed: bool,
    file_request: Option<ToolsFileRequest>,
    /// The settings' values as last read or stored, to store only changes.
    stored_ui: UiState,
}

impl std::fmt::Debug for ToolsManagerWindow {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ToolsManagerWindow")
            .field("filter", &self.filter)
            .field("notice", &self.notice)
            .finish_non_exhaustive()
    }
}

impl ToolsManagerWindow {
    /// The window over the custom tools in `file`, read now, with the search and filters
    /// GTK remembered in `config`.
    ///
    /// # Arguments
    /// * `file` - The custom tools file; `None` without a settings folder.
    /// * `config` - The settings holding the remembered filters.
    pub fn new(file: Option<PathBuf>, config: &Config) -> Self {
        let backend = file.map(ToolsManagerBackend::with_storage_path);
        let load_error = backend
            .as_ref()
            .and_then(|b| b.load_error().map(str::to_string));
        let ui = &config.ui;
        let filter = ToolFilter {
            search: ui.tools_manager_search.clone(),
            tool_type: tool_type_from_filter_id(&ui.tools_manager_type_filter),
            material: tool_material_from_filter_id(&ui.tools_manager_material_filter),
            dia_min: ui
                .tools_manager_dia_min
                .map(|v| format!("{v:.3}"))
                .unwrap_or_default(),
            dia_max: ui
                .tools_manager_dia_max
                .map(|v| format!("{v:.3}"))
                .unwrap_or_default(),
        };
        Self {
            backend,
            load_error,
            backup_named: None,
            filter,
            last_selected: ui.tools_manager_selected_tool.clone(),
            edit: None,
            question: None,
            notice: None,
            changed: false,
            file_request: None,
            stored_ui: UiState::of(config),
        }
    }

    /// Prepares the page each time it is shown, telling why the library could not be read
    /// if it could not.
    ///
    /// # Arguments
    /// * `tr` - Translator for a notice.
    pub fn open(&mut self, tr: &Translator) {
        if self.backend.is_none() {
            let text = n_!("No settings folder: custom tools cannot be loaded or saved.");
            self.say(tr.tr(text).into_owned(), true);
        }
        if let Some(error) = self.load_error.take() {
            let title = tr.tr(n_!("The custom tools file could not be read:"));
            self.say(format!("{title} {error}"), true);
        }
        if self.edit.is_none()
            && let Some(id) = self.last_selected.clone()
        {
            self.load(&id);
        }
    }

    /// Whether the form holds a tool with changes not saved yet.
    pub fn has_unsaved_changes(&self) -> bool {
        self.is_dirty()
    }

    /// Saves the form for the quit flow's Save (the app shows the page when it is refused).
    ///
    /// # Arguments
    /// * `tr` - Translator for the notice.
    ///
    /// # Returns
    /// Whether the tool was saved.
    pub fn save_for_quit(&mut self, tr: &Translator) -> bool {
        self.save(tr)
    }

    /// The latest notice (a save's result, a refusal), if any.
    pub fn notice(&self) -> Option<&str> {
        self.notice.as_ref().map(|(text, _)| text.as_str())
    }

    /// The list's search text and filters.
    pub fn filter_mut(&mut self) -> &mut ToolFilter {
        &mut self.filter
    }

    /// Every tool, standard and custom (the standard tools alone without a settings
    /// folder).
    pub fn tools(&self) -> Vec<Tool> {
        match &self.backend {
            Some(backend) => backend.get_all_tools().into_iter().cloned().collect(),
            None => gcodekit5_core::data::tools::init_standard_library()
                .get_all_tools()
                .into_iter()
                .cloned()
                .collect(),
        }
    }

    /// Whether the library changed since the last call.
    pub fn take_changed(&mut self) -> bool {
        std::mem::take(&mut self.changed)
    }

    /// The file dialog the window asked for since the last call, if any.
    pub fn take_file_request(&mut self) -> Option<ToolsFileRequest> {
        self.file_request.take()
    }

    /// The search, filters and last opened tool as the settings hold them.
    fn ui_state(&self) -> UiState {
        UiState {
            search: self.filter.search.clone(),
            type_filter: self
                .filter
                .tool_type
                .map(tool_type_filter_id)
                .unwrap_or("all")
                .to_string(),
            material_filter: self
                .filter
                .material
                .map(tool_material_filter_id)
                .unwrap_or("all")
                .to_string(),
            dia_min: self.filter.dia_min.trim().parse().ok(),
            dia_max: self.filter.dia_max.trim().parse().ok(),
            selected: self.last_selected.clone(),
        }
    }

    /// Whether the search, filters or last opened tool changed since they were last
    /// stored ([`Self::store_ui_state`]) or read from the settings.
    pub fn ui_state_changed(&self) -> bool {
        self.stored_ui != self.ui_state()
    }

    /// Writes the search, filters and last opened tool into `config` (GTK keeps them in
    /// the settings and stored them on every change).
    ///
    /// # Arguments
    /// * `config` - The settings to write into.
    pub fn store_ui_state(&mut self, config: &mut Config) {
        let state = self.ui_state();
        let ui = &mut config.ui;
        ui.tools_manager_search = state.search.clone();
        ui.tools_manager_type_filter = state.type_filter.clone();
        ui.tools_manager_material_filter = state.material_filter.clone();
        ui.tools_manager_dia_min = state.dia_min;
        ui.tools_manager_dia_max = state.dia_max;
        ui.tools_manager_selected_tool = state.selected.clone();
        self.stored_ui = state;
    }

    fn say(&mut self, text: String, error: bool) {
        self.notice = Some((text, error));
    }

    /// Adds the path of a backup the library file got since the last notice to the notice.
    fn name_backup(&mut self, tr: &Translator) {
        let Some(backup) = self
            .backend
            .as_ref()
            .and_then(|b| b.last_backup().map(Path::to_path_buf))
        else {
            return;
        };
        if self.backup_named.as_ref() == Some(&backup) {
            return;
        }
        let note = format!(
            "{} {}",
            tr.tr(n_!(
                "A copy of the unreadable custom tools file was kept in"
            )),
            backup.display()
        );
        self.notice = Some(match self.notice.take() {
            Some((text, _)) => (format!("{text} {note}"), true),
            None => (note, true),
        });
        self.backup_named = Some(backup);
    }

    /// Whether the open form holds changes not saved yet.
    fn is_dirty(&self) -> bool {
        self.edit
            .as_ref()
            .is_some_and(|e| e.form.is_dirty(e.original.as_ref()))
    }

    /// Goes to `next`, asking first when the open form has unsaved changes.
    fn request(&mut self, next: Next) {
        if self.is_dirty() {
            self.question = Some(Question::Unsaved(next));
        } else {
            self.go(next);
        }
    }

    fn go(&mut self, next: Next) {
        self.question = None;
        match next {
            Next::New => {
                self.notice = None;
                self.edit = Some(Edit {
                    original: None,
                    form: ToolForm::default(),
                    tab: Tab::Basic,
                });
            }
            Next::Select(id) => {
                self.notice = None;
                self.load(&id);
            }
            Next::Close => self.edit = None,
        }
    }

    /// Opens the stored tool `id` in the form.
    fn load(&mut self, id: &str) {
        let Some(tool) = self
            .backend
            .as_ref()
            .and_then(|b| b.get_tool(&ToolId(id.to_string())))
            .cloned()
        else {
            return;
        };
        let tab = self.edit.as_ref().map_or(Tab::Basic, |e| e.tab);
        self.last_selected = Some(tool.id.0.clone());
        self.edit = Some(Edit {
            form: ToolForm::from_tool(&tool),
            original: Some(tool),
            tab,
        });
    }

    /// Saves the form (GTK's `save_tool`).
    ///
    /// # Returns
    /// Whether the tool was saved.
    fn save(&mut self, tr: &Translator) -> bool {
        let Some(edit) = &self.edit else {
            return false;
        };
        let built = edit.form.build(edit.original.as_ref());
        let creating = edit.original.is_none();
        let Some(backend) = &mut self.backend else {
            return false;
        };
        let tool = match built {
            Ok(tool) => tool,
            Err(msgid) => {
                self.say(tr.tr(msgid).into_owned(), true);
                return false;
            }
        };
        if creating && backend.get_tool(&tool.id).is_some() {
            let text = n_!("Tool ID already exists. Please choose a unique ID.");
            self.say(tr.tr(text).into_owned(), true);
            return false;
        }
        let (id, name) = (tool.id.0.clone(), tool.name.clone());
        if let Err(error) = backend.save_tool(tool) {
            tracing::warn!(%error, "tool not saved");
            let title = tr.tr(n_!("Could not save the tool:"));
            self.say(format!("{title} {error}"), true);
            self.name_backup(tr);
            return false;
        }
        self.changed = true;
        self.load(&id);
        self.say(format!("{} {name}", tr.tr(n_!("Tool saved:"))), false);
        self.name_backup(tr);
        true
    }

    /// Asks before deleting the open tool; only custom tools can be deleted.
    fn ask_delete(&mut self) {
        let Some(original) = self.edit.as_ref().and_then(|e| e.original.as_ref()) else {
            return;
        };
        if tool_form::can_delete(original) {
            self.question = Some(Question::Delete(
                original.id.0.clone(),
                original.name.clone(),
            ));
        }
    }

    fn delete(&mut self, id: &str, name: &str, tr: &Translator) {
        let Some(backend) = &mut self.backend else {
            return;
        };
        match backend.remove_tool(&ToolId(id.to_string())) {
            Ok(_) => {
                self.changed = true;
                self.edit = None;
                if self.last_selected.as_deref() == Some(id) {
                    self.last_selected = None;
                }
                self.say(format!("{} {name}", tr.tr(n_!("Tool deleted:"))), false);
                self.name_backup(tr);
            }
            Err(error) => {
                tracing::warn!(%error, "tool not deleted");
                let title = tr.tr(n_!("Could not delete the tool:"));
                self.say(format!("{title} {error}"), true);
            }
        }
    }

    fn reset(&mut self, tr: &Translator) {
        let Some(backend) = &mut self.backend else {
            return;
        };
        match backend.reset_custom_tools() {
            Ok(()) => {
                self.changed = true;
                self.edit = None;
                self.say(tr.tr(n_!("Custom tools removed.")).into_owned(), false);
                self.name_backup(tr);
            }
            Err(error) => {
                tracing::warn!(%error, "custom tools not reset");
                let title = tr.tr(n_!("Reset failed:"));
                self.say(format!("{title} {error}"), true);
            }
        }
    }

    /// Imports the tools of the GTC catalog `path` (GTK's Import GTC) and saves them.
    ///
    /// # Arguments
    /// * `path` - The catalog file.
    /// * `kind` - Whether it is a `.zip` package or a `.json` catalog.
    /// * `tr` - Translator for the notice.
    pub fn import_gtc(&mut self, path: &Path, kind: GtcFile, tr: &Translator) {
        let Some(backend) = &mut self.backend else {
            return;
        };
        let result = match kind {
            GtcFile::Package => backend.import_gtc_package(path),
            GtcFile::Json => backend.import_gtc_json(path),
        };
        match result {
            Ok(result) => {
                self.changed = true;
                let text = format!(
                    "{} {} {} {} {} {} {}",
                    tr.tr(n_!("Import complete.")),
                    tr.tr(n_!("Imported:")),
                    result.imported_tools.len(),
                    tr.tr(n_!("Skipped:")),
                    result.skipped_tools,
                    tr.tr(n_!("Errors:")),
                    result.errors.len()
                );
                self.say(text, false);
                self.name_backup(tr);
            }
            Err(error) => {
                tracing::warn!(%error, "GTC import failed");
                let title = tr.tr(n_!("Import failed:"));
                self.say(format!("{title} {error}"), true);
            }
        }
    }

    /// Writes the custom tools to `path` (GTK's Export Custom Tools).
    ///
    /// # Arguments
    /// * `path` - The file to write.
    /// * `tr` - Translator for the notice.
    pub fn export_to(&mut self, path: &Path, tr: &Translator) {
        let Some(backend) = &self.backend else {
            return;
        };
        match backend.export_custom_tools(path) {
            Ok(()) => self.say(tr.tr(n_!("Custom tools exported.")).into_owned(), false),
            Err(error) => {
                tracing::warn!(%error, "custom tools not exported");
                let title = tr.tr(n_!("Export failed:"));
                self.say(format!("{title} {error}"), true);
            }
        }
    }
}

/// The English msgids this window shows, for the translation test.
pub fn msgids() -> Vec<&'static str> {
    let mut ids = vec![
        n_!("CNC Tools"),
        n_!("Search (name, id, type, diameter)..."),
        n_!("All Types"),
        n_!("All Materials"),
        n_!("Filters"),
        n_!("Min diameter (mm)"),
        n_!("Max diameter (mm)"),
        n_!("New Tool"),
        n_!("Create a new custom tool"),
        n_!("Library"),
        n_!("Import GTC (.zip)..."),
        n_!("Import tool catalog from a GTC zip package"),
        n_!("Import GTC (.json)..."),
        n_!("Import tool catalog from a GTC JSON file"),
        n_!("Export Custom Tools..."),
        n_!("Export custom tools to a JSON file"),
        n_!("Reset Custom Tools..."),
        n_!("Remove all custom/imported tools and delete local storage"),
        n_!("Please select or create a tool to edit"),
        n_!("Save"),
        n_!("Save changes"),
        n_!("Cancel"),
        n_!("Discard changes"),
        n_!("Delete"),
        n_!("Delete this tool"),
        n_!("Only custom tools can be deleted."),
        n_!("Delete tool?"),
        n_!("Delete '{}'? This cannot be undone."),
        n_!("Reset custom tools?"),
        n_!("This will delete all custom/imported tools and remove local custom tools storage."),
        n_!("Unsaved changes"),
        n_!("The tool has unsaved changes. Save them before continuing?"),
        n_!("Discard"),
        n_!("Yes"),
        n_!("No"),
        n_!("ID:"),
        n_!("Tool Number:"),
        n_!("Name:"),
        n_!("Tool Type:"),
        n_!("Tool Material:"),
        n_!("Coating:"),
        n_!("Shank:"),
        n_!("Diameter (mm):"),
        n_!("Length (mm):"),
        n_!("Flute Length (mm):"),
        n_!("Shaft Diameter (mm):"),
        n_!("Number of Flutes:"),
        n_!("Corner Radius (mm):"),
        n_!("Only for corner radius end mills"),
        n_!("Tip Angle (deg):"),
        n_!("Used for drills / V-bits"),
        n_!("Manufacturer:"),
        n_!("Part Number:"),
        n_!("Description:"),
        n_!("Additional Notes:"),
        n_!("No settings folder: custom tools cannot be loaded or saved."),
        n_!("The custom tools file could not be read:"),
        n_!("A copy of the unreadable custom tools file was kept in"),
        n_!("Tool ID already exists. Please choose a unique ID."),
        n_!("Could not save the tool:"),
        n_!("Tool saved:"),
        n_!("Tool deleted:"),
        n_!("Could not delete the tool:"),
        n_!("Custom tools removed."),
        n_!("Reset failed:"),
        n_!("Import complete."),
        n_!("Imported:"),
        n_!("Skipped:"),
        n_!("Errors:"),
        n_!("Import failed:"),
        n_!("Custom tools exported."),
        n_!("Export failed:"),
    ];
    ids.extend(Tab::ALL.map(Tab::msgid));
    ids.extend(tool_form::msgids());
    ids
}
