//! The Properties pane: every editable property of the selected shape(s).
//!
//! The pane shares the Designer's state ([`DesignerPane`]). Sections, top to bottom, shown
//! for the selection ([`update::Summary`]):
//!
//! - the "Rotated object" notice, Position, Size, Rotation ([`handlers::dimensions`],
//!   [`handlers::geometry`]);
//! - per shape: Corner, Text, Polygon, Path, Gear, Sprocket, Timing Pulley;
//! - Geometry Operations (offset, chamfer; not for raster images or text only);
//! - CAM Properties (CNC mode), Laser Engraving (a raster image), Laser Config (laser
//!   mode).
//!
//! Text fields apply on Enter or when the focus leaves them, so each edit is one undo step
//! ([`builders`]). The edit applies to the shapes selected when the typing began, even if
//! a click elsewhere (on the canvas) changed the selection meanwhile; if those shapes were
//! deleted or locked, nothing is applied and a notice says so. Invalid or out-of-range
//! input shows an inline error and applies nothing. Checkboxes and drop-downs apply when
//! changed. Each step marks the design
//! modified and the canvas repaints from the design. Several selected shapes: the common
//! box, a relative rotation, and every change applied to all of them. Hidden and locked
//! shapes are never selected ([`ShapeFlags`](super::canvas::ShapeFlags)), so they are never
//! edited here. While a field has the keyboard, the canvas and layers keys (Delete) do not
//! act.
//!
//! Dependencies: `egui`, `gcodekit5-designer` (model and commands), `gcodekit5-core`
//! (units), `gcodekit5-app` (translations).

pub mod builders;
pub mod handlers;
pub mod update;

use std::cell::RefCell;
use std::rc::Rc;

use gcodekit5_app::i18n::Translator;
use gcodekit5_designer::model::Shape;

use super::DesignerPane;
use crate::workspace::{Pane, PaneContext, PaneId};
use builders::Edit;
use handlers::{Ctx, cam, dimensions, effects, gear_sprocket, geometry, image, laser_override};
use update::Summary;

/// The msgid shown when nothing is selected.
const EMPTY: &str = n_!("Select a shape to edit its properties");
/// The msgid shown when a finished edit's shapes were deleted or locked.
const NOTICE: &str = n_!("The edited shapes were deleted or locked; the change was not applied.");

/// The msgids the pane shows, for the translation check.
pub fn msgids() -> Vec<&'static str> {
    let mut ids = vec![EMPTY, NOTICE];
    ids.extend(builders::msgids());
    ids.extend(dimensions::msgids());
    ids.extend(geometry::msgids());
    ids.extend(handlers::text::msgids());
    ids.extend(gear_sprocket::msgids());
    ids.extend(effects::msgids());
    ids.extend(cam::msgids());
    ids.extend(image::msgids());
    ids.extend(laser_override::msgids());
    ids
}

/// The Properties pane.
pub struct PropertiesPane {
    designer: Rc<RefCell<DesignerPane>>,
    /// The fields being edited.
    edits: Vec<Edit>,
    /// The inline errors: field key and msgid (cleared when the selection changes).
    errors: Vec<(&'static str, &'static str)>,
    /// Whether a finished edit found its shapes deleted or locked.
    notice: bool,
    /// The selection the panel showed last frame.
    shown_selection: Vec<u64>,
}

impl std::fmt::Debug for PropertiesPane {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("PropertiesPane")
            .field("edits", &self.edits)
            .field("errors", &self.errors)
            .field("notice", &self.notice)
            .finish_non_exhaustive()
    }
}

impl PropertiesPane {
    /// A Properties pane for the design of `designer`.
    ///
    /// # Arguments
    /// * `designer` - The Designer pane the app also draws; its selection is edited.
    pub fn new(designer: Rc<RefCell<DesignerPane>>) -> Self {
        Self {
            designer,
            edits: Vec::new(),
            errors: Vec::new(),
            notice: false,
            shown_selection: Vec::new(),
        }
    }

    /// Commits every edit whose field lost the keyboard (Enter, or the focus moved away)
    /// (or whose field is no longer drawn) to the shapes it was typed for, even if the
    /// selection changed since: a click on the
    /// canvas both ends the edit and changes the selection, and the value must still land
    /// where the user typed it. Shapes deleted or locked since are skipped; when none are
    /// left nothing is applied and the panel shows a notice. One undo step per edit.
    fn commit_finished(&mut self, ctx: &egui::Context, d: &mut DesignerPane, tr: &Translator) {
        let focused = ctx.memory(|m| m.focused());
        // Finished: its field lost the keyboard, or was not drawn last frame.
        let mut finished = Vec::new();
        for mut edit in std::mem::take(&mut self.edits) {
            if edit.take_seen() && Some(edit.widget()) == focused {
                self.edits.push(edit);
            } else {
                finished.push(edit);
            }
        }
        for edit in finished.into_iter().filter(Edit::changed) {
            let targets: Vec<u64> = edit
                .selection()
                .iter()
                .copied()
                .filter(|id| d.state().canvas.get_shape(*id).is_some())
                .filter(|id| d.flags().is_pickable(*id))
                .collect();
            if targets.is_empty() {
                tracing::debug!(key = edit.key(), "property edit not applied: shapes gone");
                self.notice = true;
                continue;
            }
            let now = selected_ids(d);
            select_only(d, &targets);
            let result = match Summary::of(d.state()) {
                Some(summary) => {
                    let units = d.units();
                    let mut cx = Ctx {
                        designer: d,
                        edits: &mut Vec::new(),
                        errors: &[],
                        summary: &summary,
                        units,
                        tr,
                    };
                    handlers::apply(&mut cx, edit.key(), edit.text())
                }
                None => Ok(()),
            };
            select_only(d, &now);
            self.errors.retain(|(k, _)| *k != edit.key());
            match result {
                Ok(()) => self.notice = false,
                Err(msgid) => self.errors.push((edit.key(), msgid)),
            }
        }
    }

