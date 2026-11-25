//! The CAM Tools pane: GTK's tool picker (search, category filter, a list and the chosen
//! tool's details with Open) and the tool pages, each with a Back button.
//!
//! A G-code page's Generate shows the program on the page; Send to Editor replaces the
//! editor's program with it and Preview does that and brings the visualizer to the front
//! (GTK's Generate did the latter). The app takes the request
//! ([`CamToolsPane::take_editor_request`]) and runs the designer's editor handoff: refused
//! while a job runs, asking first about unsaved editor edits.
//!
//! What a page asks the app for (preset files, shapes for the designer, a folder) is in
//! [`requests`]. Load and Save read and write a page's input as a JSON preset in GTK's file
//! format.
//!
//! The tools of later slices (laser image, probe routines) are not listed until their pages
//! land.
//!
//! Dependencies: `egui`, `gcodekit5-camtools` (generators), `gcodekit5-app` (translations),
//! `gcodekit5-settings` (units).

pub mod common;
pub mod drill_press;
pub mod jigsaw;
pub mod requests;
pub mod speeds_feeds;
pub mod spoilboard_grid;
pub mod spoilboard_surfacing;
pub mod tabbed_box;

use std::cell::RefCell;
use std::rc::Rc;

use gcodekit5_app::device_status;
use gcodekit5_app::i18n::Translator;
use gcodekit5_core::units::MeasurementSystem;
use gcodekit5_settings::config::Config;

pub use common::{CamAction, CamError, FieldError};
use drill_press::DrillPressParams;
use gcodekit5_core::data::materials::{Material, init_standard_library as material_library};
use gcodekit5_core::data::tools::{Tool, init_standard_library as tool_library};
use gcodekit5_devicedb::model::DeviceProfile;
use jigsaw::JigsawParams;
pub use requests::{DesignerPath, PresetRequest};
use requests::{DesignerPaths, PendingPreset};
use speeds_feeds::SpeedsFeedsParams;
use spoilboard_grid::SpoilboardGridParams;
use spoilboard_surfacing::SpoilboardSurfacingParams;
use tabbed_box::TabbedBoxParams;

use crate::workspace::{Pane, PaneContext, PaneId};

/// One CAM tool of the picker.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum CamTool {
    /// Jigsaw Puzzle Generator.
    Jigsaw,
    /// Tabbed Box Maker.
    TabbedBox,
    /// Speeds and Feeds Calculator.
    SpeedsFeeds,
    /// Create Spoilboard Grid.
    SpoilboardGrid,
    /// Spoilboard Surfacing.
    SpoilboardSurfacing,
    /// Drill Press.
    DrillPress,
}

impl CamTool {
    /// Every tool, in GTK's list order.
    pub const ALL: [CamTool; 6] = [
        CamTool::Jigsaw,
        CamTool::TabbedBox,
        CamTool::SpeedsFeeds,
        CamTool::SpoilboardSurfacing,
        CamTool::SpoilboardGrid,
        CamTool::DrillPress,
    ];

    /// The msgid of the tool's title.
    pub fn title_msgid(self) -> &'static str {
        match self {
            CamTool::Jigsaw => n_!("Jigsaw Puzzle Generator"),
            CamTool::TabbedBox => n_!("Tabbed Box Maker"),
            CamTool::SpeedsFeeds => n_!("Speeds and Feeds Calculator"),
            CamTool::SpoilboardGrid => n_!("Create Spoilboard Grid"),
            CamTool::SpoilboardSurfacing => n_!("Spoilboard Surfacing"),
            CamTool::DrillPress => n_!("Drill Press"),
        }
    }

    /// The msgid of the tool's description.
    pub fn description_msgid(self) -> &'static str {
        match self {
            CamTool::Jigsaw => n_!("Create custom jigsaw puzzle patterns from images"),
            CamTool::TabbedBox => n_!("Generate G-code for laser/CNC cut boxes with finger joints"),
            CamTool::SpeedsFeeds => n_!("Calculate cutting speeds and feeds for your materials"),
            CamTool::SpoilboardGrid => n_!("Generate grid patterns for spoilboard alignment"),
            CamTool::SpoilboardSurfacing => {
                n_!("Generate surfacing toolpaths to flatten your spoilboard")
            }
            CamTool::DrillPress => {
                n_!("Emulate a drill press with peck drilling and helical cycles")
            }
        }
    }

    /// The picker category the tool is listed under.
    pub fn category(self) -> Category {
        match self {
            CamTool::Jigsaw | CamTool::TabbedBox => Category::Generators,
            CamTool::SpeedsFeeds => Category::Calculators,
            CamTool::SpoilboardGrid => Category::Maintenance,
            CamTool::SpoilboardSurfacing => Category::Maintenance,
            CamTool::DrillPress => Category::Generators,
        }
    }
}

