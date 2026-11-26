//! Native file and message dialogs that never block the event loop.
//!
//! [`Dialogs`] wraps `craft_dialogs::Dialogs`: a request is queued ([`Dialogs::open_file`],
//! [`Dialogs::save_file`], [`Dialogs::confirm`]), shown at the end of the frame by
//! [`Dialogs::end_frame`] on the main thread, and its answer comes back through
//! [`Dialogs::poll`] on a later frame, tagged with what it was asked for. No call here waits
//! for the user, so the window keeps drawing while a dialog is up.
//!
//! Ported from the GTK crate's `ui/gtk/file_dialog.rs` (open and save dialogs with file-type
//! filters) and `ui/gtk/common/dialog.rs` (the Save / Discard / Cancel prompt and Yes / No
//! confirmations, whose answer is `gcodekit5_app::UnsavedChangesChoice`).
//!
//! Dependencies: `craft-dialogs` (the queue and the native dialogs), `gcodekit5-app`
//! (translations).

use std::collections::VecDeque;
use std::path::{Path, PathBuf};

use craft_dialogs::{
    Answer, FileDialog, Message, MessageButtons, MessageLevel, MessageResult, Override, Wake,
};
use gcodekit5_app::UnsavedChangesChoice;
use gcodekit5_app::i18n::Translator;
use gcodekit5_app::work_zero::Wcs;
use raw_window_handle::{HasDisplayHandle, HasWindowHandle};

/// What a file dialog is for; its answer comes back with it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FilePurpose {
    /// Choose a G-code file to open in the editor.
    OpenGcode,
    /// Choose where to save the editor's G-code.
    SaveGcode,
    /// Choose a design file to open in the designer.
    OpenDesign,
    /// Choose where to save the design.
    SaveDesign,
    /// Choose an SVG or DXF file to import into the design.
    ImportDesign,
    /// Choose an image to import into the design as a raster shape.
    ImportImage,
    /// Choose where to export the design's G-code.
    ExportGcode,
    /// Choose where to export the design as SVG.
    ExportSvg,
    /// Choose a CAM page's preset file to load.
    LoadCamPreset,
    /// Choose where to save a CAM page's preset.
    SaveCamPreset,
    /// Choose the Gerber page's project folder.
    OpenGerberFolder,
    /// Choose the image the Laser Image Engraver engraves.
    OpenEngravingImage,
    /// Choose where to save the firmware settings list.
    ExportFirmwareSettings,
    /// Choose a saved firmware settings list to load.
    ImportFirmwareSettings,
    /// Choose a GTC tool catalog package (.zip) to import into the tool library.
    ImportGtcPackage,
    /// Choose a GTC tool catalog (.json) to import into the tool library.
    ImportGtcJson,
    /// Choose where to export the custom tools.
    ExportCustomTools,
}

