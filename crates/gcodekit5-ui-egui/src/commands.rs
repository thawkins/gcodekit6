//! The command catalog: every action the app offers from its key bindings and menus,
//! with a stable id, a menu label and default key bindings.
//!
//! [`keymap`] builds the `craft_keymap::Keymap` the app dispatches key presses through, and
//! [`crate::menu`] builds the menus from the same commands, so a binding shows in the menus as
//! soon as it exists. Ids are `group.action` (`file.open`), the form craft-keymap stores user
//! overrides under, so they must not change once released.
//!
//! Most bindings work anywhere in the window; the visualizer's view keys work only while the
//! Visualizer has the keyboard, and the designer's shape keys (Alt+L/R/T/V align, with
//! Alt+O and Alt+C for bottom and centre X, since Alt+B and Alt+H are menu mnemonics of the
//! in-window bar; Ctrl+G group and Ctrl+Shift+G ungroup) only while the Designer canvas has
//! it, never while a text field does. Ctrl+U is not an ungroup key: it stays Unlock everywhere.
//!
//! The jog keys are the numeric keypad digits (8/2 Y+/Y-, 4/6 X-/X+, 9/3 Z+/Z-), bound in
//! the Machine Control scope only: they run only after the user pressed in that pane, never
//! while a widget (a text field, an open dropdown) has the keyboard. egui reports the keypad
//! digits as the top-row digits, so those jog too, matching on the typed
//! character. A held key jogs continuously in Continuous
//! mode, until it is released (see `panes::machine_control`). For machine safety Escape and
//! the arrow keys are bound to nothing,
//! since an Escape or arrow key meant for a widget must never move or
//! reset the machine.
//!
//! Feed hold and soft reset each have a modifier chord, `Cmd+Shift+H` and `Cmd+Shift+K` (Ctrl
//! off macOS), that works anywhere in the window, also while a text field has the keyboard:
//! craft-keymap's typing rule holds back only plain keys and the text-editing chords, so the
//! chords reach the dispatcher while the bare `!` of feed hold goes to the text being typed.
//! Neither chord is one modifier away from a start, run, jog or motion command: Cmd+H,
//! Cmd+K, Shift+H and Shift+K are unbound (Cmd+R, run, is why soft reset is not
//! Cmd+Shift+R).
//!
//! Dependencies: `craft-keymap` (chords, bindings, scopes, the keymap).

use craft_keymap::{Bindable, Binding, CommandRef, Keymap, Platform, Scope, Scopes};

