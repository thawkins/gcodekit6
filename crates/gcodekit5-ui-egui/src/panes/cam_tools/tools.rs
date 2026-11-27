//! The CAM tools the pane's picker lists, in their fixed order, and the picker's category
//! filter.
//!
//! Dependencies: `egui`, `gcodekit5-app` (translations).

use gcodekit5_app::i18n::Translator;

use super::CamToolsPane;
use crate::icons::{self, Icon};
use crate::windows::help::HelpTopic;
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
    /// Every tool, in list order.
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

    /// The help topic of the tool's page (its help button); Probe Tools has none.
    pub fn help_topic(self) -> Option<HelpTopic> {
        match self {
            CamTool::Jigsaw => Some(HelpTopic::JigsawPuzzle),
            CamTool::TabbedBox => Some(HelpTopic::TabbedBoxMaker),
            CamTool::BitmapEngraving => Some(HelpTopic::LaserImageEngraver),
            CamTool::SpeedsFeeds => Some(HelpTopic::SpeedsFeedsCalculator),
            CamTool::Gerber => Some(HelpTopic::Gerber),
            CamTool::SpoilboardGrid => Some(HelpTopic::SpoilboardGrid),
            CamTool::SpoilboardSurfacing => Some(HelpTopic::SpoilboardSurfacing),
            CamTool::DrillPress => Some(HelpTopic::DrillPress),
            CamTool::ProbeRoutines => None,
        }
    }

    /// The tool's list icon: an app SVG for the jigsaw and the box, else a symbolic
    /// icon, `size` points square in the theme's text colour.
    ///
    /// # Arguments
    /// * `ui` - The ui the icon goes into; its theme decides the tint.
    /// * `size` - The edge length in points.
    pub fn icon(self, ui: &egui::Ui, size: f32) -> egui::Image<'static> {
        let symbolic = |icon: Icon| icon.themed(ui).fit_to_exact_size(egui::Vec2::splat(size));
        let app = |name: &str, fallback: Icon| {
            icons::app_icon(ui, name, size).unwrap_or_else(|| symbolic(fallback))
        };
        match self {
            CamTool::Jigsaw => app("puzzle-piece.svg", Icon::Shuffle),
            CamTool::TabbedBox => app("boxes.svg", Icon::Grid),
            CamTool::BitmapEngraving => symbolic(Icon::Camera),
            CamTool::SpeedsFeeds => symbolic(Icon::Calculator),
            CamTool::SpoilboardSurfacing => symbolic(Icon::Refresh),
            CamTool::SpoilboardGrid => symbolic(Icon::Grid),
            CamTool::Gerber => symbolic(Icon::Save),
            CamTool::DrillPress => symbolic(Icon::Mouse),
            CamTool::ProbeRoutines => symbolic(Icon::Touchpad),
        }
    }

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

