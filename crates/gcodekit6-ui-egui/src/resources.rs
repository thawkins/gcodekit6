//! Files embedded in the binary: the help topics (markdown per language), the help images,
//! the designer icons and the app images, read from `resources/` at build time.
//!
//! Dependencies: none (`include_str!` / `include_bytes!` only).

/// Embeds `resources/<dir>/<name>` for each name as `(name, bytes)`.
macro_rules! embed_bytes {
    ($dir:literal: $($name:literal),* $(,)?) => {
        &[$((
            $name,
            include_bytes!(concat!(env!("CARGO_MANIFEST_DIR"), "/resources/", $dir, "/", $name))
                as &[u8],
        )),*]
    };
}

/// Embeds `resources/markdown/<lang>/<topic>.md` for each language and topic as
/// `(lang, topic, text)`.
macro_rules! embed_markdown {
    ($($lang:literal: [$($topic:literal),* $(,)?]),* $(,)?) => {
        &[$($((
            $lang,
            $topic,
            include_str!(concat!(
                env!("CARGO_MANIFEST_DIR"), "/resources/markdown/", $lang, "/", $topic, ".md"
            )),
        )),*),*]
    };
}

/// The languages the help topics are written in; English is the fallback.
pub const HELP_LANGUAGES: [&str; 2] = ["en", "es"];

/// Every help topic file as `(language, topic id, markdown)`.
const HELP_MARKDOWN: &[(&str, &str, &str)] = embed_markdown!(
    "en": [
        "cam_tools", "designer", "device_config", "device_console", "device_manager",
        "drill_press", "gcode_editor", "gerber", "index", "jigsaw_puzzle",
        "laser_image_engraver", "machine_control", "materials_manager", "settings",
        "speeds_feeds_calculator", "spoilboard_grid", "spoilboard_surfacing",
        "tabbed_box_maker", "tools_manager", "visualizer",
    ],
    "es": [
        "cam_tools", "designer", "device_config", "device_console", "device_manager",
        "drill_press", "gcode_editor", "gerber", "index", "jigsaw_puzzle",
        "laser_image_engraver", "machine_control", "materials_manager", "settings",
        "speeds_feeds_calculator", "spoilboard_grid", "spoilboard_surfacing",
        "tabbed_box_maker", "tools_manager", "visualizer",
    ],
);

/// The images the help topics show, by file name.
pub const HELP_IMAGES: &[(&str, &[u8])] = embed_bytes!("help_images":
    "designer.png", "gcode_frame.png", "gcodekit6_1.png", "global_properties.png",
    "individual_properties.png", "machine_control.png", "order_objects_1.png",
    "order_objects_2.png", "visualizer3d1.png", "visualizer3d2.png", "visualizer3d3.png",
    "visualizer3d4.png", "visualizer3d5.png", "visualizer3d.png", "visualizer.png",
    "warning_limits.png",
);

/// The designer and CAM tool icons (SVG), by file name.
pub const ICONS: &[(&str, &[u8])] = embed_bytes!("icons":
    "boxes.svg", "circle.svg", "ellipse.svg", "gear.svg", "grab.svg", "line.svg",
    "Lsupport.svg", "move_to_bottom.svg", "move_to_top.svg", "poligon.svg", "polyline.svg",
    "puzzle-piece.svg", "rectangle.svg", "scroll_bars.svg", "select.svg", "slot.svg",
    "sprocket1.svg", "sprocket.svg", "text.svg", "timingpulley.svg", "top_view.svg",
    "triangle.svg", "Usupport.svg",
);

/// The app images (logo, emergency stop), by file name.
pub const IMAGES: &[(&str, &[u8])] =
    embed_bytes!("images": "eStop2.png", "gcodekit6_1.png", "gcodekit6.png");

/// Looks `name` up in a `(name, bytes)` table.
fn lookup(table: &[(&str, &'static [u8])], name: &str) -> Option<&'static [u8]> {
    table
        .iter()
        .find(|(n, _)| *n == name)
        .map(|(_, bytes)| *bytes)
}

/// The markdown of help topic `topic` in language `lang`, if that file is embedded.
///
/// # Arguments
/// * `lang` - A language code from [`HELP_LANGUAGES`].
/// * `topic` - A topic id such as `"index"`.
pub fn help_markdown(lang: &str, topic: &str) -> Option<&'static str> {
    HELP_MARKDOWN
        .iter()
        .find(|(l, t, _)| *l == lang && *t == topic)
        .map(|(_, _, text)| *text)
}

/// Every embedded help topic file as `(language, topic id, markdown)`.
pub fn help_markdown_files() -> impl Iterator<Item = (&'static str, &'static str, &'static str)> {
    HELP_MARKDOWN.iter().copied()
}

/// The bytes of help image `name` (a file name such as `"designer.png"`).
///
/// # Arguments
/// * `name` - The image's file name.
pub fn help_image(name: &str) -> Option<&'static [u8]> {
    lookup(HELP_IMAGES, name)
}

/// The SVG source of icon `name` (a file name such as `"gear.svg"`).
///
/// # Arguments
/// * `name` - The icon's file name.
pub fn icon(name: &str) -> Option<&'static [u8]> {
    lookup(ICONS, name)
}

/// The bytes of app image `name` (a file name such as `"eStop2.png"`).
///
/// # Arguments
/// * `name` - The image's file name.
pub fn image(name: &str) -> Option<&'static [u8]> {
    lookup(IMAGES, name)
}
