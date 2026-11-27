//! Every new UI msgid of the egui front end has a translation in each shipped language.

use craft_window::Button;
use gcodekit6_app::i18n::Translator;
use gcodekit6_ui_egui::dialogs;
use gcodekit6_ui_egui::frame::caption_msgid;
use gcodekit6_ui_egui::menu;
use gcodekit6_ui_egui::panes::{cam_tools, console, designer, editor, machine_control, visualizer};
use gcodekit6_ui_egui::status_bar;
use gcodekit6_ui_egui::windows::{
    about, device_info, device_manager, firmware_settings, help, materials_manager, settings,
    tools_manager,
};
use gcodekit6_ui_egui::workspace::PaneId;

const LANGUAGES: [&str; 5] = ["de", "es", "fr", "it", "pt"];

/// Translations that are spelled like the English msgid; the Translator cannot tell them
/// from a missing entry, so the po file is checked for them instead.
const IDENTICAL: [(&str, &str); 265] = [
    // Designer context menu: GTK's short boolean labels.
    ("es", "Inter"),
    ("fr", "Union"),
    // Visualizer run preview speeds.
    ("de", "Normal"),
    ("es", "Normal"),
    ("pt", "Normal"),
    ("de", "Turbo"),
    ("es", "Turbo"),
    ("fr", "Turbo"),
    ("it", "Turbo"),
    ("pt", "Turbo"),
    // Visualizer 2D / 3D view mode.
    ("de", "2D"),
    ("es", "2D"),
    ("fr", "2D"),
    ("it", "2D"),
    ("pt", "2D"),
    ("de", "3D"),
    ("es", "3D"),
    ("fr", "3D"),
    ("it", "3D"),
    ("pt", "3D"),
    // Visualizer Simulation section.
    ("de", "Simulation"),
    ("fr", "Simulation"),
    ("de", "G-Code Editor"),
    ("de", "Designer"),
    ("it", "Designer"),
    ("pt", "Designer"),
    ("de", "Zoom"),
    ("es", "Zoom"),
    ("fr", "Zoom"),
    ("it", "Zoom"),
    ("pt", "Zoom"),
    ("de", "Cursor"),
    ("es", "Cursor"),
    ("pt", "Cursor"),
    ("es", "Inspector"),
    ("fr", "Machine"),
    ("fr", "Documentation"),
    ("fr", "Services"),
    ("fr", "Menu"),
    ("it", "File"),
    ("it", "Reset"),
    ("it", "Home"),
    ("it", "Menu"),
    ("pt", "Menu"),
    ("de", "System"),
    ("de", "Version"),
    ("es", "General"),
    // Materials Manager: words spelled the same.
    ("de", "Optional"),
    ("de", "Minimal"),
    ("fr", "Composite"),
    ("fr", "Excellent"),
    ("fr", "Minimal"),
    // Device Manager delete question.
    ("es", "No"),
    ("it", "No"),
    ("fr", "Version"),
    // Machine Control: technical terms kept as in English.
    ("de", "Port:"),
    ("de", "Host:"),
    ("de", "Baud:"),
    ("de", "Pause"),
    ("es", "Host:"),
    ("fr", "Pause"),
    ("it", "Host:"),
    ("it", "Baud:"),
    ("it", "Stop"),
    ("pt", "Baud:"),
    ("es", "Jog"),
    ("es", "Jog X+"),
    ("es", "Jog X-"),
    ("es", "Jog Y+"),
    ("es", "Jog Y-"),
    ("es", "Jog Z+"),
    ("es", "Jog Z-"),
    ("fr", "Jog"),
    ("fr", "Jog X+"),
    ("fr", "Jog X-"),
    ("fr", "Jog Y+"),
    ("fr", "Jog Y-"),
    ("fr", "Jog Z+"),
    ("fr", "Jog Z-"),
    ("es", "Error"),
    ("it", "Jog"),
    ("it", "Jog X+"),
    ("it", "Jog X-"),
    ("it", "Jog Y+"),
    ("it", "Jog Y-"),
    ("it", "Jog Z+"),
    ("it", "Jog Z-"),
    ("pt", "Jog"),
    ("pt", "Jog X+"),
    ("pt", "Jog X-"),
    ("pt", "Jog Y+"),
    ("pt", "Jog Y-"),
    ("pt", "Jog Z+"),
    ("pt", "Jog Z-"),
    // Visualizer OSD.
    ("de", "Position"),
    ("fr", "Position"),
    // Designer tools and the rotate menu.
    ("de", "Ellipse"),
    ("de", "Polygon"),
    ("fr", "Rectangle"),
    ("fr", "Ellipse"),
    ("fr", "Triangle"),
    ("de", "180°"),
    ("es", "180°"),
    ("fr", "180°"),
    ("it", "180°"),
    ("pt", "180°"),
    // Designer toolbox and layers: technical terms and short type names kept as in English.
    ("de", "CNC 3D"),
    ("de", "Laser 2D"),
    ("de", "Name"),
    ("de", "Position"),
    ("de", "Text"),
    ("de", "Tpulley"),
    ("es", "CNC 3D"),
    ("es", "Rect"),
    ("es", "Tri"),
    ("fr", "CNC 3D"),
    ("fr", "Ellip"),
    ("fr", "Image"),
    ("fr", "Laser 2D"),
    ("fr", "Passes"),
    ("fr", "Poly"),
    ("fr", "Position"),
    ("fr", "Rect"),
    ("fr", "Tpulley"),
    ("fr", "Tri"),
    ("it", "CNC 3D"),
    ("it", "Laser 2D"),
    ("it", "Tpulley"),
    ("it", "Tri"),
    ("pt", "CNC 3D"),
    ("pt", "Laser 2D"),
    ("pt", "Tpulley"),
    ("pt", "Tri"),
    // Designer properties: technical terms and names kept as in English.
    ("de", "(-Z:)"),
    ("de", "Atkinson"),
    ("de", "Bayer"),
    ("de", "Dithering:"),
    ("de", "Floyd-Ste."),
    ("de", "Horizontal"),
    ("de", "Radius:"),
    ("de", "Raster"),
    ("de", "Text:"),
    ("de", "X:"),
    ("de", "Y:"),
    ("es", "(-Z:)"),
    ("es", "Atkinson"),
    ("es", "Bayer"),
    ("es", "Floyd-Ste."),
    ("es", "Horizontal"),
    ("es", "Raster"),
    ("es", "Vertical"),
    ("es", "X:"),
    ("es", "Y:"),
    ("fr", "Atkinson"),
    ("fr", "Bayer"),
    ("fr", "Floyd-Ste."),
    ("fr", "Horizontal"),
    ("fr", "Vertical"),
    ("it", "(-Z:)"),
    ("it", "Atkinson"),
    ("it", "Bayer"),
    ("it", "Dithering:"),
    ("it", "Floyd-Ste."),
    ("it", "Offset"),
    ("it", "Offset:"),
    ("it", "Raster"),
    ("it", "X:"),
    ("it", "Y:"),
    ("pt", "(-Z:)"),
    ("pt", "Atkinson"),
    ("pt", "Bayer"),
    ("pt", "Floyd-Ste."),
    ("pt", "Horizontal"),
    ("pt", "Raster"),
    ("pt", "Vertical"),
    ("pt", "X:"),
    ("pt", "Y:"),
    ("de", "Linear"),
    ("pt", "Linear"),
    ("es", "Circular"),
    ("pt", "Circular"),
    ("fr", "Centre X"),
    ("fr", "Centre Y"),
    ("fr", "Angle"),
    ("fr", "Maintenance"),
    ("de", "Material:"),
    ("es", "Material:"),
    ("pt", "Material:"),
    ("fr", "Options"),
    // Tabbed box: the name of the boxes.py project.
    ("de", "Boxes"),
    ("es", "Boxes"),
    ("fr", "Boxes"),
    ("it", "Boxes"),
    ("pt", "Boxes"),
    // Tabbed box: offsets spelled as in English.
    ("es", "Offset X:"),
    ("it", "Offset X:"),
    ("es", "Offset Y:"),
    ("it", "Offset Y:"),
    // Probe Tools: the routine word and corner names read the same.
    ("de", "Routine"),
    ("fr", "Routine"),
    ("it", "Routine"),
    ("de", "Status:"),
    ("de", "X-min / Y-min"),
    ("de", "X-max / Y-min"),
    ("de", "X-min / Y-max"),
    ("de", "X-max / Y-max"),
    // Firmware Settings and Device Info: technical terms and names kept as in English.
    ("de", "Filter:"),
    ("de", "ID"),
    ("de", "Name:"),
    ("de", "Firmware:"),
    ("de", "Version:"),
    ("de", "CNC"),
    ("de", "Laser"),
    ("de", "Firmware"),
    ("de", "Plasma"),
    ("de", "Encoder"),
    ("de", "Modbus"),
    ("de", "RS485"),
    ("de", "Repetier EEPROM"),
    ("es", "ID"),
    ("es", "Firmware:"),
    ("es", "CNC"),
    ("es", "Firmware"),
    ("es", "Homing"),
    ("es", "Plasma"),
    ("es", "Modbus"),
    ("es", "RS485"),
    ("fr", "ID"),
    ("fr", "Description"),
    ("fr", "CNC"),
    ("fr", "Laser"),
    ("fr", "Firmware"),
    ("fr", "Plasma"),
    ("fr", "Modbus"),
    ("fr", "RS485"),
    ("fr", "Repetier EEPROM"),
    ("it", "ID"),
    ("it", "Firmware:"),
    ("it", "CNC"),
    ("it", "Laser"),
    ("it", "Firmware"),
    ("it", "Homing"),
    ("it", "I/O"),
    ("it", "Plasma"),
    ("it", "Encoder"),
    ("it", "Modbus"),
    ("it", "RS485"),
    ("pt", "ID"),
    ("pt", "Firmware:"),
    ("pt", "CNC"),
    ("pt", "Laser"),
    ("pt", "Firmware"),
    ("pt", "Plasma"),
    ("pt", "Modbus"),
    ("pt", "RS485"),
    // Help: the index page's title.
    ("de", "Index"),
    ("fr", "Index"),
    // Machine Control work zero: the axis Zero button.
    ("it", "Zero"),
    ("pt", "Zero"),
    // Machine Control: the Buffer readout.
    ("it", "Buffer:"),
    ("pt", "Buffer:"),
    // The startup notice about reset shortcuts: its OK button.
    ("de", "OK"),
    ("fr", "OK"),
    ("it", "OK"),
    ("pt", "OK"),
];