/// The picker's category filter (GTK's ids).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Category {
    /// Every tool.
    All,
    /// Program generators.
    Generators,
    /// Engraving tools.
    Engraving,
    /// Calculators.
    Calculators,
    /// Machine maintenance.
    Maintenance,
}

impl Category {
    /// Every category, in menu order.
    pub const ALL: [Category; 5] = [
        Category::All,
        Category::Generators,
        Category::Engraving,
        Category::Calculators,
        Category::Maintenance,
    ];

    /// The msgid of the category's name.
    pub fn msgid(self) -> &'static str {
        match self {
            Category::All => n_!("All"),
            Category::Generators => n_!("Generators"),
            Category::Engraving => n_!("Engraving"),
            Category::Calculators => n_!("Calculators"),
            Category::Maintenance => n_!("Maintenance"),
        }
    }
}

/// How a page's program reaches the editor.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CamDelivery {
    /// Replace the editor's program.
    Editor,
    /// Replace the editor's program and bring the visualizer to the front.
    Preview,
}

/// The msgids the pane shows, for the translation check.
pub fn msgids() -> Vec<&'static str> {
    let mut ids = vec![
        n_!("Search tools..."),
        n_!("Category"),
        n_!("Select a tool"),
        n_!("Choose a tool from the list to see details."),
        n_!("Open"),
        n_!("Back"),
        n_!("Close"),
        n_!("Generated G-Code"),
        n_!("Load Error"),
        n_!("Save Error"),
    ];
    ids.extend(Category::ALL.map(Category::msgid));
    for tool in CamTool::ALL {
        ids.push(tool.title_msgid());
        ids.push(tool.description_msgid());
    }
    ids.extend(common::msgids());
    ids.extend(speeds_feeds::msgids());
    ids.extend(spoilboard_grid::msgids());
    ids.extend(spoilboard_surfacing::msgids());
    ids.extend(drill_press::msgids());
    ids.extend(jigsaw::msgids());
    ids.extend(tabbed_box::msgids());
    ids
}

/// The program Generate made, and the page input it was made from.
#[derive(Debug)]
struct Output {
    /// The page.
    tool: CamTool,
    /// The page's input ([`CamToolsPane::input_key`]).
    input: String,
    /// The program.
    gcode: String,
}

