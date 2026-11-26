//! The override section: feed, rapid and spindle override buttons with the percentages the
//! controller last reported. Each button sends its GRBL real-time override byte.
//!
//! Dependencies: `egui`, `gcodekit5-app` (real-time commands, translations).

use gcodekit5_app::i18n::Translator;
use gcodekit5_app::machine::RealtimeCommand;

use crate::icons::Icon;
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

/// What an override button shows.
#[derive(Debug, Clone, Copy)]
enum Face {
    /// A short text such as `+10`.
    Text(&'static str),
    /// An icon (reset to 100%).
    Icon(Icon),
    /// An icon on the destructive red (stop spindle).
    Destructive(Icon),
}

/// One override button: what it shows, the msgid of its accessible name and tooltip, and
/// the byte it sends.
type OverrideButton = (Face, &'static str, RealtimeCommand);

const FEED: [OverrideButton; 5] = [
    (
        Face::Text("-10"),
        n_!("Decrease feed rate by 10%"),
        RealtimeCommand::FeedOverrideMinus10,
    ),
    (
        Face::Text("-1"),
        n_!("Decrease feed rate by 1%"),
        RealtimeCommand::FeedOverrideMinus1,
    ),
    (
        Face::Icon(Icon::Refresh),
        n_!("Reset feed rate to 100%"),
        RealtimeCommand::FeedOverrideReset,
    ),
    (
        Face::Text("+1"),
        n_!("Increase feed rate by 1%"),
        RealtimeCommand::FeedOverridePlus1,
    ),
    (
        Face::Text("+10"),
        n_!("Increase feed rate by 10%"),
        RealtimeCommand::FeedOverridePlus10,
    ),
];

/// GTK's order: the stop button sits between reset and +1.
const SPINDLE: [OverrideButton; 6] = [
    (
        Face::Text("-10"),
        n_!("Decrease spindle speed by 10%"),
        RealtimeCommand::SpindleOverrideMinus10,
    ),
    (
        Face::Text("-1"),
        n_!("Decrease spindle speed by 1%"),
        RealtimeCommand::SpindleOverrideMinus1,
    ),
    (
        Face::Icon(Icon::Refresh),
        n_!("Reset spindle speed to 100%"),
        RealtimeCommand::SpindleOverrideReset,
    ),
    (
        Face::Destructive(Icon::Stop),
        n_!("Stop spindle"),
        RealtimeCommand::SpindleStop,
    ),
    (
        Face::Text("+1"),
        n_!("Increase spindle speed by 1%"),
        RealtimeCommand::SpindleOverridePlus1,
    ),
    (
        Face::Text("+10"),
        n_!("Increase spindle speed by 10%"),
        RealtimeCommand::SpindleOverridePlus10,
    ),
];

/// Rapid override buttons; their accessible name is "Rapid:" and the text.
const RAPID: [(&str, RealtimeCommand); 3] = [
    ("25%", RealtimeCommand::RapidOverride25),
    ("50%", RealtimeCommand::RapidOverride50),
    ("100%", RealtimeCommand::RapidOverride100),
];

/// Width and height of an override button (GTK's compact row).
const BUTTON_SIZE: egui::Vec2 = egui::vec2(48.0, 26.0);

/// The button of `face`.
fn button(ui: &egui::Ui, face: Face) -> egui::Button<'static> {
    let button = match face {
        Face::Text(text) => egui::Button::new(egui::RichText::new(text).strong()),
        Face::Icon(icon) => egui::Button::image(icon.themed(ui)),
        Face::Destructive(icon) => egui::Button::image(icon.themed(ui).tint(egui::Color32::WHITE))
            .fill(crate::theme::destructive_fill(crate::theme::mode_of(ui))),
    };
    button.min_size(BUTTON_SIZE)
}

/// Draws one row of buttons; returns the command of the one clicked.
fn row(
    ui: &mut egui::Ui,
    enabled: bool,
    buttons: impl IntoIterator<Item = (String, Face, String, RealtimeCommand)>,
) -> Option<RealtimeCommand> {
    let mut clicked = None;
    ui.horizontal(|ui| {
        ui.spacing_mut().item_spacing.x = 4.0;
        for (name, face, hover, command) in buttons {
            let widget = button(ui, face);
            let response = ui.add_enabled(enabled, widget).on_hover_text(&hover);
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

/// Draws the section, one row per override (label, percentage, buttons); returns the
/// override byte to send, if a button was clicked. When `stacked` (a narrow column) each
/// row's buttons go under its label and percentage.
pub(super) fn show(
    ui: &mut egui::Ui,
    tr: &Translator,
    status: &MachineStatus,
    stacked: bool,
) -> Option<RealtimeCommand> {
    let enabled = status.connected;
    crate::theme::section_heading(ui, &tr.tr("Current Feed & Spindle"));
    let named = |buttons: &[OverrideButton]| {
        buttons
            .iter()
            .map(|(face, msgid, command)| {
                let name = tr.tr(msgid).into_owned();
                (name.clone(), *face, name, *command)
            })
            .collect::<Vec<_>>()
    };
    let rapid = tr.tr("Rapid:");
    let rapid_buttons = RAPID
        .iter()
        .map(|(text, command)| {
            let name = format!("{rapid} {text}");
            (name.clone(), Face::Text(text), name, *command)
        })
        .collect::<Vec<_>>();
    let rows = [
        (tr.tr("Feed:"), percent(status, |f, _, _| f), named(&FEED)),
        (
            tr.tr("Spindle:"),
            percent(status, |_, _, s| s),
            named(&SPINDLE),
        ),
        (rapid.clone(), percent(status, |_, r, _| r), rapid_buttons),
    ];
    let mut clicked = None;
    if stacked {
        for (label, value, buttons) in rows {
            ui.horizontal(|ui| {
                ui.weak(label);
                ui.label(value);
            });
            clicked = clicked.or(row(ui, enabled, buttons));
        }
        return clicked;
    }
    egui::Grid::new("machine-control-overrides")
        .num_columns(3)
        .spacing([12.0, 6.0])
        .show(ui, |ui| {
            for (label, value, buttons) in rows {
                ui.weak(label);
                ui.add_sized([56.0, BUTTON_SIZE.y], egui::Label::new(value));
                clicked = clicked.or(row(ui, enabled, buttons));
                ui.end_row();
            }
        });
    clicked
}
