//! The Layers pane: the design's shapes in drawing order, with selection, visibility, lock,
//! rename, z-order and grouping.
//!
//! Ported from the GTK `designer_layers.rs`. The pane shares the Designer's state
//! ([`DesignerPane`]): clicking a row selects the shape on the canvas, and the canvas's
//! selection shows in the rows. Every change to the design goes through the model's undo
//! commands, one step per action: grouping, ungrouping, deleting, moving up and down, the
//! position field, and renaming. Each marks the design modified.
//!
//! GTK has no visibility or lock; they are added here. They are view state of the Designer
//! pane ([`ShapeFlags`](super::canvas::ShapeFlags)): not undoable, not saved, and a hidden or
//! locked shape cannot be selected (hiding or locking deselects it).
//!
//! Rows run bottom to top in drawing order (position 1 is drawn first). A position field
//! takes a number and Enter, as in GTK; a move that would split a group is refused with a
//! notice (GTK's dialog). Keys: Delete removes the selection (not during a canvas drag) while the pointer is over the
//! pane and no text field has focus.
//!
//! Dependencies: `egui`, `gcodekit5-designer` (model and commands), `gcodekit5-app`
//! (translations).

use std::cell::RefCell;
use std::rc::Rc;

use gcodekit5_app::i18n::Translator;
use gcodekit5_designer::DesignerState;
use gcodekit5_designer::commands::{ChangeProperty, DesignerCommand, Reorder};
use gcodekit5_designer::model::{ParametricPathSource, Shape};
use gcodekit5_designer::shapes::OperationType;

use super::DesignerPane;
use crate::workspace::{Pane, PaneContext, PaneId};

/// A z-order button: its msgid, whether it is enabled, and the model call it makes.
type Move = (&'static str, bool, fn(&mut DesignerState) -> bool);

/// One row of the list, read from the design.
struct Row {
    id: u64,
    /// 1-based position in the drawing order.
    position: usize,
    name: String,
    group: Option<u64>,
    shape_msgid: &'static str,
    operation: OperationType,
    selected: bool,
}

/// A text field being edited: which row and field, and the text so far.
#[derive(Clone, PartialEq, Eq)]
struct Edit {
    id: u64,
    field: Field,
    text: String,
}

/// A row's editable fields.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Field {
    Name,
    Position,
}

/// The msgids the pane shows, for the translation check.
pub fn msgids() -> Vec<&'static str> {
    vec![
        n_!("Objects"),
        n_!("Group"),
        n_!("Ungroup"),
        n_!("Move to start"),
        n_!("Move up"),
        n_!("Move down"),
        n_!("Move to end"),
        n_!("Rect"),
        n_!("Circ"),
        n_!("Line"),
        n_!("Ellip"),
        n_!("Tpulley"),
        n_!("Path"),
        n_!("Text"),
        n_!("Tri"),
        n_!("Poly"),
        n_!("Gear"),
        n_!("Spro"),
        n_!("Image"),
        n_!("Pocket operation"),
        n_!("Profile operation"),
        n_!("Click to select the whole group"),
        n_!("It cannot be moved"),
        n_!("The move would split a group. The members of a group must remain contiguous."),
        n_!("Close"),
        n_!("Hide"),
        n_!("Show"),
        n_!("Lock"),
        n_!("Unlock"),
        n_!("Position"),
        n_!("Pocket"),
        n_!("Profile"),
        n_!("Name"),
    ]
}

/// The short type name msgid of `shape`, as GTK's list shows it.
fn type_msgid(shape: &Shape) -> &'static str {
    match shape {
        Shape::Rectangle(_) => n_!("Rect"),
        Shape::Circle(_) => n_!("Circ"),
        Shape::Line(_) => n_!("Line"),
        Shape::Ellipse(_) => n_!("Ellip"),
        Shape::Path(p) => match p.parametric_source {
            Some(ParametricPathSource::TimingPulley { .. }) => n_!("Tpulley"),
            _ => n_!("Path"),
        },
        Shape::Text(_) => n_!("Text"),
        Shape::Triangle(_) => n_!("Tri"),
        Shape::Polygon(_) => n_!("Poly"),
        Shape::Gear(_) => n_!("Gear"),
        Shape::Sprocket(_) => n_!("Spro"),
        Shape::RasterImage(_) => n_!("Image"),
    }
}

