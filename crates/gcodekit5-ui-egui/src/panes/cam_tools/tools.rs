//! The CAM tools the pane's picker lists, in GTK's order, and the picker's category
//! filter.
//!
//! Dependencies: `egui`, `gcodekit5-app` (translations).

use gcodekit5_app::i18n::Translator;

use super::CamToolsPane;
use crate::workspace::PaneId;

/// One CAM tool of the picker.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum CamTool {
    /// Jigsaw Puzzle Generator.
    Jigsaw,
    /// Tabbed Box Maker.
    TabbedBox,
    /// Laser Image Engraver.
    BitmapEngraving,
    /// Speeds and Feeds Calculator.
    SpeedsFeeds,
    /// Gerber to G-Code.
    Gerber,
    /// Create Spoilboard Grid.
    SpoilboardGrid,
    /// Spoilboard Surfacing.
    SpoilboardSurfacing,
    /// Drill Press.
    DrillPress,
    /// Probe Tools.
    ProbeRoutines,
}

impl CamTool {
    /// Every tool, in GTK's list order.
    pub const ALL: [CamTool; 9] = [
        CamTool::Jigsaw,
        CamTool::TabbedBox,
        CamTool::BitmapEngraving,
        CamTool::SpeedsFeeds,
        CamTool::SpoilboardSurfacing,
        CamTool::SpoilboardGrid,
        CamTool::Gerber,
        CamTool::DrillPress,
        CamTool::ProbeRoutines,
    ];

    /// The msgid of the tool's title.
    pub fn title_msgid(self) -> &'static str {
        match self {
            CamTool::Jigsaw => n_!("Jigsaw Puzzle Generator"),
            CamTool::TabbedBox => n_!("Tabbed Box Maker"),
            CamTool::BitmapEngraving => n_!("Laser Image Engraver"),
            CamTool::SpeedsFeeds => n_!("Speeds and Feeds Calculator"),
            CamTool::Gerber => n_!("Gerber to G-Code"),
            CamTool::SpoilboardGrid => n_!("Create Spoilboard Grid"),
            CamTool::SpoilboardSurfacing => n_!("Spoilboard Surfacing"),
            CamTool::DrillPress => n_!("Drill Press"),
            CamTool::ProbeRoutines => n_!("Probe Tools"),
        }
    }

    /// The msgid of the tool's description.
    pub fn description_msgid(self) -> &'static str {
        match self {
            CamTool::Jigsaw => n_!("Create custom jigsaw puzzle patterns from images"),
            CamTool::TabbedBox => n_!("Generate G-code for laser/CNC cut boxes with finger joints"),
            CamTool::BitmapEngraving => n_!("Convert raster images to G-code for laser engraving"),
            CamTool::SpeedsFeeds => n_!("Calculate cutting speeds and feeds for your materials"),
            CamTool::Gerber => n_!("Convert Gerber files to G-Code for PCB milling"),
            CamTool::SpoilboardGrid => n_!("Generate grid patterns for spoilboard alignment"),
            CamTool::SpoilboardSurfacing => {
                n_!("Generate surfacing toolpaths to flatten your spoilboard")
            }
            CamTool::DrillPress => {
                n_!("Emulate a drill press with peck drilling and helical cycles")
            }
            CamTool::ProbeRoutines => n_!(
                "Touch probe routines for WCS setup: Z-touch, edge find, corner find, \
                 bore/boss center, tool length"
            ),
        }
    }

    /// The picker category the tool is listed under.
    pub fn category(self) -> Category {
        match self {
            CamTool::Jigsaw | CamTool::TabbedBox | CamTool::Gerber => Category::Generators,
            CamTool::BitmapEngraving => Category::Engraving,
            CamTool::SpeedsFeeds => Category::Calculators,
            CamTool::SpoilboardGrid => Category::Maintenance,
            CamTool::SpoilboardSurfacing => Category::Maintenance,
            CamTool::ProbeRoutines => Category::Maintenance,
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

impl CamToolsPane {
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

    /// Draws the picker: search and category, the list and the chosen tool's details.
    pub(super) fn show_picker(&mut self, ui: &mut egui::Ui, tr: &Translator) {
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
