//! The Fast Shape Gallery: parametric shapes with preset parameters, placed with one click.
//!
//! Ported from the GTK `fast_shape_gallery.rs` and the canvas's pending fast shape. The
//! toolbox's "Shape Gallery…" button opens a modal with one card per template (name and
//! description). Choosing a card closes the gallery and arms the shape: the canvas then shows
//! it as a dashed ghost centred on the pointer, with the hint "Click to position shape". A
//! primary click places it there (snapped unless the Select tool is active, as GTK) as one
//! undo step; Escape drops it.
//!
//! Each card shows the template's SVG icon (GTK's resources, 48 points) over its name and
//! description.
//!
//! Dependencies: `egui`, `gcodekit5-designer` (shapes, parametric generators),
//! `gcodekit5-app` (translations).

use gcodekit5_app::i18n::Translator;
use gcodekit5_designer::DesignerState;
use gcodekit5_designer::model::{
    DesignGear, DesignPath, DesignRectangle, DesignSprocket, DesignerShape, ParametricPathSource,
    Point, Shape,
};
use gcodekit5_designer::parametric_shapes::{
    generate_l_bracket, generate_timing_pulley, generate_u_bracket,
};

/// One gallery card: the msgids of its name and description, and the shape it makes.
#[derive(Clone, Copy, Debug)]
pub struct ShapeTemplate {
    /// The msgid of the card's name.
    pub name: &'static str,
    /// The msgid of the card's description.
    pub description: &'static str,
    /// The app SVG (in `resources/icons/`) on the card.
    pub icon: &'static str,
    /// Makes the shape centred on a point (mm).
    pub make: fn(Point) -> Shape,
}

/// GTK's templates, in gallery order, with GTK's parameters.
pub const TEMPLATES: [ShapeTemplate; 6] = [
    ShapeTemplate {
        name: n_!("Spur Gear"),
        description: n_!("Standard involute spur gear"),
        icon: "gear.svg",
        make: |c| Shape::Gear(DesignGear::new(c, 2.0, 20)),
    },
    ShapeTemplate {
        name: n_!("Sprocket"),
        description: n_!("Chain sprocket for #40 chain"),
        icon: "sprocket1.svg",
        make: |c| Shape::Sprocket(DesignSprocket::new(c, 12.7, 15)),
    },
    ShapeTemplate {
        name: n_!("Timing Pulley"),
        description: n_!("XL timing belt pulley"),
        icon: "timingpulley.svg",
        make: timing_pulley,
    },
    ShapeTemplate {
        name: n_!("Slot"),
        description: n_!("Rectangular slot/cutout"),
        icon: "slot.svg",
        make: slot,
    },
    ShapeTemplate {
        name: n_!("L-Bracket"),
        description: n_!("L-shaped bracket with holes"),
        icon: "Lsupport.svg",
        make: |c| {
            Shape::Path(DesignPath::from_lyon_path(&generate_l_bracket(
                c, 80.0, 60.0, 5.0, 2.5, 15.0,
            )))
        },
    },
    ShapeTemplate {
        name: n_!("U-Bracket"),
        description: n_!("U-shaped channel bracket"),
        icon: "Usupport.svg",
        make: |c| {
            Shape::Path(DesignPath::from_lyon_path(&generate_u_bracket(
                c, 100.0, 40.0, 5.0, 2.5, 15.0,
            )))
        },
    },
];

/// An XL pulley: 20 teeth of 5.08 mm pitch, 9.4 mm belt, 5 mm hole, kept parametric.
fn timing_pulley(center: Point) -> Shape {
    let (pitch, teeth, belt_width, hole_radius) = (5.08, 20, 9.4, 5.0);
    let mut path = DesignPath::from_lyon_path(&generate_timing_pulley(
        center,
        pitch,
        teeth,
        belt_width,
        hole_radius,
    ));
    path.parametric_source = Some(ParametricPathSource::TimingPulley {
        pitch,
        teeth,
        belt_width,
        hole_radius,
    });
    Shape::Path(path)
}

/// A 50 x 20 mm slot (a rectangle in slot mode with a 5 mm radius).
fn slot(center: Point) -> Shape {
    let (width, height) = (50.0, 20.0);
    let mut rect = DesignRectangle::new(
        center.x - width / 2.0,
        center.y - height / 2.0,
        width,
        height,
    );
    rect.corner_radius = 5.0;
    rect.is_slot = true;
    Shape::Rectangle(rect)
}

/// The msgids the gallery shows, for the translation check.
pub fn msgids() -> Vec<&'static str> {
    let mut ids = vec![
        n_!("Fast Shapes"),
        n_!("Shape Gallery…"),
        n_!("Fast Shape Gallery"),
        n_!("Click on a shape to quickly insert it with default parameters."),
        n_!("Click to position shape"),
        n_!("Close"),
    ];
    ids.extend(TEMPLATES.iter().flat_map(|t| [t.name, t.description]));
    ids
}