/// The rows in drawing order, bottom first.
fn rows(state: &DesignerState) -> Vec<Row> {
    let store = &state.canvas.shape_store;
    store
        .draw_order()
        .iter()
        .enumerate()
        .filter_map(|(i, id)| {
            store.get(*id).map(|obj| Row {
                id: obj.id,
                position: i + 1,
                name: obj.name.clone(),
                group: obj.group_id,
                shape_msgid: type_msgid(&obj.shape),
                operation: obj.operation_type,
                selected: obj.selected,
            })
        })
        .collect()
}

/// Selects exactly `ids` (the first is the primary selection).
fn select_only(state: &mut DesignerState, ids: &[u64]) {
    let canvas = &mut state.canvas;
    canvas.deselect_all();
    for id in ids {
        if let Some(obj) = canvas.get_shape_mut(*id) {
            obj.selected = true;
        }
    }
    canvas
        .selection_manager
        .set_selected_id(ids.first().copied());
}

/// The Layers pane.
pub struct LayersPane {
    designer: Rc<RefCell<DesignerPane>>,
    edit: Option<Edit>,
    /// The row a Shift-click range starts from.
    anchor: Option<u64>,
    /// Whether the "cannot be moved" notice is shown.
    notice: bool,
}

impl std::fmt::Debug for LayersPane {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("LayersPane")
            .field("anchor", &self.anchor)
            .field("notice", &self.notice)
            .finish_non_exhaustive()
    }
}

impl LayersPane {
    /// A Layers pane for the design of `designer`.
    ///
    /// # Arguments
    /// * `designer` - The Designer pane the app also draws; its design is the one listed.
    pub fn new(designer: Rc<RefCell<DesignerPane>>) -> Self {
        Self {
            designer,
            edit: None,
            anchor: None,
            notice: false,
        }
    }

    /// Group, Ungroup and the four z-order buttons.
    fn toolbar(&mut self, ui: &mut egui::Ui, d: &mut DesignerPane, rows: &[Row], tr: &Translator) {
        let selected: Vec<usize> = rows
            .iter()
            .filter(|r| r.selected)
            .map(|r| r.position)
            .collect();
        let grouped = rows.iter().any(|r| r.selected && r.group.is_some());
        let (first, last) = (selected.contains(&1), selected.contains(&rows.len()));
        // GTK: with a single selected row at an end, the moves toward that end are disabled.
        let single = selected.len() == 1;
        let start_ok = !selected.is_empty() && !(single && first);
        let end_ok = !selected.is_empty() && !(single && last);
        ui.horizontal_wrapped(|ui| {
            if ui
                .add_enabled(selected.len() >= 2, egui::Button::new(tr.tr("Group")))
                .clicked()
            {
                d.state_mut().group_selected();
            }
            if ui
                .add_enabled(grouped, egui::Button::new(tr.tr("Ungroup")))
                .clicked()
            {
                d.state_mut().ungroup_selected();
            }
            let moves: [Move; 4] = [
                (
                    n_!("Move to start"),
                    start_ok,
                    DesignerState::move_selected_to_top,
                ),
                (n_!("Move up"), start_ok, DesignerState::move_selected_up),
                (n_!("Move down"), end_ok, DesignerState::move_selected_down),
                (
                    n_!("Move to end"),
                    end_ok,
                    DesignerState::move_selected_to_bottom,
                ),
            ];
            for (msgid, enabled, apply) in moves {
                if ui
                    .add_enabled(enabled, egui::Button::new(tr.tr(msgid)))
                    .clicked()
                    && !apply(d.state_mut())
                {
                    self.notice = true;
                }
            }
        });
    }

    /// The refusal notice (GTK's dialog) with a Close button.
    fn notice(&mut self, ui: &mut egui::Ui, tr: &Translator) {
        if !self.notice {
            return;
        }
        egui::Frame::group(ui.style()).show(ui, |ui| {
            ui.strong(tr.tr("It cannot be moved"));
            ui.label(tr.tr(
                "The move would split a group. The members of a group must remain contiguous.",
            ));
            if ui.button(tr.tr("Close")).clicked() {
                self.notice = false;
            }
        });
    }

