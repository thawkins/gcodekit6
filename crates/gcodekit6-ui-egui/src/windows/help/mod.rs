//! The help browser: the topic list on the left, the open topic on the right, Back and
//! Forward through the topics visited.
//!
//! Topics are the embedded markdown files ([`crate::resources`]) in the app language, or in
//! English when that language has no help pages. Links to `help:<id>` open that
//! topic; web links open in the browser (egui's `open_url`). Windows open their own topic
//! with [`help_button`]; the app takes the request with [`take_request`].
//!
//! Dependencies: `egui`, `image` (decodes the help images), `gcodekit6-app` (translations).

pub mod markdown;
mod render;

use std::collections::HashMap;

use gcodekit6_app::i18n::Translator;

use crate::resources;
use markdown::Block;

/// A help topic.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash)]
pub enum HelpTopic {
    /// The topic list.
    #[default]
    Index,
    /// Preferences.
    Settings,
    /// The designer.
    Designer,
    /// The visualizer.
    Visualizer,
    /// Machine control.
    MachineControl,
    /// The G-code editor.
    GcodeEditor,
    /// The device console.
    DeviceConsole,
    /// The CAM tools.
    CamTools,
    /// The tabbed box maker.
    TabbedBoxMaker,
    /// The jigsaw puzzle generator.
    JigsawPuzzle,
    /// The laser image engraver.
    LaserImageEngraver,
    /// The speeds and feeds calculator.
    SpeedsFeedsCalculator,
    /// Spoilboard surfacing.
    SpoilboardSurfacing,
    /// The spoilboard grid.
    SpoilboardGrid,
    /// Gerber to G-code.
    Gerber,
    /// The drill press.
    DrillPress,
    /// The device manager.
    DeviceManager,
    /// Firmware settings (the "Device Config" page).
    DeviceConfig,
    /// The CNC tools manager.
    ToolsManager,
    /// The materials manager.
    MaterialsManager,
}

impl HelpTopic {
    /// Every topic, in the order the topic list shows them.
    pub const ALL: [HelpTopic; 20] = [
        Self::Index,
        Self::Settings,
        Self::Designer,
        Self::Visualizer,
        Self::MachineControl,
        Self::GcodeEditor,
        Self::DeviceConsole,
        Self::CamTools,
        Self::TabbedBoxMaker,
        Self::JigsawPuzzle,
        Self::LaserImageEngraver,
        Self::SpeedsFeedsCalculator,
        Self::SpoilboardSurfacing,
        Self::SpoilboardGrid,
        Self::Gerber,
        Self::DrillPress,
        Self::DeviceManager,
        Self::DeviceConfig,
        Self::ToolsManager,
        Self::MaterialsManager,
    ];

    /// The topic's id: its markdown file name and its `help:` link target.
    pub fn id(self) -> &'static str {
        self.entry().0
    }

    /// The English msgid of the topic's title in the topic list.
    pub fn title(self) -> &'static str {
        self.entry().1
    }

    /// Whether the topic list shows the topic under the one before it.
    pub fn is_sub_topic(self) -> bool {
        self.entry().2
    }

    /// The topic with id `id`.
    ///
    /// # Arguments
    /// * `id` - A topic id such as `"designer"`.
    pub fn from_id(id: &str) -> Option<HelpTopic> {
        Self::ALL.into_iter().find(|t| t.id() == id)
    }

    /// `(id, title msgid, sub-topic)`.
    fn entry(self) -> (&'static str, &'static str, bool) {
        match self {
            Self::Index => ("index", n_!("Index"), false),
            Self::Settings => ("settings", n_!("Preferences"), false),
            Self::Designer => ("designer", n_!("Designer"), false),
            Self::Visualizer => ("visualizer", n_!("Visualizer"), false),
            Self::MachineControl => ("machine_control", n_!("Machine Control"), false),
            Self::GcodeEditor => ("gcode_editor", n_!("G-Code Editor"), true),
            Self::DeviceConsole => ("device_console", n_!("Device Console"), true),
            Self::CamTools => ("cam_tools", n_!("CAM Tools"), false),
            Self::TabbedBoxMaker => ("tabbed_box_maker", n_!("Tabbed Box Maker"), true),
            Self::JigsawPuzzle => ("jigsaw_puzzle", n_!("Jigsaw Puzzle Generator"), true),
            Self::LaserImageEngraver => ("laser_image_engraver", n_!("Laser Image Engraver"), true),
            Self::SpeedsFeedsCalculator => (
                "speeds_feeds_calculator",
                n_!("Speeds and Feeds Calculator"),
                true,
            ),
            Self::SpoilboardSurfacing => {
                ("spoilboard_surfacing", n_!("Spoilboard Surfacing"), true)
            }
            Self::SpoilboardGrid => ("spoilboard_grid", n_!("Create Spoilboard Grid"), true),
            Self::Gerber => ("gerber", n_!("Gerber to G-Code"), true),
            Self::DrillPress => ("drill_press", n_!("Drill Press"), true),
            Self::DeviceManager => ("device_manager", n_!("Device Manager"), false),
            Self::DeviceConfig => ("device_config", n_!("Device Config"), false),
            Self::ToolsManager => ("tools_manager", n_!("CNC Tools"), false),
            Self::MaterialsManager => ("materials_manager", n_!("Materials"), false),
        }
    }
}

