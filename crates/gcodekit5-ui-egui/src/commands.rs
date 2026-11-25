//! The command catalog: every action the GTK front end offers from its accelerators and menus,
//! with a stable id, a menu label and default key bindings.
//!
//! [`keymap`] builds the `craft_keymap::Keymap` the app dispatches key presses through, and
//! [`crate::menu`] builds the menus from the same commands, so a binding shows in the menus as
//! soon as it exists. Ids are `group.action` (`file.open`), the form craft-keymap stores user
//! overrides under, so they must not change once released.
//!
//! Most bindings work anywhere in the window; the visualizer's view keys work only while the
//! Visualizer has the keyboard. For machine safety, the jog commands have no default keys and
//! Escape is bound to nothing: GTK never registered Escape or the arrow keys (the
//! accelerators.rs constants were unused), an Escape or arrow key meant for a widget must never
//! move or reset the machine, and craft-keymap cannot tell GTK's numeric-keypad jog keys from
//! the top-row digits. Jogging stays in the Machine Control pane.
//!
//! Feed hold and soft reset each have a modifier chord, `Cmd+Shift+H` and `Cmd+Shift+R` (Ctrl
//! off macOS), that works anywhere in the window, also while a text field has the keyboard:
//! craft-keymap's typing rule holds back only plain keys and the text-editing chords, so the
//! chords reach the dispatcher while the bare `!` of feed hold goes to the text being typed.
//!
//! Dependencies: `craft-keymap` (chords, bindings, scopes, the keymap).

use craft_keymap::{Bindable, Binding, CommandRef, Keymap, Platform, Scope, Scopes};

/// The scope of the bindings that work only while Machine Control has the keyboard.
pub const MACHINE_CONTROL_SCOPE: &str = "MachineControl";
/// The scope of the bindings that work only while the Visualizer has the keyboard.
pub const VISUALIZER_SCOPE: &str = "Visualizer";

/// One action the user can run from a key, a menu or a pane.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Command {
    /// New 2D design.
    FileNew,
    /// New 3D design.
    FileNew3d,
    /// Open a G-code file.
    FileOpen,
    /// Save the current file.
    FileSave,
    /// Save the current file under a new name.
    FileSaveAs,
    /// Import a design file.
    FileImport,
    /// Import a bitmap image.
    FileImportImage,
    /// Export the design as G-code.
    FileExportGcode,
    /// Export the design as SVG.
    FileExportSvg,
    /// Send the loaded G-code to the machine.
    FileRun,
    /// Close the window, asking about unsaved changes.
    Quit,
    /// Undo the last edit.
    Undo,
    /// Redo the last undone edit.
    Redo,
    /// Cut the selection to the clipboard.
    Cut,
    /// Copy the selection to the clipboard.
    Copy,
    /// Paste from the clipboard.
    Paste,
    /// Connect to the machine.
    Connect,
    /// Disconnect from the machine.
    Disconnect,
    /// Start streaming the loaded program.
    StreamStart,
    /// Pause streaming.
    StreamPause,
    /// Stop streaming.
    StreamStop,
    /// Real-time feed hold.
    FeedHold,
    /// Real-time cycle start / resume.
    CycleStart,
    /// Soft reset of the controller.
    SoftReset,
    /// Run the homing cycle.
    Home,
    /// Clear an alarm lock.
    Unlock,
    /// Jog +X.
    JogXPos,
    /// Jog -X.
    JogXNeg,
    /// Jog +Y.
    JogYPos,
    /// Jog -Y.
    JogYNeg,
    /// Jog +Z.
    JogZPos,
    /// Jog -Z.
    JogZNeg,
    /// Fit the toolpath into the view.
    ZoomFit,
    /// Zoom the view in.
    ZoomIn,
    /// Zoom the view out.
    ZoomOut,
    /// Reset the view's pan, zoom and angle.
    ResetView,
    /// Look from the top.
    ViewTop,
    /// Look from the front.
    ViewFront,
    /// Isometric view.
    ViewIso,
    /// Open the preferences.
    OpenSettings,
    /// Open the device manager.
    OpenDeviceManager,
    /// Open the CNC tools manager.
    OpenToolsManager,
    /// Open the materials manager.
    OpenMaterialsManager,
    /// Open the documentation.
    OpenHelp,
    /// Show the About window.
    About,
}

/// Where a default binding works.
#[derive(Clone, Copy, Debug)]
enum Home {
    /// Anywhere in the window.
    App,
    /// While the named pane has the keyboard.
    Panel(&'static str),
}

/// A command's id, label, home scope and default chords, in canonical text form.
struct Spec {
    id: &'static str,
    label: &'static str,
    home: Home,
    keys: &'static [&'static str],
}