    /// One row; applies what the user did in it.
    fn row(
        &mut self,
        ui: &mut egui::Ui,
        d: &mut DesignerPane,
        rows: &[Row],
        row: &Row,
        tr: &Translator,
    ) {
        let hidden = d.flags().is_hidden(row.id);
        let locked = d.flags().is_locked(row.id);
        ui.horizontal(|ui| {
            if ui
                .selectable_label(hidden, tr.tr(if hidden { "Show" } else { "Hide" }))
                .clicked()
            {
                d.set_hidden(row.id, !hidden);
            }
            if ui
                .selectable_label(locked, tr.tr(if locked { "Unlock" } else { "Lock" }))
                .clicked()
            {
                d.set_locked(row.id, !locked);
            }
            let operation = match row.operation {
                OperationType::Pocket => (n_!("Pocket operation"), n_!("Pocket")),
                OperationType::Profile => (n_!("Profile operation"), n_!("Profile")),
            };
            ui.weak(tr.tr(operation.1))
                .on_hover_text(tr.tr(operation.0));
            // A locked shape is read-only here: no rename, no move in the order.
            ui.add_enabled_ui(!locked, |ui| {
                self.position_field(ui, d, row, rows.len(), tr)
            });
            let label = format!("{} #{}", tr.tr(row.shape_msgid), row.id);
            let pickable = d.flags().is_pickable(row.id);
            if ui
                .add_enabled(pickable, egui::Button::selectable(row.selected, label))
                .clicked()
            {
                let modifiers = ui.input(|i| i.modifiers);
                self.click_row(d, rows, row, modifiers);
            }
            ui.add_enabled_ui(!locked, |ui| self.name_field(ui, d, row, tr));
            self.group_badge(ui, d, rows, row, tr);
        });
    }

    /// A click on a row: plain selects it alone, Ctrl/Cmd toggles, Shift adds the range from
    /// the anchor row.
    fn click_row(
        &mut self,
        d: &mut DesignerPane,
        rows: &[Row],
        row: &Row,
        modifiers: egui::Modifiers,
    ) {
        let mut ids: Vec<u64> = rows.iter().filter(|r| r.selected).map(|r| r.id).collect();
        if modifiers.shift {
            let anchor = self
                .anchor
                .and_then(|a| rows.iter().position(|r| r.id == a));
            let here = rows.iter().position(|r| r.id == row.id).unwrap_or(0);
            let (a, b) = (anchor.unwrap_or(here), here);
            for r in &rows[a.min(b)..=a.max(b)] {
                if d.flags().is_pickable(r.id) && !ids.contains(&r.id) {
                    ids.push(r.id);
                }
            }
        } else if modifiers.command {
            if let Some(i) = ids.iter().position(|id| *id == row.id) {
                ids.remove(i);
            } else {
                ids.push(row.id);
            }
            self.anchor = Some(row.id);
        } else {
            ids = vec![row.id];
            self.anchor = Some(row.id);
        }
        select_only(d.state_mut(), &ids);
    }

    /// The editable position (1-based draw order); Enter moves the shape there.
    fn position_field(
        &mut self,
        ui: &mut egui::Ui,
        d: &mut DesignerPane,
        row: &Row,
        count: usize,
        tr: &Translator,
    ) {
        let shown = row.position.to_string();
        let (response, text) = self.text_field(
            ui,
            row.id,
            Field::Position,
            &shown,
            &tr.tr("Position"),
            28.0,
        );
        response.on_hover_text(tr.tr("Position"));
        let Some(text) = text else {
            return;
        };
        // Only Enter commits; anything else restores the shown value.
        if !text.committed {
            return;
        }
        let state = d.state_mut();
        let target = text
            .value
            .trim()
            .parse::<usize>()
            .ok()
            .filter(|n| (1..=count).contains(n));
        let Some(target) = target else {
            return;
        };
        if target == row.position {
            return;
        }
        if state
            .canvas
            .shape_store
            .would_split_group(row.id, target - 1)
        {
            self.notice = true;
            return;
        }
        // The model's Reorder command applies `after` itself, so restore `before` first.
        let before = state.canvas.shape_store.draw_order().clone();
        state
            .canvas
            .shape_store
            .move_to_position(row.id, target - 1);
        let after = state.canvas.shape_store.draw_order().clone();
        state.canvas.shape_store.apply_order(before.clone());
        state.push_command(DesignerCommand::Reorder(Reorder {
            before,
            after,
            name: "Reorder".to_string(),
        }));
    }