impl FilePurpose {
    /// The English msgid of the dialog's title.
    pub fn title_msgid(self) -> &'static str {
        match self {
            Self::OpenGcode => n_!("Open G-Code File"),
            Self::SaveGcode => n_!("Save G-Code File"),
            Self::OpenDesign => n_!("Open Design File"),
            Self::SaveDesign => n_!("Save Design File"),
            Self::ImportDesign => n_!("Import Design File"),
            Self::ImportImage => n_!("Import Image"),
            Self::ExportGcode => n_!("Export G-Code"),
            Self::ExportSvg => n_!("Export SVG"),
            Self::LoadCamPreset => n_!("Load Parameters"),
            Self::SaveCamPreset => n_!("Save Parameters"),
            Self::OpenGerberFolder => n_!("Open Gerber Directory"),
            Self::OpenEngravingImage => n_!("Select Image"),
            Self::ExportFirmwareSettings => n_!("Export Settings"),
            Self::ImportFirmwareSettings => n_!("Import Settings"),
            Self::ImportGtcPackage => n_!("Import GTC Package"),
            Self::ImportGtcJson => n_!("Import GTC JSON"),
            Self::ExportCustomTools => n_!("Export Custom Tools"),
        }
    }

    /// Whether a file chosen for this purpose becomes the folder the next file dialog starts
    /// in: the editor's and the designer's files, as GTK remembered them; not presets,
    /// firmware settings, tool catalogs or the Gerber and engraving inputs.
    pub fn remembers_folder(self) -> bool {
        match self {
            Self::OpenGcode
            | Self::SaveGcode
            | Self::OpenDesign
            | Self::SaveDesign
            | Self::ImportDesign
            | Self::ImportImage
            | Self::ExportGcode
            | Self::ExportSvg => true,
            Self::LoadCamPreset
            | Self::SaveCamPreset
            | Self::OpenGerberFolder
            | Self::OpenEngravingImage
            | Self::ExportFirmwareSettings
            | Self::ImportFirmwareSettings
            | Self::ImportGtcPackage
            | Self::ImportGtcJson
            | Self::ExportCustomTools => false,
        }
    }

    /// The file types the dialog offers, the first one selected: an Open dialog offers its
    /// type and All Files, or nothing (every file) where All Files cannot be a filter
    /// ([`FileFilter::all_files`]); a Save dialog offers its own type. Import offers the
    /// supported types together, then each, then All Files where it can.
    pub fn filters(self) -> Vec<FileFilter> {
        let open = |own: FileFilter| {
            FileFilter::all_files()
                .map(|all| vec![own, all])
                .unwrap_or_default()
        };
        match self {
            Self::OpenGcode => open(FileFilter::GCODE),
            Self::OpenDesign => open(FileFilter::DESIGN),
            Self::SaveGcode => vec![FileFilter::GCODE],
            Self::SaveDesign => vec![FileFilter::DESIGN],
            Self::ImportDesign => import_design_filters(false),
            Self::ImportImage => vec![FileFilter::IMAGE],
            Self::ExportGcode => vec![FileFilter::GCODE_EXPORT],
            Self::ExportSvg => vec![FileFilter::SVG],
            // GTK's preset dialogs offered no file types.
            Self::LoadCamPreset | Self::SaveCamPreset => Vec::new(),
            // A folder has no file types.
            Self::OpenGerberFolder => Vec::new(),
            Self::OpenEngravingImage => vec![FileFilter::ENGRAVING_IMAGE],
            // GTK's firmware settings dialogs offered no file types.
            Self::ExportFirmwareSettings | Self::ImportFirmwareSettings => Vec::new(),
            Self::ImportGtcPackage => open(FileFilter::GTC_PACKAGE),
            Self::ImportGtcJson => open(FileFilter::JSON),
            // GTK's export offered JSON only.
            Self::ExportCustomTools => vec![FileFilter::JSON],
        }
    }
}

/// The file types the designer's Import dialog offers, as GTK's: the supported files, SVG,
/// DXF and, when Settings' "STL File Import" is on, STL; then All Files.
///
/// # Arguments
/// * `stl_import` - Whether STL import is switched on (`ui.enable_stl_import`).
pub fn import_design_filters(stl_import: bool) -> Vec<FileFilter> {
    let mut filters = if stl_import {
        vec![
            FileFilter::IMPORT_WITH_STL,
            FileFilter::SVG,
            FileFilter::DXF,
            FileFilter::STL,
        ]
    } else {
        vec![FileFilter::IMPORT, FileFilter::SVG, FileFilter::DXF]
    };
    filters.extend(FileFilter::all_files());
    filters
}

/// A file type a dialog offers: the English msgid of its name and its extensions.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct FileFilter {
    /// The English msgid of the name the dialog lists.
    pub name_msgid: &'static str,
    /// The extensions, without the dot.
    pub extensions: &'static [&'static str],
}

impl FileFilter {
    /// G-code programs, with the extensions the GTK editor offered.
    pub const GCODE: Self = Self {
        name_msgid: n_!("G-Code Files"),
        extensions: &["gcode", "nc", "gc", "tap"],
    };
    /// GCodeKit design files.
    pub const DESIGN: Self = Self {
        name_msgid: n_!("GCodeKit Design Files"),
        extensions: &["gckd", "gck5"],
    };