impl Command {
    /// Every command, in menu order.
    pub const ALL: [Command; 45] = [
        Command::FileNew,
        Command::FileNew3d,
        Command::FileOpen,
        Command::FileSave,
        Command::FileSaveAs,
        Command::FileImport,
        Command::FileImportImage,
        Command::FileExportGcode,
        Command::FileExportSvg,
        Command::FileRun,
        Command::Quit,
        Command::Undo,
        Command::Redo,
        Command::Cut,
        Command::Copy,
        Command::Paste,
        Command::Connect,
        Command::Disconnect,
        Command::StreamStart,
        Command::StreamPause,
        Command::StreamStop,
        Command::FeedHold,
        Command::CycleStart,
        Command::SoftReset,
        Command::Home,
        Command::Unlock,
        Command::JogXPos,
        Command::JogXNeg,
        Command::JogYPos,
        Command::JogYNeg,
        Command::JogZPos,
        Command::JogZNeg,
        Command::ZoomFit,
        Command::ZoomIn,
        Command::ZoomOut,
        Command::ResetView,
        Command::ViewTop,
        Command::ViewFront,
        Command::ViewIso,
        Command::OpenSettings,
        Command::OpenDeviceManager,
        Command::OpenToolsManager,
        Command::OpenMaterialsManager,
        Command::OpenHelp,
        Command::About,
    ];