/// The CAM Tools pane.
#[derive(Debug)]
pub struct CamToolsPane {
    /// The display units of the pages' lengths.
    units: MeasurementSystem,
    /// The open tool page; `None` shows the picker.
    open: Option<CamTool>,
    /// The tool chosen in the picker's list.
    selected: Option<CamTool>,
    /// The picker's search text.
    search: String,
    /// The picker's category filter.
    category: Category,
    /// The last program Generate made, and its tool.
    output: Option<Output>,
    /// A message shown above the page (a generator's refusal, a refused handoff).
    notice: Option<String>,
    /// The Drill Press page.
    drill_press: DrillPressParams,
    /// The Spoilboard Surfacing page.
    spoilboard_surfacing: SpoilboardSurfacingParams,
    /// The Create Spoilboard Grid page.
    spoilboard_grid: SpoilboardGridParams,
    /// The Speeds and Feeds page.
    speeds_feeds: SpeedsFeedsParams,
    /// The Jigsaw Puzzle Generator page.
    jigsaw: JigsawParams,
    /// The Tabbed Box Maker page.
    tabbed_box: TabbedBoxParams,
    /// Send to Editor / Preview, waiting for the app.
    editor_request: Option<CamDelivery>,
    /// The program the request hands to the editor.
    pending: Option<String>,
    /// The shapes a page asked to add to the design, waiting for the app.
    designer_request: Option<DesignerPaths>,
    /// The Gerber page's Browse, waiting for the app.
    folder_request: bool,
    /// Load / Save, waiting for the app.
    preset_request: Option<PresetRequest>,
    /// The Load / Save whose file dialog is up.
    preset: Option<PendingPreset>,
}

impl Default for CamToolsPane {
    fn default() -> Self {
        Self::new()
    }
}

impl CamToolsPane {
    /// The pane with the picker showing and every page at GTK's defaults, in millimetres.
    pub fn new() -> Self {
        let units = MeasurementSystem::Metric;
        Self {
            units,
            open: None,
            selected: None,
            search: String::new(),
            category: Category::All,
            drill_press: DrillPressParams::new(units),
            spoilboard_surfacing: SpoilboardSurfacingParams::new(units),
            spoilboard_grid: SpoilboardGridParams::new(units),
            speeds_feeds: SpeedsFeedsParams::new(
                tool_library()
                    .get_all_tools()
                    .into_iter()
                    .cloned()
                    .collect(),
                material_library()
                    .get_all_materials()
                    .into_iter()
                    .cloned()
                    .collect(),
                None,
            ),
            jigsaw: JigsawParams::new(units),
            tabbed_box: TabbedBoxParams::new(units),
            output: None,
            notice: None,
            designer_request: None,
            folder_request: false,
            editor_request: None,
            pending: None,
            preset_request: None,
            preset: None,
        }
    }

    /// The open tool page, or `None` while the picker shows.
    pub fn open_tool(&self) -> Option<CamTool> {
        self.open
    }

    /// Opens `tool`'s page, or the picker for `None`.
    ///
    /// # Arguments
    /// * `tool` - The page to show.
    pub fn set_open_tool(&mut self, tool: Option<CamTool>) {
        self.open = tool;
    }

    /// The tools the picker lists for its search text and category.
    ///
    /// # Arguments
    /// * `tr` - Translator for the current UI language (the search matches what is shown).
    pub fn listed_tools(&self, tr: &Translator) -> Vec<CamTool> {
        let query = self.search.trim().to_lowercase();
        CamTool::ALL
            .into_iter()
            .filter(|tool| self.category == Category::All || tool.category() == self.category)
            .filter(|tool| {
                query.is_empty()
                    || tr.tr(tool.title_msgid()).to_lowercase().contains(&query)
                    || tr
                        .tr(tool.description_msgid())
                        .to_lowercase()
                        .contains(&query)
            })
            .collect()
    }

    /// Sets the picker's search text.
    ///
    /// # Arguments
    /// * `text` - The search text.
    pub fn set_search(&mut self, text: &str) {
        self.search = text.to_string();
    }

    /// Sets the picker's category filter.
    ///
    /// # Arguments
    /// * `category` - The category.
    pub fn set_category(&mut self, category: Category) {
        self.category = category;
    }

    /// The message shown above the page, if any.
    pub fn notice(&self) -> Option<&str> {
        self.notice.as_deref()
    }

    /// Shows `message` above the page.
    ///
    /// # Arguments
    /// * `message` - The message.
    pub fn set_notice(&mut self, message: String) {
        self.notice = Some(message);
    }

    /// The program the last Generate made on the open page, while the page's input is still
    /// the one it was made from.
    pub fn output(&self) -> Option<&str> {
        self.output
            .as_ref()
            .filter(|o| Some(o.tool) == self.open && o.input == self.input_key(o.tool))
            .map(|o| o.gcode.as_str())
    }