    /// The vector files the designer imports (GTK's "Supported Files").
    pub const IMPORT: Self = Self {
        name_msgid: n_!("Supported Files"),
        extensions: &["svg", "dxf"],
    };
    /// GTK's "Supported Files" with STL import switched on in the settings.
    pub const IMPORT_WITH_STL: Self = Self {
        name_msgid: n_!("Supported Files"),
        extensions: &["svg", "dxf", "stl"],
    };
    /// STL models (imported as their shadow, when the settings allow it).
    pub const STL: Self = Self {
        name_msgid: n_!("STL Files"),
        extensions: &["stl"],
    };
    /// SVG drawings.
    pub const SVG: Self = Self {
        name_msgid: n_!("SVG Files"),
        extensions: &["svg"],
    };
    /// DXF drawings.
    pub const DXF: Self = Self {
        name_msgid: n_!("DXF Files"),
        extensions: &["dxf"],
    };
    /// The images the raster import reads (GTK's JPEG, PNG, BMP, TIFF and WebP types).
    pub const IMAGE: Self = Self {
        name_msgid: n_!("Image files"),
        extensions: &["png", "jpg", "jpeg", "bmp", "tif", "tiff", "webp"],
    };
    /// The images the Laser Image Engraver reads (GTK's PNG, JPEG, BMP, GIF and TIFF types).
    pub const ENGRAVING_IMAGE: Self = Self {
        name_msgid: n_!("Image files"),
        extensions: &["png", "jpg", "jpeg", "bmp", "gif", "tif", "tiff"],
    };
    /// GTC tool catalog packages.
    pub const GTC_PACKAGE: Self = Self {
        name_msgid: n_!("GTC Packages"),
        extensions: &["zip"],
    };
    /// JSON files (GTC catalogs, exported custom tools).
    pub const JSON: Self = Self {
        name_msgid: "JSON",
        extensions: &["json"],
    };
    /// The G-code types the designer's export offers (GTK's).
    pub const GCODE_EXPORT: Self = Self {
        name_msgid: n_!("G-Code Files"),
        extensions: &["nc", "gcode", "gc"],
    };

    /// The "All Files" choice in a form that matches every file, names without a dot
    /// included, on the dialog backends of this platform; `None` where no filter can, and an
    /// Open dialog then has no filters at all. This is the one place that knows the backends
    /// (rfd 0.17.2 through craft-dialogs):
    ///
    /// - Windows: rfd writes `*.{ext}`, so `*` becomes `*.*`, which the Windows file dialog
    ///   matches against every file, extensionless ones included.
    /// - Linux and the BSDs: the desktop portal keeps `*` as the glob `*`. Its zenity fallback
    ///   writes `*.{ext}` for each extension into one space-separated pattern list, which
    ///   zenity splits on spaces; `*` alone would give only `*.*` (names with a dot), so `" *"`
    ///   is added, which zenity reads as `*.` and `*`. The portal reads `" *"` as the glob
    ///   `*. *`, which matches nothing extra and is harmless.
    /// - macOS: craft-dialogs' panel merges every filter's extensions into one allowed-types
    ///   list with no filter menu, and would take `*` for an extension, so no filter can offer
    ///   a choice: `None`.
    pub fn all_files() -> Option<Self> {
        #[cfg(target_os = "macos")]
        let all = None;
        #[cfg(target_os = "windows")]
        let all = Some(Self {
            name_msgid: n_!("All Files"),
            extensions: &["*"],
        });
        #[cfg(not(any(target_os = "macos", target_os = "windows")))]
        let all = Some(Self {
            name_msgid: n_!("All Files"),
            extensions: &["*", " *"],
        });
        all
    }
}

/// A question with a fixed set of answers.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ConfirmRequest {
    /// The editor's G-code has unsaved changes: Save, Discard or Cancel.
    UnsavedGcode,
    /// The design has unsaved changes: Save, Discard or Cancel.
    UnsavedDesign,
    /// The CNC Tools window holds an unsaved tool: Save, Discard or Cancel.
    UnsavedTool,
    /// The Materials Manager holds an unsaved material: Save, Discard or Cancel.
    UnsavedMaterial,
    /// The Device Manager holds an unsaved device profile: Save, Discard or Cancel.
    UnsavedDevice,
    /// Changed settings take effect after a restart: restart now (Yes) or later (No).
    Restart,
    /// A job is running while the user quits: stop it and quit (Yes) or keep running (No).
    StopJob,
    /// Zero All Axes: set the current X/Y/Z as 0 of the work coordinate system it names,
    /// the active one when asked (Yes), or not (No). `None`: a controller without systems
    /// (Repetier), whose zero sets the position with `G92`.
    ZeroAllAxes(Option<Wcs>),
    /// Clear editor: remove all G-code from the editor (Yes), or keep it (No).
    ClearEditor,
}