    /// The editable name; losing focus or Enter renames the shape (one undo step).
    fn name_field(&mut self, ui: &mut egui::Ui, d: &mut DesignerPane, row: &Row, tr: &Translator) {
        let (_, text) = self.text_field(ui, row.id, Field::Name, &row.name, &tr.tr("Name"), 110.0);
        let Some(text) = text else {
            return;
        };
        if text.value == row.name {
            return;
        }
        let state = d.state_mut();
        let Some(old_state) = state.canvas.get_shape(row.id).cloned() else {
            return;
        };
        let mut new_state = old_state.clone();
        new_state.name = text.value;
        state.push_command(DesignerCommand::ChangeProperty(ChangeProperty {
            id: row.id,
            old_state,
            new_state,
        }));
    }

    /// A single-line field showing `shown`, or the text being typed while it has focus.
    ///
    /// `name` is the field's accessible name. Returns the response, and the finished text when editing ended this frame (Enter, or
    /// focus lost).
    fn text_field(
        &mut self,
        ui: &mut egui::Ui,
        id: u64,
        field: Field,
        shown: &str,
        name: &str,
        width: f32,
    ) -> (egui::Response, Option<Finished>) {
        let editing = self
            .edit
            .as_ref()
            .filter(|e| e.id == id && e.field == field)
            .map(|e| e.text.clone());
        let mut buffer = editing.clone().unwrap_or_else(|| shown.to_string());
        let response = ui.add(
            egui::TextEdit::singleline(&mut buffer)
                .id_salt((id, field as u8))
                .hint_text(name)
                .desired_width(width),
        );
        if response.changed() || (response.has_focus() && editing.is_none()) {
            self.edit = Some(Edit {
                id,
                field,
                text: buffer.clone(),
            });
        }
        let enter = response.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter));
        let finished = (response.lost_focus() && editing.is_some()).then_some(Finished {
            value: buffer,
            committed: enter,
        });
        if finished.is_some() {
            self.edit = None;
        }
        (response, finished)
    }

    /// "G:n" for a grouped shape (click selects the whole group), "-" otherwise.
    fn group_badge(
        &mut self,
        ui: &mut egui::Ui,
        d: &mut DesignerPane,
        rows: &[Row],
        row: &Row,
        tr: &Translator,
    ) {
        let Some(gid) = row.group else {
            ui.weak("-");
            return;
        };
        let response = ui
            .add(egui::Label::new(format!("G:{gid}")).sense(egui::Sense::click()))
            .on_hover_text(tr.tr("Click to select the whole group"));
        if response.clicked() {
            let additive = ui.input(|i| i.modifiers.command);
            let mut ids: Vec<u64> = if additive {
                rows.iter().filter(|r| r.selected).map(|r| r.id).collect()
            } else {
                Vec::new()
            };
            for r in rows.iter().filter(|r| r.group == Some(gid)) {
                if d.flags().is_pickable(r.id) && !ids.contains(&r.id) {
                    ids.push(r.id);
                }
            }
            select_only(d.state_mut(), &ids);
        }
    }
}

/// A text field's result when editing ended.
struct Finished {
    value: String,
    /// Whether it ended with Enter (not by losing focus).
    committed: bool,
}

impl Pane for LayersPane {
    fn id(&self) -> PaneId {
        PaneId::DesignerLayers
    }

    fn title(&self, tr: &Translator) -> String {
        tr.tr(PaneId::DesignerLayers.title_msgid()).into_owned()
    }

    fn ui(&mut self, ui: &mut egui::Ui, ctx: &mut PaneContext) {
        let tr = ctx.tr;
        let designer = Rc::clone(&self.designer);
        let Ok(mut d) = designer.try_borrow_mut() else {
            tracing::warn!("designer busy; layers not drawn this frame");
            return;
        };
        let list = rows(d.state());
        ui.strong(tr.tr("Objects"));
        self.toolbar(ui, &mut d, &list, tr);
        self.notice(ui, tr);
        ui.separator();
        // Delete with the pointer over the pane, unless a text field (a rename) has the keys.
        let over = ui.rect_contains_pointer(ui.max_rect());
        if over
            && !d.dragging()
            && !ui.ctx().egui_wants_keyboard_input()
            && ui.input(|i| i.key_pressed(egui::Key::Delete) || i.key_pressed(egui::Key::Backspace))
        {
            d.state_mut().delete_selected();
        }
        egui::ScrollArea::vertical()
            .auto_shrink([false, false])
            .show(ui, |ui| {
                for row in &list {
                    self.row(ui, &mut d, &list, row, tr);
                }
            });
    }
}