    /// The command's catalog entry.
    fn spec(self) -> Spec {
        use Home::{App, Panel};
        const MC: Home = Panel(MACHINE_CONTROL_SCOPE);
        const VIS: Home = Panel(VISUALIZER_SCOPE);
        let (id, label, home, keys): (_, _, _, &'static [&'static str]) = match self {
            Command::FileNew => ("file.new", n_!("New 2D"), App, &["Cmd+N"]),
            Command::FileNew3d => ("file.new_3d", n_!("New 3D"), App, &[]),
            Command::FileOpen => ("file.open", n_!("Open"), App, &["Cmd+O"]),
            Command::FileSave => ("file.save", n_!("Save"), App, &["Cmd+S"]),
            Command::FileSaveAs => ("file.save_as", n_!("Save As..."), App, &["Cmd+Shift+S"]),
            Command::FileImport => ("file.import", n_!("Import"), App, &[]),
            Command::FileImportImage => (
                "file.import_image",
                n_!("Import Image..."),
                App,
                &["Cmd+Shift+I"],
            ),
            Command::FileExportGcode => ("file.export_gcode", n_!("Export G-Code..."), App, &[]),
            Command::FileExportSvg => ("file.export_svg", n_!("Export SVG..."), App, &[]),
            Command::FileRun => ("file.run", n_!("Run"), App, &["Cmd+R", "F5"]),
            Command::Quit => ("app.quit", n_!("Quit"), App, &["Cmd+Q"]),
            Command::Undo => ("edit.undo", n_!("Undo"), App, &["Cmd+Z"]),
            Command::Redo => ("edit.redo", n_!("Redo"), App, &["Cmd+Y", "Cmd+Shift+Z"]),
            Command::Cut => ("edit.cut", n_!("Cut"), App, &["Cmd+X"]),
            Command::Copy => ("edit.copy", n_!("Copy"), App, &["Cmd+C"]),
            Command::Paste => ("edit.paste", n_!("Paste"), App, &["Cmd+V"]),
            Command::Connect => ("machine.connect", n_!("Connect"), App, &[]),
            Command::Disconnect => ("machine.disconnect", n_!("Disconnect"), App, &[]),
            Command::StreamStart => ("stream.start", n_!("Start Streaming"), App, &[]),
            Command::StreamPause => ("stream.pause", n_!("Pause Streaming"), App, &[]),
            Command::StreamStop => ("stream.stop", n_!("Stop Streaming"), App, &[]),
            Command::FeedHold => (
                "machine.feed_hold",
                n_!("Feed Hold"),
                App,
                &["!", "Cmd+Shift+H"],
            ),
            // craft-keymap has no `~` key. Shift+` types `~` only on US-style layouts; on others
            // (German, French, ...) this chord is a different key from the one typing `~`.
            Command::CycleStart => ("machine.cycle_start", n_!("Cycle Start"), App, &["Shift+`"]),
            // Never a bare Escape: it would reset the controller when closing a dropdown.
            Command::SoftReset => ("machine.soft_reset", n_!("Reset"), App, &["Cmd+Shift+R"]),
            Command::Home => ("machine.home", n_!("Home"), App, &["Cmd+Home"]),
            Command::Unlock => ("machine.unlock", n_!("Unlock"), App, &["Cmd+U"]),
            // No default keys: GTK jogs from the numeric keypad (8/2/4/6/9/3), which
            // craft-keymap folds into the top-row digits, and arrows move between widgets.
            Command::JogXPos => ("jog.x_pos", n_!("Jog X+"), MC, &[]),
            Command::JogXNeg => ("jog.x_neg", n_!("Jog X-"), MC, &[]),
            Command::JogYPos => ("jog.y_pos", n_!("Jog Y+"), MC, &[]),
            Command::JogYNeg => ("jog.y_neg", n_!("Jog Y-"), MC, &[]),
            Command::JogZPos => ("jog.z_pos", n_!("Jog Z+"), MC, &[]),
            Command::JogZNeg => ("jog.z_neg", n_!("Jog Z-"), MC, &[]),
            Command::ZoomFit => ("view.zoom_fit", n_!("Zoom to Fit"), VIS, &["F"]),
            Command::ZoomIn => ("view.zoom_in", n_!("Zoom In"), VIS, &["+"]),
            Command::ZoomOut => ("view.zoom_out", n_!("Zoom Out"), VIS, &["-"]),
            Command::ResetView => ("view.reset", n_!("Reset View"), VIS, &["Cmd+0"]),
            Command::ViewTop => ("view.top", n_!("Top"), App, &[]),
            Command::ViewFront => ("view.front", n_!("Front"), App, &[]),
            Command::ViewIso => ("view.iso", n_!("Isometric"), App, &[]),
            Command::OpenSettings => ("app.settings", n_!("Preferences"), App, &[]),
            Command::OpenDeviceManager => ("tools.device_manager", n_!("Device Manager"), App, &[]),
            Command::OpenToolsManager => ("tools.tools_manager", n_!("CNC Tools"), App, &[]),
            Command::OpenMaterialsManager => (
                "tools.materials_manager",
                n_!("Materials Manager"),
                App,
                &[],
            ),
            Command::OpenHelp => ("help.docs", n_!("Documentation"), App, &["F1"]),
            Command::About => ("help.about", n_!("About"), App, &[]),
        };
        Spec {
            id,
            label,
            home,
            keys,
        }
    }

    /// The command's stable id, as stored in shortcut overrides (`file.open`).
    pub fn id(self) -> &'static str {
        self.spec().id
    }

    /// The English msgid of the command's menu label, as listed in `po/gcodekit5.pot`.
    pub fn label_msgid(self) -> &'static str {
        self.spec().label
    }

    /// The command as craft-keymap and craft-menu name it.
    pub fn command_ref(self) -> CommandRef {
        CommandRef::new(self.id())
    }

    /// The command a craft `CommandRef` names, if it is one of ours without parameters.
    ///
    /// # Arguments
    /// * `command` - A command reported by the keymap or a menu.
    pub fn from_ref(command: &CommandRef) -> Option<Command> {
        if !command.params.is_null() {
            return None;
        }
        Command::ALL.into_iter().find(|c| c.id() == command.id)
    }

    /// Whether this is a jog command (bound to keys, never shown in the menus).
    pub fn is_jog(self) -> bool {
        matches!(
            self,
            Command::JogXPos
                | Command::JogXNeg
                | Command::JogYPos
                | Command::JogYNeg
                | Command::JogZPos
                | Command::JogZNeg
        )
    }

    /// The command's catalog entry for craft-keymap.
    fn bindable(self) -> Bindable {
        let spec = self.spec();
        let scope = match spec.home {
            Home::App => Scope::app(),
            // The panel names are constants that `Scope::new` accepts.
            Home::Panel(name) => Scope::new(name).unwrap_or_else(Scope::app),
        };
        let bindable = Bindable::new(self.command_ref(), spec.label, scope);
        spec.keys
            .iter()
            .filter_map(|text| match Binding::parse(text) {
                Ok(binding) => Some(binding),
                Err(error) => {
                    tracing::warn!(%error, text, "default binding does not parse");
                    None
                }
            })
            .fold(bindable, Bindable::with_default)
    }
}

/// The app's keymap with the default bindings, for the platform this build runs on.
pub fn keymap() -> Keymap {
    let scopes = Scopes::new()
        .with_panel(MACHINE_CONTROL_SCOPE)
        .with_panel(VISUALIZER_SCOPE)
        // A panel's keys never run while another panel, or none, has the keyboard.
        .with_panel_fallthrough(false);
    let catalog = Command::ALL.into_iter().map(Command::bindable).collect();
    Keymap::new(catalog, scopes, Platform::current())
}