/// The markdown of `topic` in the language `tr` translates into, or in English when that
/// language has no help pages, with the language used.
///
/// # Arguments
/// * `tr` - The app's translator; its language picks the help language.
/// * `topic` - The topic.
pub fn load_topic(tr: &Translator, topic: HelpTopic) -> (&'static str, &'static str) {
    let preferred = tr.language().unwrap_or("en");
    if let Some((lang, text)) = resources::HELP_LANGUAGES
        .into_iter()
        .find(|lang| *lang == preferred)
        .and_then(|lang| Some((lang, resources::help_markdown(lang, topic.id())?)))
    {
        return (lang, text);
    }
    if preferred != "en" {
        tracing::debug!(topic = topic.id(), preferred, "help topic in English");
    }
    match resources::help_markdown("en", topic.id()) {
        Some(text) => ("en", text),
        None => {
            tracing::warn!(topic = topic.id(), "help topic not embedded");
            ("en", "")
        }
    }
}

/// The memory key of a help button's request.
fn request_id() -> egui::Id {
    egui::Id::unique("gcodekit6-help-request")
}

/// Draws the help icon button (named "Help (F1)") that asks for help on `topic`; the app
/// opens it after the frame's windows are drawn ([`take_request`]).
///
/// # Arguments
/// * `ui` - Where the button goes.
/// * `topic` - The topic it opens.
/// * `tr` - Translates the tooltip.
pub fn help_button(ui: &mut egui::Ui, topic: HelpTopic, tr: &Translator) {
    let label = tr.tr(n_!("Help (F1)"));
    let response = crate::icons::icon_only_button(ui, crate::icons::Icon::Help, label.as_ref());
    if response.clicked() {
        ui.ctx().data_mut(|d| d.insert_temp(request_id(), topic));
    }
}

/// A row with a [`help_button`] at its right end, for the top of a window.
///
/// # Arguments
/// * `ui` - Where the row goes.
/// * `topic` - The topic the button opens.
/// * `tr` - Translates the tooltip.
pub fn help_row(ui: &mut egui::Ui, topic: HelpTopic, tr: &Translator) {
    ui.horizontal(|ui| {
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            help_button(ui, topic, tr);
        });
    });
}

/// Takes the topic a [`help_button`] asked for this frame, if any.
///
/// # Arguments
/// * `ctx` - The egui context.
pub fn take_request(ctx: &egui::Context) -> Option<HelpTopic> {
    ctx.data_mut(|d| d.remove_temp::<HelpTopic>(request_id()))
}

/// The topic being read, parsed.
struct Page {
    /// The language it is in.
    lang: &'static str,
    /// The topic.
    topic: HelpTopic,
    /// Its blocks.
    blocks: Vec<Block>,
}

/// The help window.
#[derive(Default)]
pub struct HelpWindow {
    /// Whether the window is open.
    open: bool,
    /// The topic shown.
    current: HelpTopic,
    /// Topics Back returns to, the latest last.
    back: Vec<HelpTopic>,
    /// Topics Forward goes to, the next last.
    forward: Vec<HelpTopic>,
    /// The current topic, parsed.
    page: Option<Page>,
    /// The help images decoded so far, by file name.
    textures: HashMap<String, egui::TextureHandle>,
}

impl std::fmt::Debug for HelpWindow {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("HelpWindow")
            .field("open", &self.open)
            .field("current", &self.current)
            .field("back", &self.back)
            .field("forward", &self.forward)
            .field("textures", &self.textures.len())
            .finish_non_exhaustive()
    }
}

impl HelpWindow {
    /// Opens the window on `topic`. An open window goes to `topic` like a link (Back
    /// returns); a closed one starts a new history.
    ///
    /// # Arguments
    /// * `topic` - The topic to show.
    pub fn open(&mut self, topic: HelpTopic) {
        if self.open {
            self.go_to(topic);
        } else {
            self.open = true;
            self.current = topic;
            self.back.clear();
            self.forward.clear();
        }
    }