/// The scope of the bindings that work only while Machine Control has the keyboard.
pub const MACHINE_CONTROL_SCOPE: &str = "MachineControl";
/// The scope of the bindings that work only while the Visualizer has the keyboard.
pub const VISUALIZER_SCOPE: &str = "Visualizer";
/// The scope of the bindings that work only while the Designer canvas has the keyboard.
pub const DESIGNER_SCOPE: &str = "Designer";

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
    /// Open the firmware settings.
    OpenFirmwareSettings,
    /// Open the device information.
    OpenDeviceInfo,
    /// Open the documentation.
    OpenHelp,
    /// Show the About window.
    About,
    /// Align the designer selection by its left edges.
    AlignLeft,
    /// Align the designer selection by its right edges.
    AlignRight,
    /// Align the designer selection by its top edges.
    AlignTop,
    /// Align the designer selection by its bottom edges.
    AlignBottom,
    /// Align the designer selection on its horizontal centre.
    AlignCenterX,
    /// Align the designer selection on its vertical centre.
    AlignCenterY,
    /// Group the designer selection.
    Group,
    /// Ungroup the designer selection.
    Ungroup,
    /// Duplicate the designer selection 10 mm right and up (the canvas Ctrl+D).
    Duplicate,
    /// Fit the device work area into the designer view (the canvas D).
    FitDevice,
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
    pub const ALL: [Command; 57] = [
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
        Command::OpenFirmwareSettings,
        Command::OpenDeviceInfo,
        Command::OpenHelp,
        Command::About,
        Command::AlignLeft,
        Command::AlignRight,
        Command::AlignTop,
        Command::AlignBottom,
        Command::AlignCenterX,
        Command::AlignCenterY,
        Command::Group,
        Command::Ungroup,
        Command::Duplicate,
        Command::FitDevice,
    ];

    /// The command's catalog entry.
    fn spec(self) -> Spec {
        use Home::{App, Panel};
        const MC: Home = Panel(MACHINE_CONTROL_SCOPE);
        const VIS: Home = Panel(VISUALIZER_SCOPE);
        const DES: Home = Panel(DESIGNER_SCOPE);
        let (id, label, home, keys): (_, _, _, &'static [&'static str]) = match self {
            Command::FileNew => ("file.new", n_!("New 2D"), App, &["Cmd+N"]),
            Command::FileNew3d => ("file.new_3d", n_!("New 3D"), App, &[]),
            Command::FileOpen => ("file.open", n_!("Open"), App, &["Cmd+O"]),
            Command::FileSave => ("file.save", n_!("Save"), App, &["Cmd+S"]),
            Command::FileSaveAs => ("file.save_as", n_!("Save As..."), App, &["Cmd+Shift+S"]),
            Command::FileImport => ("file.import", n_!("Import"), App, &[]),
            // An app-wide key, and Ctrl+I on the designer canvas.
            Command::FileImportImage => (
                "file.import_image",
                n_!("Import Image..."),
                App,
                &["Cmd+Shift+I", "Designer: Cmd+I"],
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
            // A bare key that resumes a held job runs only while
            // Machine Control has the keyboard (its arming rules), never from a stray press on
            // another page.
            Command::CycleStart => ("machine.cycle_start", n_!("Cycle Start"), MC, &["Shift+`"]),
            // Never a bare Escape: it would reset the controller when closing a dropdown.
            Command::SoftReset => ("machine.soft_reset", n_!("Reset"), App, &["Cmd+Shift+K"]),
            Command::Home => ("machine.home", n_!("Home"), App, &["Cmd+Home"]),
            Command::Unlock => ("machine.unlock", n_!("Unlock"), App, &["Cmd+U"]),
            // The numeric keypad keys (egui folds them into the top-row digits), in the
            // Machine Control scope only; arrows move between widgets.
            Command::JogXPos => ("jog.x_pos", n_!("Jog X+"), MC, &["6"]),
            Command::JogXNeg => ("jog.x_neg", n_!("Jog X-"), MC, &["4"]),
            Command::JogYPos => ("jog.y_pos", n_!("Jog Y+"), MC, &["8"]),
            Command::JogYNeg => ("jog.y_neg", n_!("Jog Y-"), MC, &["2"]),
            Command::JogZPos => ("jog.z_pos", n_!("Jog Z+"), MC, &["9"]),
            Command::JogZNeg => ("jog.z_neg", n_!("Jog Z-"), MC, &["3"]),
            // The view keys of the visualizer, and of the designer canvas
            // (+ and = zoom in, - out, F fits, R resets).
            Command::ZoomFit => (
                "view.zoom_fit",
                n_!("Zoom to Fit"),
                VIS,
                &["F", "Designer: F"],
            ),
            Command::ZoomIn => (
                "view.zoom_in",
                n_!("Zoom In"),
                VIS,
                &["+", "Designer: +", "Designer: ="],
            ),
            Command::ZoomOut => ("view.zoom_out", n_!("Zoom Out"), VIS, &["-", "Designer: -"]),
            Command::ResetView => (
                "view.reset",
                n_!("Reset View"),
                VIS,
                &["Cmd+0", "Designer: R"],
            ),
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
            Command::OpenFirmwareSettings => (
                "tools.firmware_settings",
                n_!("Firmware Settings"),
                App,
                &[],
            ),
            Command::OpenDeviceInfo => ("tools.device_info", n_!("Device Info"), App, &[]),
            Command::OpenHelp => ("help.docs", n_!("Documentation"), App, &["F1"]),
            Command::About => ("help.about", n_!("About"), App, &[]),
            // The designer keys, on the canvas only.
            Command::AlignLeft => ("designer.align_left", n_!("Align Left"), DES, &["Alt+L"]),
            Command::AlignRight => ("designer.align_right", n_!("Align Right"), DES, &["Alt+R"]),
            Command::AlignTop => ("designer.align_top", n_!("Align Top"), DES, &["Alt+T"]),
            Command::AlignBottom => (
                "designer.align_bottom",
                n_!("Align Bottom"),
                DES,
                // Not Alt+B: that is the German Edit menu's ("Bearbeiten") mnemonic here.
                &["Alt+O"],
            ),
            Command::AlignCenterX => (
                "designer.align_center_x",
                n_!("Align Center X"),
                DES,
                // Not Alt+H: that is the Help menu's mnemonic in the in-window bar.
                &["Alt+C"],
            ),
            Command::AlignCenterY => (
                "designer.align_center_y",
                n_!("Align Center Y"),
                DES,
                &["Alt+V"],
            ),
            Command::Group => ("designer.group", n_!("Group"), DES, &["Cmd+G"]),
            Command::Ungroup => ("designer.ungroup", n_!("Ungroup"), DES, &["Cmd+Shift+G"]),
            Command::Duplicate => ("designer.duplicate", n_!("Duplicate"), DES, &["Cmd+D"]),
            Command::FitDevice => (
                "designer.fit_device",
                n_!("Fit to Device Working Area"),
                DES,
                &["D"],
            ),
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

    /// Whether this is a designer shape command: run from the canvas's keys and context menu,
    /// not from the menu bar.
    pub fn is_designer_shape_op(self) -> bool {
        matches!(
            self,
            Command::AlignLeft
                | Command::AlignRight
                | Command::AlignTop
                | Command::AlignBottom
                | Command::AlignCenterX
                | Command::AlignCenterY
                | Command::Group
                | Command::Ungroup
                | Command::Duplicate
        )
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

    /// Whether the command moves the machine or starts or continues a job: a jog, homing, a
    /// stream start or a cycle start (resume). Its keys never run while a text field has the
    /// keyboard, stay off the feed hold and soft reset keys and, outside Machine Control, need
    /// a modifier or a function key (`crate::shortcuts`). File > Run only previews the
    /// program in the Visualizer and sends nothing, so it is not one. Go to Work Zero, the
    /// quick probes and Zero have no keys (pane buttons only).
    pub fn starts_motion(self) -> bool {
        self.is_jog()
            || matches!(
                self,
                Command::StreamStart | Command::CycleStart | Command::Home
            )
    }

    /// Whether the command may run now, by any route it can arrive by: a key, the macOS menu
    /// (its key equivalents included) or the in-window menu bar (its keyboard navigation
    /// included). A command that moves the machine never runs while a text field has the
    /// keyboard: the key was meant for the text.
    ///
    /// # Arguments
    /// * `text_field_has_keyboard` - Whether a text field has the keyboard this frame.
    pub fn may_run(self, text_field_has_keyboard: bool) -> bool {
        !(text_field_has_keyboard && self.starts_motion())
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
        .with_panel(DESIGNER_SCOPE)
        // A panel's keys never run while another panel, or none, has the keyboard.
        .with_panel_fallthrough(false);
    let catalog = Command::ALL.into_iter().map(Command::bindable).collect();
    Keymap::new(catalog, scopes, Platform::current())
}