    /// The sections for `summary`, in display order.
    fn sections(
        &mut self,
        ui: &mut egui::Ui,
        d: &mut DesignerPane,
        summary: &Summary,
        tr: &Translator,
    ) {
        let units = d.units();
        let mut cx = Ctx {
            designer: d,
            edits: &mut self.edits,
            errors: &self.errors,
            summary,
            units,
            tr,
        };
        if summary.rotated() {
            let warning = crate::theme::palette(ui.ctx().theme()).warning;
            ui.colored_label(warning, tr.tr(geometry::ROTATED_NOTICE));
        }
        let single = summary.single();
        let pulley = summary.timing_pulley().is_some();
        let path = matches!(single, Some(Shape::Path(_))) && !pulley;
        dimensions::position(ui, &mut cx);
        if !matches!(single, Some(Shape::Gear(_) | Shape::Sprocket(_))) && !pulley {
            dimensions::size(ui, &mut cx);
        }
        let unrotatable = matches!(
            single,
            Some(Shape::Circle(_) | Shape::Gear(_) | Shape::Sprocket(_) | Shape::RasterImage(_))
        );
        if !unrotatable && !pulley {
            geometry::rotation(ui, &mut cx);
        }
        if matches!(single, Some(Shape::Rectangle(_))) || path {
            geometry::corner(ui, &mut cx);
        }
        handlers::text::show(ui, &mut cx);
        geometry::polygon(ui, &mut cx);
        if path {
            geometry::path(ui, &mut cx);
        }
        gear_sprocket::gear(ui, &mut cx);
        gear_sprocket::sprocket(ui, &mut cx);
        gear_sprocket::timing_pulley(ui, &mut cx);
        if !summary.raster && summary.any_not_text {
            effects::show(ui, &mut cx);
        }
        if !summary.laser_mode && !summary.raster {
            cam::show(ui, &mut cx);
        }
        image::show(ui, &mut cx);
        if summary.laser_mode && !summary.raster {
            laser_override::show(ui, &mut cx);
        }
    }
}

impl Pane for PropertiesPane {
    fn id(&self) -> PaneId {
        PaneId::DesignerProperties
    }

    fn title(&self, tr: &Translator) -> String {
        tr.tr(PaneId::DesignerProperties.title_msgid()).into_owned()
    }

    fn ui(&mut self, ui: &mut egui::Ui, ctx: &mut PaneContext) {
        let tr = ctx.tr;
        let designer = Rc::clone(&self.designer);
        let Ok(mut d) = designer.try_borrow_mut() else {
            tracing::warn!("designer busy; properties not drawn this frame");
            return;
        };
        let now = selected_ids(&d);
        if now != self.shown_selection {
            self.errors.clear();
            self.notice = false;
            self.shown_selection = now;
        }
        self.commit_finished(ui.ctx(), &mut d, tr);
        if self.notice {
            let warning = crate::theme::palette(ui.ctx().theme()).warning;
            ui.add(egui::Label::new(egui::RichText::new(tr.tr(NOTICE)).color(warning)).wrap());
        }
        // The preview lasts only while an offset or chamfer is being typed.
        let previewing = self.edits.iter().any(|e| effects::KEYS.contains(&e.key()));
        if !previewing && !d.effect_preview().is_empty() {
            d.set_effect_preview(Vec::new());
        }
        match Summary::of(d.state()) {
            Some(summary) => {
                egui::ScrollArea::vertical()
                    .auto_shrink([false, false])
                    .show(ui, |ui| self.sections(ui, &mut d, &summary, tr));
            }
            None => {
                ui.add(egui::Label::new(egui::RichText::new(tr.tr(EMPTY)).weak()).wrap());
            }
        }
        if self.edits.iter().any(|e| !e.seen()) {
            // An edit whose field was not drawn this frame finishes on the next one.
            ui.ctx().request_repaint();
        }
    }

    /// The pane was not drawn this frame (hidden, closed, or the window lost focus): the
    /// canvas must not keep showing an offset or chamfer preview no one is typing.
    fn cancel_hold(&mut self) {
        match self.designer.try_borrow_mut() {
            Ok(mut d) if !d.effect_preview().is_empty() => d.set_effect_preview(Vec::new()),
            Ok(_) => {}
            Err(error) => tracing::warn!(%error, "designer busy; effect preview kept"),
        }
    }
}

/// The selected shapes, in drawing order.
fn selected_ids(d: &DesignerPane) -> Vec<u64> {
    let canvas = &d.state().canvas;
    canvas
        .shape_store
        .draw_order_iter()
        .filter(|id| canvas.get_shape(*id).is_some_and(|o| o.selected))
        .collect()
}

/// Selects exactly `ids` (view state only, no undo step; the first is the primary).
fn select_only(d: &mut DesignerPane, ids: &[u64]) {
    let canvas = &mut d.state_mut().canvas;
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
