//! The CAM tools the pane's picker lists, in GTK's order, and the picker's category
//! filter.
//!
//! Dependencies: none beyond the translation macro.

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
}

impl CamTool {
    /// Every tool, in GTK's list order.
    pub const ALL: [CamTool; 8] = [
        CamTool::Jigsaw,
        CamTool::TabbedBox,
        CamTool::BitmapEngraving,
        CamTool::SpeedsFeeds,
        CamTool::SpoilboardSurfacing,
        CamTool::SpoilboardGrid,
        CamTool::Gerber,
        CamTool::DrillPress,
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
