//! The panel's sections, one module per property group, and what they share.
//!
//! Each section shows its rows from the [`Summary`] and applies a committed edit to every
//! selected shape through the model's undo commands, one step per edit (an edit that
//! changes nothing records nothing). Every step marks the design modified; the canvas
//! repaints from the design on the next frame.
//!
//! Dependencies: `egui`, `gcodekit5-designer` (model and commands), `gcodekit5-app`
//! (translations).

pub mod cam;
pub mod dimensions;
pub mod effects;
pub mod gear_sprocket;
pub mod geometry;
pub mod image;
pub mod laser_override;
pub mod text;

use gcodekit5_app::i18n::Translator;
use gcodekit5_core::units::MeasurementSystem;
use gcodekit5_designer::DesignerState;
use gcodekit5_designer::canvas::DrawingObject;
use gcodekit5_designer::commands::{ChangeProperty, CompositeCommand, DesignerCommand};

use super::builders::{Check, Edit, Fields};
use super::update::Summary;
use crate::panes::designer::DesignerPane;

/// What every section needs for one frame.
pub struct Ctx<'a> {
    /// The designer, whose design is edited.
    pub designer: &'a mut DesignerPane,
    /// The fields being edited.
    pub edits: &'a mut Vec<Edit>,
    /// The inline errors: field key and msgid.
    pub errors: &'a [(&'static str, &'static str)],
    /// The selection as shown.
    pub summary: &'a Summary,
    /// The display units.
    pub units: MeasurementSystem,
    /// Translator for the current UI language.
    pub tr: &'a Translator,
}

impl Ctx<'_> {
    /// The fields of this frame.
    pub fn fields(&mut self) -> Fields<'_> {
        Fields {
            edits: self.edits,
            selection: &self.summary.ids,
            errors: self.errors,
            tr: self.tr,
        }
    }

    /// The design.
    pub fn state(&mut self) -> &mut DesignerState {
        self.designer.state_mut()
    }
}

/// A section's `apply`: `None` when the field is not the section's.
type SectionApply = fn(&mut Ctx<'_>, &str, &str) -> Option<Check>;

/// Applies the committed `text` of field `key` to the selected shapes, through the section
/// the field belongs to.
///
/// # Arguments
/// * `cx` - The frame's context; its selection is the shapes the edit belongs to.
/// * `key` - The field.
/// * `text` - The committed text.
///
/// # Errors
/// The inline error's msgid when the text is invalid or out of range (nothing applied).
pub fn apply(cx: &mut Ctx<'_>, key: &str, text: &str) -> Check {
    let sections: [SectionApply; 8] = [
        dimensions::apply,
        geometry::apply,
        text::apply,
        gear_sprocket::apply,
        effects::apply,
        cam::apply,
        image::apply,
        laser_override::apply,
    ];
    sections
        .iter()
        .find_map(|apply| apply(cx, key, text))
        .unwrap_or(Ok(()))
}

/// Applies `modify` to a copy of every selected shape and records the changed ones as one
/// undo step named `name`.
///
/// # Arguments
/// * `state` - The design.
/// * `name` - The undo step's name.
/// * `modify` - Changes a copy of one shape; returns whether it changed anything.
///
/// # Returns
/// Whether a step was recorded.
pub fn change_selected<F>(state: &mut DesignerState, name: &str, mut modify: F) -> bool
where
    F: FnMut(&mut DrawingObject) -> bool,
{
    let commands: Vec<DesignerCommand> = state
        .canvas
        .shapes()
        .filter(|s| s.selected)
        .filter_map(|obj| {
            let mut new_state = obj.clone();
            modify(&mut new_state).then(|| {
                DesignerCommand::ChangeProperty(ChangeProperty {
                    id: obj.id,
                    old_state: obj.clone(),
                    new_state,
                })
            })
        })
        .collect();
    if commands.is_empty() {
        return false;
    }
    state.push_command(DesignerCommand::CompositeCommand(CompositeCommand {
        commands,
        name: name.to_string(),
    }));
    true
}

/// Replaces `*field` with `value`; returns whether it changed.
pub fn set_f64(field: &mut f64, value: f64) -> bool {
    let changed = (*field - value).abs() > f64::EPSILON;
    *field = value;
    changed
}

/// Replaces `*field` with `value`; returns whether it changed.
pub fn set_value<T: PartialEq>(field: &mut T, value: T) -> bool {
    let changed = *field != value;
    *field = value;
    changed
}