/// The picker's category filter (the ids presets and settings use).
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

    /// Draws the picker: the page title, search and category, then the
    /// boxed list of tools (icon, title, description) beside the chosen tool's details.
    pub(super) fn show_picker(&mut self, ui: &mut egui::Ui, tr: &Translator) {
        crate::theme::title(
            ui,
            &tr.tr(PaneId::CamTools.title_msgid()),
            crate::theme::TITLE_1,
        );
        ui.add_space(6.0);
        ui.horizontal(|ui| {
            let hint = tr.tr("Search tools...");
            let category_width = 220.0;
            ui.add(
                egui::TextEdit::singleline(&mut self.search)
                    .id_salt("cam-search")
                    .hint_text(hint.as_ref())
                    .desired_width((ui.available_width() - category_width).max(160.0)),
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
        ui.add_space(12.0);
        let listed = self.listed_tools(tr);
        // The list starts at 45% of the width.
        let list_width = (ui.available_width() * 0.45).max(320.0);
        ui.horizontal_top(|ui| {
            ui.vertical(|ui| {
                ui.set_width(list_width);
                egui::ScrollArea::vertical()
                    .id_salt("cam-tool-list")
                    .auto_shrink([false, true])
                    .show(ui, |ui| self.tool_list(ui, tr, &listed));
            });
            ui.add_space(12.0);
            ui.vertical(|ui| {
                ui.add_space(12.0);
                match self.selected.filter(|tool| listed.contains(tool)) {
                    Some(tool) => {
                        crate::theme::title(ui, &tr.tr(tool.title_msgid()), crate::theme::TITLE_2);
                        ui.add_space(6.0);
                        ui.label(tr.tr(tool.description_msgid()));
                        ui.add_space(6.0);
                        if ui
                            .add(crate::theme::primary_button(tr.tr("Open")))
                            .clicked()
                        {
                            self.open = Some(tool);
                        }
                    }
                    None => {
                        crate::theme::title(ui, &tr.tr("Select a tool"), crate::theme::TITLE_2);
                        ui.add_space(6.0);
                        ui.label(tr.tr("Choose a tool from the list to see details."));
                        ui.add_space(6.0);
                        ui.add_enabled(false, crate::theme::primary_button(tr.tr("Open")));
                    }
                }
            });
        });
    }

    /// The boxed list of `listed` tools: one row each, separated by lines, the chosen one
    /// highlighted. A click chooses a tool, a double click opens it.
    fn tool_list(&mut self, ui: &mut egui::Ui, tr: &Translator, listed: &[CamTool]) {
        crate::theme::group_frame(ui)
            .inner_margin(egui::Margin::same(0))
            .show(ui, |ui| {
                ui.spacing_mut().item_spacing.y = 0.0;
                for (index, tool) in listed.iter().enumerate() {
                    if index > 0 {
                        let y = ui.cursor().top();
                        let x = ui.max_rect().x_range();
                        ui.painter()
                            .hline(x, y, ui.visuals().widgets.noninteractive.bg_stroke);
                    }
                    let chosen = self.selected == Some(*tool);
                    let response = tool_row(ui, tr, *tool, chosen);
                    if response.clicked() {
                        self.selected = Some(*tool);
                    }
                    if response.double_clicked() {
                        self.open = Some(*tool);
                    }
                }
            });
    }
}

/// Room around a row's contents: 10 px top and bottom, 12 px at the sides.
const ROW_MARGIN: egui::Vec2 = egui::vec2(12.0, 10.0);
/// Edge of a tool list icon: 24 px.
const ROW_ICON: f32 = 24.0;
/// Gap between a row's title and its description.
const ROW_GAP: f32 = 4.0;
/// Font size of a row's description (a caption).
pub const CAPTION_SIZE: f32 = 12.0;

/// One row of the tool list: icon, bold title and the description under it, the row as
/// tall as its wrapped text. The title is the row's accessible name and the description
/// its hint.
fn tool_row(ui: &mut egui::Ui, tr: &Translator, tool: CamTool, chosen: bool) -> egui::Response {
    let title = tr.tr(tool.title_msgid()).into_owned();
    let description = tr.tr(tool.description_msgid()).into_owned();
    let visuals = ui.visuals().clone();
    let text_color = if chosen {
        visuals.selection.stroke.color
    } else {
        visuals.strong_text_color()
    };
    let weak = if chosen {
        text_color
    } else {
        visuals.weak_text_color()
    };
    let width = ui.available_width();
    let text_left = ROW_MARGIN.x + ROW_ICON + ROW_MARGIN.x;
    let wrap = (width - text_left - ROW_MARGIN.x).max(40.0);
    let title_font = egui::TextStyle::Body.resolve(ui.style());
    let title_galley = ui
        .painter()
        .layout(title.clone(), title_font, text_color, wrap);
    let caption = egui::FontId::proportional(CAPTION_SIZE);
    let description_galley = ui
        .painter()
        .layout(description.clone(), caption, weak, wrap);
    let text_height = title_galley.size().y + ROW_GAP + description_galley.size().y;
    let height = 2.0 * ROW_MARGIN.y + text_height.max(ROW_ICON);
    let (rect, response) = ui.allocate_exact_size(egui::vec2(width, height), egui::Sense::click());
    response.widget_info(|| {
        let mut info = egui::WidgetInfo::selected(egui::Role::Button, true, chosen, &title);
        info.hint_text = Some(description.clone());
        info
    });
    if ui.is_rect_visible(rect) {
        if chosen {
            ui.painter()
                .rect_filled(rect, 0.0, visuals.selection.bg_fill);
        } else if response.hovered() {
            ui.painter()
                .rect_filled(rect, 0.0, visuals.widgets.hovered.weak_bg_fill);
        }
        let top = rect.top() + ROW_MARGIN.y;
        let icon_rect = egui::Rect::from_min_size(
            egui::pos2(rect.left() + ROW_MARGIN.x, top),
            egui::Vec2::splat(ROW_ICON),
        );
        tool.icon(ui, ROW_ICON).paint_at(ui, icon_rect);
        let title_pos = egui::pos2(rect.left() + text_left, top);
        // Bold heading rows: the same faux bold as `theme::section_heading`.
        ui.painter()
            .galley(title_pos, title_galley.clone(), text_color);
        let title_height = title_galley.size().y;
        ui.painter()
            .galley(title_pos + egui::vec2(0.6, 0.0), title_galley, text_color);
        let description_pos = title_pos + egui::vec2(0.0, title_height + ROW_GAP);
        ui.painter()
            .galley(description_pos, description_galley, weak);
    }
    response
}