impl ConfirmRequest {
    /// The English msgids of the title and the message.
    fn msgids(self) -> (&'static str, &'static str) {
        match self {
            Self::UnsavedGcode => (
                n_!("Unsaved changes"),
                n_!("The G-code in the editor has unsaved changes. Save them before continuing?"),
            ),
            Self::UnsavedDesign => (
                n_!("Unsaved changes"),
                n_!("The current design has unsaved changes. Save them before continuing?"),
            ),
            Self::UnsavedTool => (
                n_!("Unsaved changes"),
                n_!("The tool has unsaved changes. Save them before continuing?"),
            ),
            Self::UnsavedMaterial => (
                n_!("Unsaved changes"),
                n_!("The material has unsaved changes. Save them before continuing?"),
            ),
            Self::UnsavedDevice => (
                n_!("Unsaved changes"),
                n_!("The device profile has unsaved changes. Save them before continuing?"),
            ),
            Self::Restart => (
                n_!("Restart to apply settings?"),
                n_!("GCodeKit5 must restart to use the new settings. \
                     The machine connection will be closed. Restart now?"),
            ),
            Self::StopJob => (
                n_!("Stop job and quit?"),
                n_!("A job is running. Quitting stops it and closes the machine connection."),
            ),
            Self::ClearEditor => (
                n_!("Clear the editor?"),
                n_!("All G-code in the editor will be removed."),
            ),
            Self::ZeroAllAxes(Some(_)) => (
                n_!("Zero all work axes?"),
                n_!("This sets work coordinate system {wcs} so the current X/Y/Z becomes 0."),
            ),
            Self::ZeroAllAxes(None) => (
                n_!("Zero all work axes?"),
                n_!("This sets the work position (G92) so the current X/Y/Z becomes 0."),
            ),
        }
    }
}

/// How a confirmation was answered.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ConfirmChoice {
    /// Yes: save first (unsaved changes), or restart now.
    Yes,
    /// No: go on without saving (unsaved changes), or restart later.
    No,
    /// Do nothing (unsaved changes; also a dismissed dialog).
    Cancel,
}

impl ConfirmChoice {
    /// The answer to an unsaved-changes prompt ([`ConfirmRequest::UnsavedGcode`],
    /// [`ConfirmRequest::UnsavedDesign`], [`ConfirmRequest::UnsavedTool`],
    /// [`ConfirmRequest::UnsavedMaterial`], [`ConfirmRequest::UnsavedDevice`]): Yes is Save,
    /// No is Discard.
    pub fn as_unsaved(self) -> UnsavedChangesChoice {
        match self {
            Self::Yes => UnsavedChangesChoice::Save,
            Self::No => UnsavedChangesChoice::Discard,
            Self::Cancel => UnsavedChangesChoice::Cancel,
        }
    }
}

/// What a dialog was asked for; an [`DialogAnswer::Unavailable`] answer carries it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Asked {
    /// A file dialog ([`Dialogs::open_file`], [`Dialogs::save_file`]).
    File(FilePurpose),
    /// A confirmation asked by a feature ([`Dialogs::confirm`]).
    Confirm(ConfirmRequest),
    /// A confirmation asked by the quit flow ([`Dialogs::confirm_quit`]).
    Quit(ConfirmRequest),
}

/// A dialog's answer, with what it was asked for.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum DialogAnswer {
    /// A file dialog chose `path`.
    File {
        /// What the dialog was for.
        purpose: FilePurpose,
        /// The chosen file.
        path: PathBuf,
    },
    /// A file dialog was closed without choosing.
    Cancelled {
        /// What the dialog was for.
        purpose: FilePurpose,
    },
    /// A confirmation asked by a feature ([`Dialogs::confirm`]) was answered.
    Confirm {
        /// The question.
        request: ConfirmRequest,
        /// The answer.
        choice: ConfirmChoice,
    },
    /// A confirmation asked by the quit flow ([`Dialogs::confirm_quit`]) was answered; kept
    /// apart so the same question asked by another feature never decides a quit.
    QuitConfirm {
        /// The question.
        request: ConfirmRequest,
        /// The answer.
        choice: ConfirmChoice,
    },
    /// No dialog could be shown, or it stopped without an answer.
    Unavailable {
        /// What the dialog was asked for.
        asked: Asked,
        /// Why, as craft-dialogs reports it.
        reason: String,
    },
}