fn msgids() -> Vec<&'static str> {
    PaneId::ALL
        .into_iter()
        .map(PaneId::title_msgid)
        .chain(gcodekit6_ui_egui::workspace::msgids())
        .chain([Button::Minimize, Button::Maximize, Button::Close].map(caption_msgid))
        .chain(menu::msgids())
        .chain(settings::msgids())
        .chain(about::msgids())
        .chain(firmware_settings::msgids())
        .chain(device_info::msgids())
        .chain(device_manager::msgids())
        .chain(tools_manager::msgids())
        .chain(materials_manager::msgids())
        .chain(help::msgids())
        .chain(dialogs::msgids())
        .chain(machine_control::msgids())
        .chain(console::msgids())
        .chain(editor::msgids())
        .chain(visualizer::msgids())
        .chain(designer::msgids())
        .chain(cam_tools::msgids())
        .chain(gcodekit6_ui_egui::app::designer_files::msgids())
        .chain(gcodekit6_ui_egui::app::msgids())
        .chain(status_bar::MSGIDS.iter().copied())
        .chain(gcodekit6_ui_egui::machine::worker_msgids())
        .collect()
}

#[test]
fn test_every_ui_msgid_is_translated() {
    let po_dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../po");
    for lang in LANGUAGES {
        let tr = Translator::new(lang);
        assert_eq!(tr.language(), Some(lang), "no catalog for {lang}");
        let po = std::fs::read_to_string(po_dir.join(format!("{lang}.po"))).expect("read po file");
        for msgid in msgids() {
            let translated = tr.tr(msgid);
            if IDENTICAL.contains(&(lang, msgid)) {
                assert_eq!(
                    translated, msgid,
                    "{lang}: {msgid:?} is no longer identical"
                );
                let entry = format!("msgid \"{msgid}\"\nmsgstr \"{msgid}\"\n");
                assert!(po.contains(&entry), "{lang}: {msgid:?} has no entry");
            } else {
                assert_ne!(translated, msgid, "{lang}: {msgid:?} is not translated");
            }
        }
    }
}