/// `shape` moved so its bounding-box centre is at `at`.
fn centred(mut shape: Shape, at: (f64, f64)) -> Shape {
    let (x1, y1, x2, y2) = shape.bounds();
    shape.translate(at.0 - (x1 + x2) * 0.5, at.1 - (y1 + y2) * 0.5);
    shape
}

/// The gallery dialog and the shape it armed.
#[derive(Clone, Debug, Default)]
pub struct ShapeGallery {
    open: bool,
    pending: Option<Shape>,
}

impl ShapeGallery {
    /// Opens the gallery.
    pub fn open(&mut self) {
        self.open = true;
    }

    /// Whether the gallery is open.
    pub fn is_open(&self) -> bool {
        self.open
    }

    /// The shape waiting to be placed, at the origin.
    pub fn pending(&self) -> Option<&Shape> {
        self.pending.as_ref()
    }

    /// Arms template `index` (of [`TEMPLATES`]) and closes the gallery.
    ///
    /// # Arguments
    /// * `index` - The template; an index out of range arms nothing.
    pub fn choose(&mut self, index: usize) {
        if let Some(template) = TEMPLATES.get(index) {
            self.pending = Some((template.make)(Point::new(0.0, 0.0)));
        }
        self.open = false;
    }

    /// Drops the armed shape (Escape).
    pub fn cancel(&mut self) {
        self.pending = None;
    }

    /// The armed shape centred on `at`, as the canvas shows it under the pointer.
    ///
    /// # Arguments
    /// * `at` - The world point, mm.
    pub fn ghost(&self, at: (f64, f64)) -> Option<Shape> {
        self.pending.clone().map(|shape| centred(shape, at))
    }

    /// Places the armed shape centred on `at` as one undo step.
    ///
    /// # Arguments
    /// * `state` - The design.
    /// * `at` - The world point, mm.
    ///
    /// # Returns
    /// The new shape's id, or `None` when nothing was armed.
    pub fn place(&mut self, state: &mut DesignerState, at: (f64, f64)) -> Option<u64> {
        let shape = self.pending.take()?;
        Some(state.add_shape_with_undo(centred(shape, at)))
    }

    /// Shows the gallery while it is open.
    ///
    /// # Arguments
    /// * `ctx` - The egui context (the gallery is modal).
    /// * `tr` - Translator for the current UI language.
    pub fn show(&mut self, ctx: &egui::Context, tr: &Translator) {
        if !self.open {
            return;
        }
        let mut chosen = None;
        let mut close = false;
        let response = egui::Modal::new(egui::Id::unique("gcodekit5-designer-shape-gallery")).show(
            ctx,
            |ui| {
                ui.heading(tr.tr("Fast Shape Gallery"));
                ui.label(
                    egui::RichText::new(
                        tr.tr("Click on a shape to quickly insert it with default parameters."),
                    )
                    .weak(),
                );
                ui.add_space(8.0);
                egui::Grid::new("gcodekit5-shape-gallery-grid")
                    .num_columns(3)
                    .spacing([12.0, 12.0])
                    .show(ui, |ui| {
                        for (index, template) in TEMPLATES.iter().enumerate() {
                            if card(ui, template, tr) {
                                chosen = Some(index);
                            }
                            if index % 3 == 2 {
                                ui.end_row();
                            }
                        }
                    });
                ui.add_space(8.0);
                close = ui.button(tr.tr("Close")).clicked();
            },
        );
        if let Some(index) = chosen {
            self.choose(index);
        } else if close || response.should_close() {
            self.open = false;
        }
    }
}

/// Edge length of a card's icon in points (GTK's 48 px).
const CARD_ICON: f32 = 48.0;

/// One card, as GTK's: the icon over the name and the description in a framed group; the
/// whole card is clickable. The name is the card's accessible label.
///
/// # Returns
/// Whether the card was clicked.
fn card(ui: &mut egui::Ui, template: &ShapeTemplate, tr: &Translator) -> bool {
    let shown = crate::theme::group_frame(ui).show(ui, |ui| {
        ui.set_width(150.0);
        ui.vertical_centered(|ui| {
            if let Some(image) = crate::icons::app_icon(ui, template.icon, CARD_ICON) {
                ui.add(image);
            }
            ui.add(
                egui::Label::new(egui::RichText::new(tr.tr(template.name)).strong())
                    .selectable(false),
            );
            ui.add(
                egui::Label::new(egui::RichText::new(tr.tr(template.description)).small())
                    .selectable(false)
                    .wrap(),
            );
        });
    });
    let id = ui
        .scope_id()
        .with(("gcodekit5-gallery-card", template.name));
    ui.interact(shown.response.rect, id, egui::Sense::click())
        .on_hover_cursor(egui::CursorIcon::PointingHand)
        .clicked()
}
