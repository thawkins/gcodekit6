//! The override section: feed, rapid and spindle override buttons with the percentages the
//! controller last reported. Each button sends its GRBL real-time override byte.
//!
//! Dependencies: `egui`, `gcodekit5-app` (real-time commands, translations).

use gcodekit5_app::i18n::Translator;
use gcodekit5_app::machine::RealtimeCommand;

use crate::machine::MachineStatus;

/// The msgids this section shows.
pub(super) const MSGIDS: &[&str] = &[
    n_!("Current Feed & Spindle"),
    n_!("Feed:"),
    n_!("Rapid:"),
    n_!("Spindle:"),
    n_!("Decrease feed rate by 10%"),
    n_!("Decrease feed rate by 1%"),
    n_!("Reset feed rate to 100%"),
    n_!("Increase feed rate by 1%"),
    n_!("Increase feed rate by 10%"),
    n_!("Decrease spindle speed by 10%"),
    n_!("Decrease spindle speed by 1%"),
    n_!("Reset spindle speed to 100%"),
    n_!("Increase spindle speed by 1%"),
    n_!("Increase spindle speed by 10%"),
    n_!("Stop spindle"),
];

/// One override button: its text, the msgid of its accessible name and tooltip, and the
/// byte it sends.
type OverrideButton = (&'static str, &'static str, RealtimeCommand);

const FEED: [OverrideButton; 5] = [
    (
        "-10%",
        n_!("Decrease feed rate by 10%"),
        RealtimeCommand::FeedOverrideMinus10,
    ),
    (
        "-1%",
        n_!("Decrease feed rate by 1%"),
        RealtimeCommand::FeedOverrideMinus1,
    ),
    (
        "100%",
        n_!("Reset feed rate to 100%"),
        RealtimeCommand::FeedOverrideReset,
    ),
    (
        "+1%",
        n_!("Increase feed rate by 1%"),
        RealtimeCommand::FeedOverridePlus1,
    ),
    (
        "+10%",
        n_!("Increase feed rate by 10%"),
        RealtimeCommand::FeedOverridePlus10,
    ),
];

const SPINDLE: [OverrideButton; 6] = [
    (
        "-10%",
        n_!("Decrease spindle speed by 10%"),
        RealtimeCommand::SpindleOverrideMinus10,
    ),
    (
        "-1%",
        n_!("Decrease spindle speed by 1%"),
        RealtimeCommand::SpindleOverrideMinus1,
    ),
    (
        "100%",
        n_!("Reset spindle speed to 100%"),
        RealtimeCommand::SpindleOverrideReset,
    ),
    (
        "+1%",
        n_!("Increase spindle speed by 1%"),
        RealtimeCommand::SpindleOverridePlus1,
    ),
    (
        "+10%",
        n_!("Increase spindle speed by 10%"),
        RealtimeCommand::SpindleOverridePlus10,
    ),
    ("Off", n_!("Stop spindle"), RealtimeCommand::SpindleStop),
];

/// Rapid override buttons; their accessible name is "Rapid:" and the text.
const RAPID: [(&str, RealtimeCommand); 3] = [
    ("25%", RealtimeCommand::RapidOverride25),
    ("50%", RealtimeCommand::RapidOverride50),
    ("100%", RealtimeCommand::RapidOverride100),
];

/// Draws one row of buttons; returns the command of the one clicked.
fn row(
    ui: &mut egui::Ui,
    enabled: bool,
    buttons: impl IntoIterator<Item = (String, &'static str, String, RealtimeCommand)>,
) -> Option<RealtimeCommand> {
    let mut clicked = None;
    ui.horizontal_wrapped(|ui| {
        for (name, text, hover, command) in buttons {
            let response = ui
                .add_enabled(enabled, egui::Button::new(text))
                .on_hover_text(&hover);
            response.widget_info(|| egui::WidgetInfo::labeled(egui::Role::Button, enabled, &name));
            if response.clicked() {
                clicked = Some(command);
            }
        }
    });
    clicked
}

/// The override percentage of the latest report, as text.
fn percent(status: &MachineStatus, pick: impl Fn(u16, u16, u16) -> u16) -> String {
    status
        .report
        .as_ref()
        .and_then(|r| r.overrides)
        .map_or_else(
            || "-".to_string(),
            |o| format!("{}%", pick(o.feed, o.rapid, o.spindle)),
        )
}

/// Draws the section; returns the override byte to send, if a button was clicked.
pub(super) fn show(
    ui: &mut egui::Ui,
    tr: &Translator,
    status: &MachineStatus,
) -> Option<RealtimeCommand> {
    let enabled = status.connected;
    ui.strong(tr.tr("Current Feed & Spindle"));
    let mut clicked = None;
    let named = |buttons: &[OverrideButton]| {
        buttons
            .iter()
            .map(|(text, msgid, command)| {
                let name = tr.tr(msgid).into_owned();
                (name.clone(), *text, name, *command)
            })
            .collect::<Vec<_>>()
    };
    ui.label(format!(
        "{} {}",
        tr.tr("Feed:"),
        percent(status, |f, _, _| f)
    ));
    clicked = clicked.or(row(ui, enabled, named(&FEED)));
    let rapid = tr.tr("Rapid:");
    ui.label(format!("{rapid} {}", percent(status, |_, r, _| r)));
    let rapid_buttons = RAPID.iter().map(|(text, command)| {
        let name = format!("{rapid} {text}");
        (name.clone(), *text, name, *command)
    });
    clicked = clicked.or(row(ui, enabled, rapid_buttons));
    ui.label(format!(
        "{} {}",
        tr.tr("Spindle:"),
        percent(status, |_, _, s| s)
    ));
    clicked.or(row(ui, enabled, named(&SPINDLE)))
}
