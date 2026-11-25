//! Every new UI msgid of the egui front end has a translation in each shipped language.

use craft_window::Button;
use gcodekit5_app::i18n::Translator;
use gcodekit5_ui_egui::dialogs;
use gcodekit5_ui_egui::frame::caption_msgid;
use gcodekit5_ui_egui::menu;
use gcodekit5_ui_egui::panes::{cam_tools, console, designer, editor, machine_control, visualizer};
use gcodekit5_ui_egui::status_bar;
use gcodekit5_ui_egui::windows::{about, settings};
use gcodekit5_ui_egui::workspace::PaneId;

const LANGUAGES: [&str; 5] = ["de", "es", "fr", "it", "pt"];

/// Translations that are spelled like the English msgid; the Translator cannot tell them
/// from a missing entry, so the po file is checked for them instead.
const IDENTICAL: [(&str, &str); 173] = [
    ("de", "G-Code Editor"),
    ("de", "Designer"),
    ("it", "Designer"),
    ("pt", "Designer"),
    ("de", "Zoom"),
    ("es", "Zoom"),
    ("fr", "Zoom"),
    ("it", "Zoom"),
    ("pt", "Zoom"),
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
];

fn msgids() -> Vec<&'static str> {
    PaneId::ALL
        .into_iter()
        .map(PaneId::title_msgid)
        .chain([Button::Minimize, Button::Maximize, Button::Close].map(caption_msgid))
        .chain(menu::msgids())
        .chain(settings::msgids())
        .chain(about::msgids())
        .chain(dialogs::msgids())
        .chain(machine_control::msgids())
        .chain(console::msgids())
        .chain(editor::msgids())
        .chain(visualizer::msgids())
        .chain(designer::msgids())
        .chain(cam_tools::msgids())
        .chain(gcodekit5_ui_egui::app::designer_files::msgids())
        .chain(status_bar::MSGIDS.iter().copied())
        .chain(gcodekit5_ui_egui::machine::worker_msgids())
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