    /// Everything `tool`'s page holds, to tell when its output is out of date.
    fn input_key(&self, tool: CamTool) -> String {
        match tool {
            CamTool::SpeedsFeeds | CamTool::Jigsaw => String::new(),
            CamTool::TabbedBox => format!("{:?}", self.tabbed_box),
            CamTool::SpoilboardSurfacing => format!("{:?}", self.spoilboard_surfacing),
            CamTool::SpoilboardGrid => format!("{:?}", self.spoilboard_grid),
            CamTool::DrillPress => format!("{:?}", self.drill_press),
        }
    }

    /// Takes the Send to Editor / Preview request; the program waits in
    /// [`CamToolsPane::take_editor_gcode`].
    pub fn take_editor_request(&mut self) -> Option<CamDelivery> {
        self.editor_request.take()
    }

    /// Takes the program a Send to Editor / Preview request hands to the editor.
    pub fn take_editor_gcode(&mut self) -> Option<String> {
        self.pending.take()
    }

    /// Asks the app to hand `gcode` to the editor, as a page's Send to Editor or Preview
    /// does.
    ///
    /// # Arguments
    /// * `gcode` - The program.
    /// * `delivery` - Editor only, or the visualizer too.
    pub fn request_to_editor(&mut self, gcode: String, delivery: CamDelivery) {
        self.pending = Some(gcode);
        self.editor_request = Some(delivery);
    }

    /// The display units of the pages' lengths.
    pub fn units(&self) -> MeasurementSystem {
        self.units
    }

    /// Shows the pages' lengths in `units`, converting what is typed.
    fn set_units(&mut self, units: MeasurementSystem) {
        self.units = units;
        self.spoilboard_grid.set_units(units);
        self.spoilboard_surfacing.set_units(units);
        self.drill_press.set_units(units);
        self.jigsaw.set_units(units);
        self.tabbed_box.set_units(units);
    }

    /// The Drill Press page's input.
    pub fn drill_press_mut(&mut self) -> &mut DrillPressParams {
        &mut self.drill_press
    }

    /// The Spoilboard Surfacing page's input.
    pub fn spoilboard_surfacing_mut(&mut self) -> &mut SpoilboardSurfacingParams {
        &mut self.spoilboard_surfacing
    }

    /// The Create Spoilboard Grid page's input.
    pub fn spoilboard_grid_mut(&mut self) -> &mut SpoilboardGridParams {
        &mut self.spoilboard_grid
    }

    /// The Jigsaw Puzzle Generator page's input.
    pub fn jigsaw_mut(&mut self) -> &mut JigsawParams {
        &mut self.jigsaw
    }

    /// The Tabbed Box Maker page's input.
    pub fn tabbed_box_mut(&mut self) -> &mut TabbedBoxParams {
        &mut self.tabbed_box
    }

    /// The Speeds and Feeds page's input and results.
    pub fn speeds_feeds_mut(&mut self) -> &mut SpeedsFeedsParams {
        &mut self.speeds_feeds
    }

    /// Gives the speeds and feeds calculator the user's libraries and active device (the
    /// page's choices start over).
    ///
    /// # Arguments
    /// * `tools` - The tool library.
    /// * `materials` - The material library.
    /// * `device` - The active device profile, if any.
    pub fn set_libraries(
        &mut self,
        tools: Vec<Tool>,
        materials: Vec<Material>,
        device: Option<DeviceProfile>,
    ) {
        self.speeds_feeds = SpeedsFeedsParams::new(tools, materials, device);
    }

