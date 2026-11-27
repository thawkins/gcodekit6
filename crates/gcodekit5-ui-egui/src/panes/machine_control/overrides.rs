//! The override section: feed, rapid and spindle override buttons with the percentages the
//! controller last reported. Each button sends its GRBL real-time override byte. Under them,
//! the "Set %" fields move the feed or spindle override to an exact percentage, one
//! confirmed step at a time (`gcodekit5_app::override_target`, driven by the pane).
//!
//! Dependencies: `egui`, `gcodekit5-app` (real-time commands, override targets,
//! translations).

use gcodekit5_app::i18n::Translator;
use gcodekit5_app::machine::RealtimeCommand;
use gcodekit5_app::override_target::OverrideKind;

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
    n_!("Set %"),
    n_!("Target feed override (%)"),
    n_!("Set the feed override to exactly the target percentage"),
    n_!("Target spindle override (%)"),
    n_!("Set the spindle override to exactly the target percentage"),
];

/// What the section asks for.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum Action {
    /// Send this override byte.
    Realtime(RealtimeCommand),
    /// Move `kind` to `percent` (Set %).
    SetTarget(OverrideKind, u16),
}

/// The Set % fields' values and whether a target is being reached.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) struct Targets {
    /// The feed override target, in percent.
    pub feed: u16,
    /// The spindle override target, in percent.
    pub spindle: u16,
    /// A target is being reached: the Set buttons wait (disabled).
    pub busy: bool,
}

impl Default for Targets {
    fn default() -> Self {
        Self {
            feed: 100,
            spindle: 100,
            busy: false,
        }
    }
}

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

/// The order: the stop button sits between reset and +1.
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

/// Width and height of an override button (a compact row).
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

/// The override percentage last reported, as text, and whether it is at a limit.
///
/// An override at its limit is marked with " MAX" (200 % and over) or " MIN" (the feed at
/// 10 % and under, the spindle at 50 % and under) in the error colour; `limits` gives those
/// bounds, `None` for the rapid override, which has none.
///
/// # Arguments
/// * `status` - The machine status with the last `Ov:` values.
/// * `pick` - Picks the override from (feed, rapid, spindle).
/// * `limits` - The (MIN, MAX) bounds in percent, if the override has them.
fn percent(
    status: &MachineStatus,
    pick: impl Fn(u16, u16, u16) -> u16,
    limits: Option<(u16, u16)>,
) -> (String, bool) {
    let Some(o) = status.overrides else {
        return ("-".to_string(), false);
    };
    let value = pick(o.feed, o.rapid, o.spindle);
    let marker = match limits {
        Some((_, max)) if value >= max => " MAX",
        Some((min, _)) if value <= min => " MIN",
        _ => "",
    };
    (format!("{value}%{marker}"), !marker.is_empty())
}

/// The feed override limits for the MIN/MAX marker, in percent.
const FEED_LIMITS: (u16, u16) = (10, 200);
/// The spindle override limits for the MIN/MAX marker, in percent.
const SPINDLE_LIMITS: (u16, u16) = (50, 200);

/// The percentage text, in the error colour at a limit.
fn value_text(ui: &egui::Ui, (text, limit): (String, bool)) -> egui::RichText {
    let text = egui::RichText::new(text);
    if limit {
        text.color(ui.visuals().error_fg_color)
    } else {
        text
    }
}

/// Draws one Set % field and its button; returns the target when Set was clicked.
fn target_field(
    ui: &mut egui::Ui,
    tr: &Translator,
    value: &mut u16,
    enabled: bool,
    (field_tip, set_tip): (&str, &str),
) -> Option<u16> {
    let field_tip = tr.tr(field_tip);
    ui.add_enabled(
        enabled,
        egui::DragValue::new(value)
            .range(10..=200)
            .speed(1.0)
            .suffix("%"),
    )
    .on_hover_text(field_tip.as_ref());
    let set = ui
        .add_enabled(enabled, egui::Button::new(tr.tr("Set %")))
        .on_hover_text(tr.tr(set_tip));
    set.clicked().then_some(*value)
}