/// What an answer continues: the request, with the labels of its custom buttons.
#[derive(Clone, Debug)]
enum Then {
    File(FilePurpose),
    Confirm {
        /// [`Asked::Confirm`] or [`Asked::Quit`].
        asked: Asked,
        request: ConfirmRequest,
        save: String,
        discard: String,
    },
}

/// The app's dialogs: requests waiting to be shown, the one on screen, and answers not yet
/// polled.
pub struct Dialogs {
    inner: craft_dialogs::Dialogs<Then>,
    translator: Translator,
    answers: VecDeque<DialogAnswer>,
    /// Preferences > General's default folder, as last seen ([`Dialogs::follow_folder`]).
    configured_folder: PathBuf,
    /// The folder of the last file opened or saved since the default folder last changed
    /// ([`Dialogs::remember_folder`]).
    last_folder: Option<PathBuf>,
}

impl std::fmt::Debug for Dialogs {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Dialogs")
            .field("waiting", &self.waiting())
            .field("answers", &self.answers)
            .finish_non_exhaustive()
    }
}

impl Dialogs {
    /// Dialogs labelled through `translator`; `wake` asks the app for a frame when an answer
    /// comes in.
    ///
    /// # Arguments
    /// * `translator` - Translates titles, filter names and buttons.
    /// * `wake` - Called from any thread when a dialog answered.
    pub fn new(translator: Translator, wake: Wake) -> Self {
        Self {
            inner: craft_dialogs::Dialogs::new(wake),
            translator,
            answers: VecDeque::new(),
            configured_folder: PathBuf::new(),
            last_folder: None,
        }
    }

    /// Follows Preferences > General's default folder
    /// (`file_processing.output_directory`): a change replaces the folder remembered from
    /// the last file. Call it every frame.
    ///
    /// # Arguments
    /// * `configured` - The folder in the settings (a file names its folder).
    pub fn follow_folder(&mut self, configured: &Path) {
        if self.configured_folder != configured {
            self.configured_folder = configured.to_path_buf();
            self.last_folder = None;
        }
    }

    /// Remembers the folder of `file`, so the next file dialog starts there, as GTK's did.
    ///
    /// # Arguments
    /// * `file` - A file just opened or saved.
    ///
    /// # Returns
    /// The folder remembered, for the caller to keep in the settings; `None` for a path
    /// without one.
    pub fn remember_folder(&mut self, file: &Path) -> Option<PathBuf> {
        let folder = file.parent().filter(|f| !f.as_os_str().is_empty())?;
        self.last_folder = Some(folder.to_path_buf());
        self.last_folder.clone()
    }

    /// The folder a file dialog starts in: the last file's, else the default folder (its
    /// folder when it names a file); `None` without either.
    pub fn start_folder(&self) -> Option<PathBuf> {
        if let Some(last) = &self.last_folder {
            return Some(last.clone());
        }
        let configured = &self.configured_folder;
        if configured.as_os_str().is_empty() {
            None
        } else if configured.is_file() {
            configured.parent().map(Path::to_path_buf)
        } else {
            Some(configured.clone())
        }
    }

    /// Queues a dialog choosing one existing file.
    ///
    /// # Arguments
    /// * `purpose` - What the file is for; it comes back with the answer.
    /// * `filters` - The file types to offer, in order.
    pub fn open_file(&mut self, purpose: FilePurpose, filters: &[FileFilter]) {
        self.ask_file(FileDialog::open(), purpose, filters);
    }

    /// Queues a dialog choosing one folder.
    ///
    /// # Arguments
    /// * `purpose` - What the folder is for; it comes back with the answer.
    pub fn open_folder(&mut self, purpose: FilePurpose) {
        self.ask_file(FileDialog::folder(), purpose, &[]);
    }

    /// Queues a dialog choosing where to save a file.
    ///
    /// # Arguments
    /// * `purpose` - What the file is for; it comes back with the answer.
    /// * `filters` - The file types to offer, in order.
    pub fn save_file(&mut self, purpose: FilePurpose, filters: &[FileFilter]) {
        self.ask_file(FileDialog::save(), purpose, filters);
    }

    /// Queues a dialog choosing where to save a file, suggesting `file_name`.
    ///
    /// # Arguments
    /// * `purpose` - What the file is for; it comes back with the answer.
    /// * `filters` - The file types to offer, in order.
    /// * `file_name` - The name the dialog starts with.
    pub fn save_file_named(
        &mut self,
        purpose: FilePurpose,
        filters: &[FileFilter],
        file_name: &str,
    ) {
        self.ask_file(FileDialog::save().file_name(file_name), purpose, filters);
    }