    /// Runs a page's action.
    fn act(&mut self, tool: CamTool, action: CamAction, tr: &Translator) {
        if tool == CamTool::SpeedsFeeds {
            // Calculate: the results show on the page; there is no program.
            if action == CamAction::Generate {
                match self.speeds_feeds.calculate() {
                    Ok(results) => self.speeds_feeds.store_results(results),
                    Err(_) => self.notice = Some(CamError::Invalid.message(tr)),
                }
            }
            return;
        }
        let delivery = match action {
            CamAction::None => return,
            CamAction::LoadPreset | CamAction::SavePreset => {
                self.ask_preset(tool, action, tr);
                return;
            }
            CamAction::SendToDesigner => {
                self.send_to_designer(tool, tr);
                return;
            }
            CamAction::ChooseFolder => {
                self.folder_request = true;
                return;
            }
            CamAction::Generate => None,
            CamAction::SendToEditor => Some(CamDelivery::Editor),
            CamAction::Preview => Some(CamDelivery::Preview),
        };
        match self.generate(tool) {
            Ok(gcode) => {
                self.notice = None;
                match delivery {
                    None => {
                        let input = self.input_key(tool);
                        self.output = Some(Output { tool, input, gcode });
                    }
                    Some(delivery) => self.request_to_editor(gcode, delivery),
                }
            }
            Err(error) => self.notice = Some(error.message(tr)),
        }
    }

    /// Generates `tool`'s program from its page.
    fn generate(&mut self, tool: CamTool) -> Result<String, CamError> {
        match tool {
            // The calculator makes numbers and the jigsaw shapes, not a program (see `act`).
            CamTool::SpeedsFeeds | CamTool::Jigsaw => Err(CamError::Invalid),
            CamTool::SpoilboardGrid => self.spoilboard_grid.generate(),
            CamTool::SpoilboardSurfacing => self.spoilboard_surfacing.generate(),
            CamTool::TabbedBox => self.tabbed_box.generate(),
            CamTool::DrillPress => self.drill_press.generate(),
        }
    }

    /// Draws `tool`'s page.
    fn show_page(&mut self, ui: &mut egui::Ui, tool: CamTool, tr: &Translator) -> CamAction {
        match tool {
            CamTool::Jigsaw => jigsaw::show(ui, &mut self.jigsaw, tr),
            CamTool::TabbedBox => tabbed_box::show(ui, &mut self.tabbed_box, tr),
            CamTool::SpeedsFeeds => speeds_feeds::show(ui, &mut self.speeds_feeds, tr),
            CamTool::SpoilboardGrid => spoilboard_grid::show(ui, &mut self.spoilboard_grid, tr),
            CamTool::SpoilboardSurfacing => {
                spoilboard_surfacing::show(ui, &mut self.spoilboard_surfacing, tr)
            }
            CamTool::DrillPress => drill_press::show(ui, &mut self.drill_press, tr),
        }
    }

    /// Draws the pane.
    fn ui(&mut self, ui: &mut egui::Ui, tr: &Translator) {
        // GTK checked the device's axes when Generate was pressed.
        let axes = device_status::get_active_num_axes();
        self.drill_press.num_axes = axes;
        self.spoilboard_surfacing.num_axes = axes;
        self.tabbed_box.num_axes = axes;
        self.drill_press.laser_mode = drill_press::device_in_laser_mode();
        if let Some(notice) = self.notice.clone() {
            ui.horizontal(|ui| {
                ui.colored_label(ui.visuals().error_fg_color, notice);
                if ui.button(tr.tr("Close")).clicked() {
                    self.notice = None;
                }
            });
            ui.separator();
        }
        match self.open {
            None => self.show_picker(ui, tr),
            Some(tool) => self.show_tool(ui, tool, tr),
        }
    }