/// The Set % field and button of `kind`; returns the action when Set was clicked.
fn kind_target(
    ui: &mut egui::Ui,
    tr: &Translator,
    kind: OverrideKind,
    enabled: bool,
    targets: &mut Targets,
) -> Option<Action> {
    let enabled = enabled && !targets.busy;
    // No taller than a label, so the field adds no height to the line it sits on.
    let row = ui.text_style_height(&egui::TextStyle::Body);
    ui.spacing_mut().interact_size.y = row;
    ui.spacing_mut().button_padding.y = 0.0;
    let (value, tips) = match kind {
        OverrideKind::Feed => (
            &mut targets.feed,
            (
                "Target feed override (%)",
                "Set the feed override to exactly the target percentage",
            ),
        ),
        OverrideKind::Spindle => (
            &mut targets.spindle,
            (
                "Target spindle override (%)",
                "Set the spindle override to exactly the target percentage",
            ),
        ),
    };
    target_field(ui, tr, value, enabled, tips).map(|percent| Action::SetTarget(kind, percent))
}

/// Draws the section, one row per override (label, percentage, buttons); returns what was
/// asked for. The Set % fields (feed, then spindle) sit on the heading's line in a wide
/// column; when `stacked` (a narrow column) each row's buttons go under its label and
/// percentage, and its Set % field beside them, so neither adds a line.
pub(super) fn show(
    ui: &mut egui::Ui,
    tr: &Translator,
    status: &MachineStatus,
    stacked: bool,
    targets: &mut Targets,
) -> Option<Action> {
    let enabled = status.connected;
    let mut target = None;
    let heading = tr.tr("Current Feed & Spindle");
    let title = crate::theme::section_heading(ui, &heading).rect;
    if !stacked {
        // Over the heading's line, right of its text: the line keeps the heading's height.
        let line = egui::Rect::from_min_max(
            egui::pos2(title.right() + 24.0, title.top()),
            egui::pos2(ui.max_rect().right(), title.bottom()),
        );
        let mut line = ui.new_child(
            egui::UiBuilder::new()
                .max_rect(line)
                .layout(egui::Layout::left_to_right(egui::Align::Center)),
        );
        for (kind, label) in [
            (OverrideKind::Feed, tr.tr("Feed:")),
            (OverrideKind::Spindle, tr.tr("Spindle:")),
        ] {
            line.weak(label);
            target = target.or(kind_target(&mut line, tr, kind, enabled, targets));
            line.add_space(12.0);
        }
    }
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
        (
            tr.tr("Feed:"),
            percent(status, |f, _, _| f, Some(FEED_LIMITS)),
            named(&FEED),
            Some(OverrideKind::Feed),
        ),
        (
            tr.tr("Spindle:"),
            percent(status, |_, _, s| s, Some(SPINDLE_LIMITS)),
            named(&SPINDLE),
            Some(OverrideKind::Spindle),
        ),
        (
            rapid.clone(),
            percent(status, |_, r, _| r, None),
            rapid_buttons,
            None,
        ),
    ];
    let mut clicked = None;
    if stacked {
        for (label, value, buttons, kind) in rows {
            ui.horizontal(|ui| {
                ui.weak(label);
                ui.label(value_text(ui, value));
                if let Some(kind) = kind {
                    ui.add_space(8.0);
                    target = target.or(kind_target(ui, tr, kind, enabled, targets));
                }
            });
            clicked = clicked.or(row(ui, enabled, buttons));
        }
    } else {
        egui::Grid::new("machine-control-overrides")
            .num_columns(3)
            .spacing([12.0, 6.0])
            .show(ui, |ui| {
                for (label, value, buttons, _) in rows {
                    ui.weak(label);
                    let value = value_text(ui, value);
                    ui.add_sized([72.0, BUTTON_SIZE.y], egui::Label::new(value));
                    clicked = clicked.or(row(ui, enabled, buttons));
                    ui.end_row();
                }
            });
    }
    clicked.map(Action::Realtime).or(target)
}