    fn ask_file(&mut self, dialog: FileDialog, purpose: FilePurpose, filters: &[FileFilter]) {
        let dialog = match self.start_folder() {
            Some(folder) => dialog.directory(folder),
            None => dialog,
        };
        let tr = &self.translator;
        let dialog = filters.iter().fold(
            dialog.title(tr.tr(purpose.title_msgid())),
            |dialog, filter| dialog.filter(tr.tr(filter.name_msgid), filter.extensions),
        );
        self.inner.ask(dialog, Then::File(purpose));
    }

    /// Queues a confirmation for a feature; it is answered with [`DialogAnswer::Confirm`].
    ///
    /// # Arguments
    /// * `request` - The question; it comes back with the answer.
    pub fn confirm(&mut self, request: ConfirmRequest) {
        self.ask_confirm(Asked::Confirm(request), request);
    }

    /// Queues a confirmation for the quit flow; it is answered with
    /// [`DialogAnswer::QuitConfirm`].
    ///
    /// # Arguments
    /// * `request` - The question; it comes back with the answer.
    pub fn confirm_quit(&mut self, request: ConfirmRequest) {
        self.ask_confirm(Asked::Quit(request), request);
    }

    fn ask_confirm(&mut self, asked: Asked, request: ConfirmRequest) {
        let tr = &self.translator;
        let (title, description) = request.msgids();
        let save = tr.tr(n_!("Save")).into_owned();
        let discard = tr.tr(n_!("Discard")).into_owned();
        let (level, buttons) = match request {
            ConfirmRequest::UnsavedGcode
            | ConfirmRequest::UnsavedDesign
            | ConfirmRequest::UnsavedTool
            | ConfirmRequest::UnsavedMaterial
            | ConfirmRequest::UnsavedDevice => (
                MessageLevel::Warning,
                MessageButtons::YesNoCancelCustom(
                    save.clone(),
                    discard.clone(),
                    tr.tr(n_!("Cancel")).into_owned(),
                ),
            ),
            ConfirmRequest::Restart => (MessageLevel::Info, MessageButtons::YesNo),
            ConfirmRequest::StopJob
            | ConfirmRequest::ZeroAllAxes(_)
            | ConfirmRequest::ClearEditor => (MessageLevel::Warning, MessageButtons::YesNo),
        };
        let message = Message {
            level,
            title: tr.tr(title).into_owned(),
            description: match request {
                ConfirmRequest::ZeroAllAxes(Some(wcs)) => {
                    tr.tr(description).replace("{wcs}", wcs.code())
                }
                ConfirmRequest::ZeroAllAxes(None) => tr.tr(description).into_owned(),
                ConfirmRequest::UnsavedGcode
                | ConfirmRequest::UnsavedDesign
                | ConfirmRequest::UnsavedTool
                | ConfirmRequest::UnsavedMaterial
                | ConfirmRequest::UnsavedDevice
                | ConfirmRequest::Restart
                | ConfirmRequest::StopJob
                | ConfirmRequest::ClearEditor => tr.tr(description).into_owned(),
            },
            buttons,
        };
        self.inner.ask_message(
            message,
            Then::Confirm {
                asked,
                request,
                save,
                discard,
            },
        );
    }

    /// Shows the next queued request unless a dialog is open. Call it on the main thread at
    /// the end of every frame with the window as `parent`.
    ///
    /// # Arguments
    /// * `parent` - The window the dialog belongs to (`eframe::Frame`), if any.
    pub fn end_frame<W: HasWindowHandle + HasDisplayHandle>(&mut self, parent: Option<&W>) {
        self.inner.show_pending(parent);
    }

    /// The next answer that came in, oldest first; each answer is returned once.
    pub fn poll(&mut self) -> Option<DialogAnswer> {
        let answers = self.inner.take_answers();
        self.answers.extend(
            answers
                .into_iter()
                .map(|(then, answer)| convert(then, answer)),
        );
        self.answers.pop_front()
    }

    /// Requests not answered yet: the one on screen and those queued behind it.
    pub fn waiting(&self) -> usize {
        self.inner.queued() + usize::from(self.inner.is_open())
    }