    /// Draws `tool`'s page with its header and the generated program.
    fn show_tool(&mut self, ui: &mut egui::Ui, tool: CamTool, tr: &Translator) {
        ui.horizontal(|ui| {
            if ui.button(tr.tr("Back")).clicked() {
                self.open = None;
            }
            ui.heading(tr.tr(tool.title_msgid()));
        });
        ui.separator();
        egui::ScrollArea::vertical()
            .id_salt("cam-tool-page")
            .auto_shrink([false, false])
            .show(ui, |ui| {
                let action = self.show_page(ui, tool, tr);
                self.act(tool, action, tr);
                if let Some(gcode) = self.output() {
                    let mut text = gcode;
                    ui.add_space(8.0);
                    let name = ui.strong(tr.tr("Generated G-Code"));
                    ui.add(
                        egui::TextEdit::multiline(&mut text)
                            .id_salt("cam-generated")
                            .code_editor()
                            .desired_rows(12)
                            .desired_width(f32::INFINITY),
                    )
                    .labelled_by(name.id);
                }
            });
    }

    /// Draws the picker: search and category, the list and the chosen tool's details.
    fn show_picker(&mut self, ui: &mut egui::Ui, tr: &Translator) {
        ui.heading(tr.tr(PaneId::CamTools.title_msgid()));
        ui.horizontal(|ui| {
            let hint = tr.tr("Search tools...");
            ui.add(
                egui::TextEdit::singleline(&mut self.search)
                    .id_salt("cam-search")
                    .hint_text(hint.as_ref()),
            )
            .widget_info(|| egui::WidgetInfo::labeled(egui::Role::TextInput, true, &hint));
            let category = ui.label(tr.tr("Category"));
            egui::ComboBox::from_id_salt("cam-category")
                .selected_text(tr.tr(self.category.msgid()))
                .show_ui(ui, |ui| {
                    for category in Category::ALL {
                        ui.selectable_value(&mut self.category, category, tr.tr(category.msgid()));
                    }
                })
                .response
                .labelled_by(category.id);
        });
        ui.separator();
        let listed = self.listed_tools(tr);
        ui.columns(2, |columns| {
            egui::ScrollArea::vertical()
                .id_salt("cam-tool-list")
                .show(&mut columns[0], |ui| {
                    for tool in &listed {
                        let chosen = self.selected == Some(*tool);
                        let response = ui.selectable_label(chosen, tr.tr(tool.title_msgid()));
                        ui.weak(tr.tr(tool.description_msgid()));
                        ui.add_space(4.0);
                        if response.clicked() {
                            self.selected = Some(*tool);
                        }
                        if response.double_clicked() {
                            self.open = Some(*tool);
                        }
                    }
                });
            let ui = &mut columns[1];
            match self.selected.filter(|tool| listed.contains(tool)) {
                Some(tool) => {
                    ui.heading(tr.tr(tool.title_msgid()));
                    ui.label(tr.tr(tool.description_msgid()));
                    if ui.button(tr.tr("Open")).clicked() {
                        self.open = Some(tool);
                    }
                }
                None => {
                    ui.heading(tr.tr("Select a tool"));
                    ui.label(tr.tr("Choose a tool from the list to see details."));
                    ui.add_enabled(false, egui::Button::new(tr.tr("Open")));
                }
            }
        });
    }
}

/// The CAM Tools pane shared between the workspace, which draws it, and the app, which takes
/// its editor requests.
#[derive(Debug, Clone)]
pub struct SharedCamTools(pub Rc<RefCell<CamToolsPane>>);

impl Pane for SharedCamTools {
    fn id(&self) -> PaneId {
        PaneId::CamTools
    }

    fn title(&self, tr: &Translator) -> String {
        tr.tr(PaneId::CamTools.title_msgid()).into_owned()
    }

    fn ui(&mut self, ui: &mut egui::Ui, ctx: &mut PaneContext) {
        match self.0.try_borrow_mut() {
            Ok(mut pane) => pane.ui(ui, ctx.tr),
            Err(error) => tracing::warn!(%error, "CAM tools busy; not drawn this frame"),
        }
    }

    fn configure(&mut self, config: &Config) {
        match self.0.try_borrow_mut() {
            Ok(mut pane) => pane.set_units(config.ui.measurement_system),
            Err(error) => tracing::warn!(%error, "CAM tools busy; settings not applied"),
        }
    }
}
