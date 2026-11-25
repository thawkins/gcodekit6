//! The Materials Manager window (GTK's Materials Manager): the material library searched
//! and filtered by category, listed by category then name, and an edit form in five tabs
//! (Basic Info, Properties, Machining, Safety, Notes).
//!
//! - Custom materials are kept in the GTK build's `custom_materials.json` (legacy settings
//!   folder) through `gcodekit5-app`'s `MaterialsManagerBackend`, so a library written by
//!   either build loads in the other. Saves are atomic; an unreadable file is reported when
//!   the window opens and backed up (`custom_materials.json.bak-<time>`, named in the
//!   notice) before the first save replaces it.
//! - The form's rules and messages are GTK's ([`MaterialForm`]). Saving a standard material
//!   stores a custom copy (GTK's override); only custom materials can be deleted, and
//!   deleting an override brings the standard material back.
//! - Switching away from an edited material asks Save / Discard / Cancel (GTK dropped the
//!   edits silently).
//! - Every change is reported through [`MaterialsManagerWindow::take_changed`], so the app
//!   hands the library to the speeds and feeds calculator at once.
//!
//! Dependencies: `egui`, `gcodekit5-app` (form rules, back end, translations),
//! `gcodekit5-core` (material types).

use std::path::{Path, PathBuf};

use gcodekit5_app::i18n::Translator;
use gcodekit5_app::material_form::{self, MaterialFilter, MaterialForm};
use gcodekit5_app::materials_manager_backend::MaterialsManagerBackend;
use gcodekit5_core::data::materials::{Material, MaterialId};

mod view;

/// The form's tabs.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Tab {
    Basic,
    Properties,
    Machining,
    Safety,
    Notes,
}

impl Tab {
    const ALL: [Self; 5] = [
        Self::Basic,
        Self::Properties,
        Self::Machining,
        Self::Safety,
        Self::Notes,
    ];

    fn msgid(self) -> &'static str {
        match self {
            Self::Basic => n_!("Basic Info"),
            Self::Properties => n_!("Properties"),
            Self::Machining => n_!("Machining"),
            Self::Safety => n_!("Safety"),
            Self::Notes => n_!("Notes"),
        }
    }
}

/// The material being edited.
#[derive(Debug, Clone)]
struct Edit {
    /// The material as stored, or `None` for a new material.
    original: Option<Material>,
    form: MaterialForm,
    tab: Tab,
}

/// Where the user wants to go from the open form.
#[derive(Debug, Clone, PartialEq, Eq)]
enum Next {
    /// A new material's form.
    New,
    /// The stored material with this id.
    Select(String),
    /// No form (Cancel).
    Close,
}

/// A question waiting for the user's answer.
#[derive(Debug, Clone, PartialEq, Eq)]
enum Question {
    /// Delete the material (id, name)?
    Delete(String, String),
    /// Save the edited material before going on?
    Unsaved(Next),
}

/// The Materials Manager window.
pub struct MaterialsManagerWindow {
    open: bool,
    /// The library and its file; `None` without a settings folder.
    backend: Option<MaterialsManagerBackend>,
    /// Why the file could not be read, until the window shows it.
    load_error: Option<String>,
    /// The backup of an unreadable file the notice last named.
    backup_named: Option<PathBuf>,
    filter: MaterialFilter,
    edit: Option<Edit>,
    question: Option<Question>,
    /// The latest notice and whether it reports a failure.
    notice: Option<(String, bool)>,
    /// The library changed since [`Self::take_changed`].
    changed: bool,
}

impl std::fmt::Debug for MaterialsManagerWindow {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("MaterialsManagerWindow")
            .field("open", &self.open)
            .field("filter", &self.filter)
            .field("notice", &self.notice)
            .finish_non_exhaustive()
    }
}

impl MaterialsManagerWindow {
    /// The window over the custom materials in `file`, read now.
    ///
    /// # Arguments
    /// * `file` - The custom materials file; `None` without a settings folder.
    pub fn new(file: Option<PathBuf>) -> Self {
        let backend = file.map(MaterialsManagerBackend::with_storage_path);
        let load_error = backend
            .as_ref()
            .and_then(|b| b.load_error().map(str::to_string));
        Self {
            open: false,
            backend,
            load_error,
            backup_named: None,
            filter: MaterialFilter::default(),
            edit: None,
            question: None,
            notice: None,
            changed: false,
        }
    }

    /// Opens the window, telling why the library could not be read if it could not.
    ///
    /// # Arguments
    /// * `tr` - Translator for a notice.
    pub fn open(&mut self, tr: &Translator) {
        self.open = true;
        if self.backend.is_none() {
            let text = n_!("No settings folder: custom materials cannot be loaded or saved.");
            self.say(tr.tr(text).into_owned(), true);
        }
        if let Some(error) = self.load_error.take() {
            let title = tr.tr(n_!("The custom materials file could not be read:"));
            self.say(format!("{title} {error}"), true);
        }
    }

    /// Whether the window is open.
    pub fn is_open(&self) -> bool {
        self.open
    }

    /// The latest notice (a save's result, a refusal), if any.
    pub fn notice(&self) -> Option<&str> {
        self.notice.as_ref().map(|(text, _)| text.as_str())
    }

    /// The list's search text and category filter.
    pub fn filter_mut(&mut self) -> &mut MaterialFilter {
        &mut self.filter
    }