    /// Answers every request with `answer_with` instead of a native dialog (craft-dialogs'
    /// hook for tests and automation); `None` shows native dialogs again.
    ///
    /// # Arguments
    /// * `answer_with` - The stand-in for the native dialogs.
    pub fn set_override(&mut self, answer_with: Option<Override>) {
        self.inner.set_override(answer_with);
    }
}

/// The app's answer for craft-dialogs' `answer` to the request `then` continues.
fn convert(then: Then, answer: Answer) -> DialogAnswer {
    match (then, answer) {
        (Then::File(purpose), Answer::Unavailable(reason)) => DialogAnswer::Unavailable {
            asked: Asked::File(purpose),
            reason,
        },
        (Then::Confirm { asked, .. }, Answer::Unavailable(reason)) => {
            DialogAnswer::Unavailable { asked, reason }
        }
        (Then::File(purpose), Answer::Paths(paths)) => match paths.into_iter().next() {
            Some(path) => DialogAnswer::File { purpose, path },
            None => DialogAnswer::Cancelled { purpose },
        },
        (Then::File(purpose), Answer::Cancelled | Answer::Message(_)) => {
            DialogAnswer::Cancelled { purpose }
        }
        (
            Then::Confirm {
                asked,
                request,
                save,
                discard,
            },
            answer,
        ) => {
            let choice = match answer {
                Answer::Message(MessageResult::Yes | MessageResult::Ok) => ConfirmChoice::Yes,
                Answer::Message(MessageResult::No) => ConfirmChoice::No,
                Answer::Message(MessageResult::Custom(label)) if label == save => {
                    ConfirmChoice::Yes
                }
                Answer::Message(MessageResult::Custom(label)) if label == discard => {
                    ConfirmChoice::No
                }
                Answer::Message(MessageResult::Custom(_) | MessageResult::Cancel)
                | Answer::Cancelled
                | Answer::Paths(_)
                | Answer::Unavailable(_) => ConfirmChoice::Cancel,
            };
            match asked {
                Asked::Quit(_) => DialogAnswer::QuitConfirm { request, choice },
                Asked::Confirm(_) | Asked::File(_) => DialogAnswer::Confirm { request, choice },
            }
        }
    }
}

/// The English msgids the dialogs show, for the translation test.
pub fn msgids() -> Vec<&'static str> {
    let purposes = [
        FilePurpose::OpenGcode,
        FilePurpose::SaveGcode,
        FilePurpose::OpenDesign,
        FilePurpose::SaveDesign,
        FilePurpose::ImportDesign,
        FilePurpose::ImportImage,
        FilePurpose::ExportGcode,
        FilePurpose::ExportSvg,
        FilePurpose::LoadCamPreset,
        FilePurpose::SaveCamPreset,
        FilePurpose::OpenGerberFolder,
        FilePurpose::OpenEngravingImage,
        FilePurpose::ExportFirmwareSettings,
        FilePurpose::ImportFirmwareSettings,
        FilePurpose::ImportGtcPackage,
        FilePurpose::ImportGtcJson,
        FilePurpose::ExportCustomTools,
    ]
    .map(FilePurpose::title_msgid);
    let filters = [
        FileFilter::GCODE,
        FileFilter::DESIGN,
        FileFilter::IMPORT,
        FileFilter::SVG,
        FileFilter::DXF,
        FileFilter::STL,
        FileFilter::IMAGE,
        FileFilter::GTC_PACKAGE,
    ]
    .map(|f| f.name_msgid)
    .into_iter()
    .chain([n_!("All Files")]);
    let confirms = [
        ConfirmRequest::UnsavedGcode,
        ConfirmRequest::UnsavedDesign,
        ConfirmRequest::UnsavedTool,
        ConfirmRequest::UnsavedMaterial,
        ConfirmRequest::UnsavedDevice,
        ConfirmRequest::Restart,
        ConfirmRequest::StopJob,
        ConfirmRequest::ZeroAllAxes(Some(Wcs::G54)),
        ConfirmRequest::ZeroAllAxes(None),
        ConfirmRequest::ClearEditor,
    ]
    .into_iter()
    .flat_map(|request| {
        let (title, message) = request.msgids();
        [title, message]
    });
    purposes
        .into_iter()
        .chain(filters)
        .chain(confirms)
        .chain([n_!("Save"), n_!("Discard"), n_!("Cancel")])
        .collect()
}
