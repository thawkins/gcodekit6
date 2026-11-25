//! The Properties pane: every property of the selected shape(s) that the GTK
//! `designer_properties` panel edits.
//!
//! The pane shares the Designer's state ([`DesignerPane`]). Sections, top to bottom, shown
//! as GTK shows them for the selection ([`update::Summary`]):
//!
//! - the "Rotated object" notice, Position, Size, Rotation ([`handlers::dimensions`],
//!   [`handlers::geometry`]);
//! - per shape: Corner, Text, Polygon, Path, Gear, Sprocket, Timing Pulley;
//! - Geometry Operations (offset, chamfer; not for raster images or text only);
//! - CAM Properties (CNC mode), Laser Engraving (a raster image), Laser Config (laser
//!   mode).
//!
//! Text fields apply on Enter or when the focus leaves them, so each edit is one undo step
//! ([`builders`]); checkboxes and drop-downs apply when changed. Each step marks the design
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

/// The msgids the pane shows, for the translation check.
pub fn msgids() -> Vec<&'static str> {
    let mut ids = vec![EMPTY];
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
}

impl std::fmt::Debug for PropertiesPane {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("PropertiesPane")
            .field("edits", &self.edits)
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
        }
    }

    /// The sections for `summary`, in GTK's order.
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
            summary,
            units,
            tr,
        };
        if summary.rotated() {
            let warning = crate::theme::palette(ui.ctx().theme()).warning;
            ui.colored_label(warning, tr.tr(geometry::ROTATED_NOTICE));
        }
        let single = summary.single.as_ref();
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
        // The preview lasts only while an offset or chamfer is being typed.
        let previewing = self.edits.iter().any(|e| effects::KEYS.contains(&e.key()));
        if !previewing && !d.effect_preview().is_empty() {
            d.set_effect_preview(Vec::new());
        }
        let Some(summary) = Summary::of(d.state()) else {
            ui.add(egui::Label::new(egui::RichText::new(tr.tr(EMPTY)).weak()).wrap());
            return;
        };
        egui::ScrollArea::vertical()
            .auto_shrink([false, false])
            .show(ui, |ui| self.sections(ui, &mut d, &summary, tr));
    }
}