    /// Every material, standard and custom (the standard materials alone without a
    /// settings folder).
    pub fn materials(&self) -> Vec<Material> {
        match &self.backend {
            Some(backend) => backend.get_all_materials().into_iter().cloned().collect(),
            None => gcodekit5_core::data::materials::init_standard_library()
                .get_all_materials()
                .into_iter()
                .cloned()
                .collect(),
        }
    }

    /// Whether the library changed since the last call.
    pub fn take_changed(&mut self) -> bool {
        std::mem::take(&mut self.changed)
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
                "A copy of the unreadable custom materials file was kept in"
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
        self.edit.as_ref().is_some_and(|e| match &e.original {
            Some(original) => e.form != MaterialForm::from_material(original),
            // A new material's form counts as changed once anything was typed.
            None => e.form != MaterialForm::default(),
        })
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
                    form: MaterialForm::default(),
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

    /// Opens the stored material `id` in the form.
    fn load(&mut self, id: &str) {
        let Some(material) = self
            .backend
            .as_ref()
            .and_then(|b| b.get_material(&MaterialId(id.to_string())))
            .cloned()
        else {
            return;
        };
        let tab = self.edit.as_ref().map_or(Tab::Basic, |e| e.tab);
        self.edit = Some(Edit {
            form: MaterialForm::from_material(&material),
            original: Some(material),
            tab,
        });
    }

    /// Saves the form (GTK's `save_material`).
    ///
    /// # Returns
    /// Whether the material was saved.
    fn save(&mut self, tr: &Translator) -> bool {
        let Some(edit) = &self.edit else {
            return false;
        };
        let built = edit.form.build(edit.original.as_ref());
        let creating = edit.original.is_none();
        let Some(backend) = &mut self.backend else {
            return false;
        };
        let material = match built {
            Ok(material) => material,
            Err(msgid) => {
                self.say(tr.tr(msgid).into_owned(), true);
                return false;
            }
        };
        if creating && backend.get_material(&material.id).is_some() {
            let text = n_!("A material with this ID already exists.");
            self.say(tr.tr(text).into_owned(), true);
            return false;
        }
        let (id, name) = (material.id.0.clone(), material.name.clone());
        if let Err(error) = backend.save_material(material) {
            tracing::warn!(%error, "material not saved");
            let title = tr.tr(n_!("Could not save the material:"));
            self.say(format!("{title} {error}"), true);
            self.name_backup(tr);
            return false;
        }
        self.changed = true;
        self.load(&id);
        self.say(format!("{} {name}", tr.tr(n_!("Material saved:"))), false);
        self.name_backup(tr);
        true
    }

    /// Asks before deleting the open material; only custom materials can be deleted.
    fn ask_delete(&mut self) {
        let Some(original) = self.edit.as_ref().and_then(|e| e.original.as_ref()) else {
            return;
        };
        if original.custom {
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
        match backend.remove_material(&MaterialId(id.to_string())) {
            Ok(_) => {
                self.changed = true;
                self.edit = None;
                let title = tr.tr(n_!("Material deleted:"));
                self.say(format!("{title} {name}"), false);
                self.name_backup(tr);
            }
            Err(error) => {
                tracing::warn!(%error, "material not deleted");
                let title = tr.tr(n_!("Could not delete the material:"));
                self.say(format!("{title} {error}"), true);
            }
        }
    }
}

/// The English msgids this window shows, for the translation test.
pub fn msgids() -> Vec<&'static str> {
    let mut ids = vec![
        n_!("Materials Manager"),
        n_!("Search materials..."),
        n_!("All Categories"),
        n_!("No materials found"),
        n_!("New Material"),
        n_!("Please select or create a material to edit"),
        n_!("Machinability:"),
        n_!("Save"),
        n_!("Cancel"),
        n_!("Delete"),
        n_!("Only custom materials can be deleted."),
        n_!("Delete material?"),
        n_!("Delete '{}'? This cannot be undone."),
        n_!("Unsaved changes"),
        n_!("The material has unsaved changes. Save them before continuing?"),
        n_!("Discard"),
        n_!("Yes"),
        n_!("No"),
        n_!("ID:"),
        n_!("Name:"),
        n_!("Category:"),
        n_!("Subcategory:"),
        n_!("e.g., Hardwood, Alloy"),
        n_!("Description:"),
        n_!("Density (kg/m3):"),
        n_!("Machinability (1-10):"),
        n_!("Tensile Strength (MPa):"),
        n_!("Melting Point (deg C):"),
        n_!("Optional"),
        n_!("Chip Type:"),
        n_!("Heat Sensitivity:"),
        n_!("Abrasiveness:"),
        n_!("Surface Finish:"),
        n_!("Dust Hazard:"),
        n_!("Fume Hazard:"),
        n_!("Coolant Required"),
        n_!("Additional Notes and Tips:"),
        n_!("No settings folder: custom materials cannot be loaded or saved."),
        n_!("The custom materials file could not be read:"),
        n_!("A copy of the unreadable custom materials file was kept in"),
        n_!("A material with this ID already exists."),
        n_!("Could not save the material:"),
        n_!("Material saved:"),
        n_!("Material deleted:"),
        n_!("Could not delete the material:"),
    ];
    ids.extend(Tab::ALL.map(Tab::msgid));
    ids.extend(material_form::msgids());
    ids
}
