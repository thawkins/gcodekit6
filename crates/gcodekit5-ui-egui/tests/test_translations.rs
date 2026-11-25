//! Every new UI msgid of the egui front end has a translation in each shipped language.

use craft_window::Button;
use gcodekit5_app::i18n::Translator;
use gcodekit5_ui_egui::dialogs;
use gcodekit5_ui_egui::frame::caption_msgid;
use gcodekit5_ui_egui::menu;
use gcodekit5_ui_egui::windows::{about, settings};
use gcodekit5_ui_egui::workspace::PaneId;

const LANGUAGES: [&str; 5] = ["de", "es", "fr", "it", "pt"];

/// Translations that are spelled like the English msgid; the Translator cannot tell them
/// from a missing entry, so the po file is checked for them instead.
const IDENTICAL: [(&str, &str); 22] = [
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