    /// Whether the window is open.
    pub fn is_open(&self) -> bool {
        self.open
    }

    /// The topic shown.
    pub fn current(&self) -> HelpTopic {
        self.current
    }

    /// Shows `topic`, keeping the current one for Back and dropping Forward.
    fn go_to(&mut self, topic: HelpTopic) {
        if topic != self.current {
            self.back.push(self.current);
            self.current = topic;
            self.forward.clear();
        }
    }

    /// Returns to the previous topic.
    fn go_back(&mut self) {
        if let Some(topic) = self.back.pop() {
            self.forward.push(self.current);
            self.current = topic;
        }
    }

    /// Goes to the topic Back left.
    fn go_forward(&mut self) {
        if let Some(topic) = self.forward.pop() {
            self.back.push(self.current);
            self.current = topic;
        }
    }

    /// Draws the window when it is open.
    ///
    /// # Arguments
    /// * `ctx` - The egui context.
    /// * `tr` - Translates the labels and picks the help language.
    pub fn show(&mut self, ctx: &egui::Context, tr: &Translator) {
        if !self.open {
            return;
        }
        let (lang, text) = load_topic(tr, self.current);
        if self
            .page
            .as_ref()
            .is_none_or(|p| p.lang != lang || p.topic != self.current)
        {
            self.page = Some(Page {
                lang,
                topic: self.current,
                blocks: markdown::parse(text),
            });
        }
        let mut open = self.open;
        let mut action = None;
        egui::Window::new(tr.tr(n_!("Help")))
            .id(egui::Id::unique("gcodekit6-help"))
            .open(&mut open)
            .default_size([1000.0, 700.0])
            .show(ctx, |ui| action = self.contents(ui, tr));
        self.open = open;
        match action {
            Some(Action::Back) => self.go_back(),
            Some(Action::Forward) => self.go_forward(),
            Some(Action::Show(topic)) => self.go_to(topic),
            None => {}
        }
    }

    /// The toolbar, the topic list and the topic.
    fn contents(&mut self, ui: &mut egui::Ui, tr: &Translator) -> Option<Action> {
        let mut action = None;
        ui.horizontal(|ui| {
            let back = ui.add_enabled(!self.back.is_empty(), egui::Button::new(tr.tr(n_!("Back"))));
            if back.clicked() {
                action = Some(Action::Back);
            }
            let forward = ui.add_enabled(
                !self.forward.is_empty(),
                egui::Button::new(tr.tr(n_!("Forward"))),
            );
            if forward.clicked() {
                action = Some(Action::Forward);
            }
        });
        ui.separator();
        ui.horizontal_top(|ui| {
            let size = egui::vec2(220.0, ui.available_height());
            ui.allocate_ui_with_layout(size, egui::Layout::top_down(egui::Align::Min), |ui| {
                egui::ScrollArea::vertical()
                    .id_salt("gcodekit6-help-topics")
                    .show(ui, |ui| {
                        for topic in HelpTopic::ALL {
                            ui.horizontal(|ui| {
                                if topic.is_sub_topic() {
                                    ui.add_space(16.0);
                                }
                                let label = tr.tr(topic.title());
                                if ui.selectable_label(topic == self.current, label).clicked() {
                                    action = Some(Action::Show(topic));
                                }
                            });
                        }
                    });
            });
            ui.separator();
            let Some(page) = &self.page else {
                return;
            };
            egui::ScrollArea::vertical()
                .id_salt(("gcodekit6-help-page", page.topic.id()))
                .auto_shrink([false, false])
                .show(ui, |ui| {
                    // The page is inside the horizontal row: lay its blocks out top-down.
                    ui.vertical(|ui| {
                        if let Some(topic) = render::blocks(ui, &page.blocks, &mut self.textures) {
                            action = Some(Action::Show(topic));
                        }
                    });
                });
        });
        action
    }
}

/// What the user asked of the window this frame.
enum Action {
    /// Back.
    Back,
    /// Forward.
    Forward,
    /// Show a topic (topic list or link).
    Show(HelpTopic),
}

/// The English msgids this window shows, for the translation test.
pub fn msgids() -> Vec<&'static str> {
    HelpTopic::ALL
        .into_iter()
        .map(HelpTopic::title)
        .chain([n_!("Help"), n_!("Back"), n_!("Forward"), n_!("Help (F1)")])
        .collect()
}
