//! The icon catalog: the Tabler outline icons (MIT, `resources/icons/tabler/`) that replace the
//! Adwaita symbolic icons the app used before, plus the buttons that draw them.
//!
//! Icons are embedded SVGs prepared once by `craft_ui::icons::SvgIconSet` (`currentColor`
//! becomes white) and tinted at draw time with the theme's text colour, so they follow the
//! light and dark themes. [`install`] must run once on the context before any icon is drawn.
//!
//! The app's own SVGs (the designer tools, the shape gallery, the z-order buttons) are not
//! in [`Icon`]; [`app_icon`] draws them from `resources/icons/`.
//!
//! Dependencies: `craft-ui` (icon set), `egui_extras` (SVG image loader).

use std::sync::OnceLock;

use craft_ui::icons::{SvgIconSet, SvgStyle};
use egui::{Color32, Image, Response, Sense, Ui, Vec2};

/// Declares [`Icon`] and its Tabler file name (without `.svg`), embedding every file.
macro_rules! icons {
    ($($variant:ident => $file:literal,)*) => {
        /// One icon per use; each maps to a Tabler file.
        #[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
        pub enum Icon {
            $(
                #[doc = concat!("Tabler `", $file, "`.")]
                $variant,
            )*
        }

        impl Icon {
            /// Every icon, in declaration order.
            pub const ALL: &'static [Icon] = &[$(Icon::$variant),*];

            /// The catalog name (the Tabler file name without `.svg`).
            pub fn name(self) -> &'static str {
                match self {
                    $(Icon::$variant => $file,)*
                }
            }
        }

        /// The embedded files as `(name, svg bytes)`.
        const FILES: &[(&str, &[u8])] = &[$((
            $file,
            include_bytes!(concat!(
                env!("CARGO_MANIFEST_DIR"), "/resources/icons/tabler/", $file, ".svg"
            )) as &[u8],
        )),*];
    };
}

icons! {
    JogLeft => "chevron-left",
    JogUp => "chevron-up",
    JogDown => "chevron-down",
    JogRight => "chevron-right",
    Top => "arrow-bar-to-up",
    Bottom => "arrow-bar-to-down",
    Jump => "arrow-forward",
    Home => "home",
    Clear => "backspace",
    Abort => "circle-x",
    Play => "player-play",
    Pause => "player-pause",
    Stop => "player-stop",
    Refresh => "refresh",
    Unlock => "lock-open",
    Zero => "focus-centered",
    Connect => "plug-connected",
    Save => "device-floppy",
    Open => "folder-open",
    Copy => "copy",
    Trash => "trash",
    Replace => "replace",
    Add => "plus",
    AddImage => "photo-plus",
    Files => "files",
    BookmarkAdd => "bookmark-plus",
    Show => "eye",
    Hide => "eye-off",
    Restore => "arrows-minimize",
    ZoomIn => "zoom-in",
    ZoomOut => "zoom-out",
    Fit => "maximize",
    Grid => "layout-grid",
    Select => "list-check",
    Group => "layers-union",
    Ungroup => "layers-subtract",
    Network => "network",
    Disconnected => "plug-connected-x",
    Transfer => "transfer",
    Send => "send",
    Exchange => "arrows-exchange",
    Display => "device-desktop",
    Mouse => "mouse",
    Touchpad => "hand-finger",
    Close => "x",
    Ok => "circle-check",
    Help => "help-circle",
    Error => "alert-circle",
    Camera => "camera",
    Calculator => "calculator",
    Shuffle => "arrows-shuffle",
}

/// Edge length of an icon in points, a 16 px symbolic icon plus padding.
pub const ICON_SIZE: f32 = 18.0;

/// The prepared catalog, built on first use.
fn set() -> &'static SvgIconSet {
    static SET: OnceLock<SvgIconSet> = OnceLock::new();
    SET.get_or_init(|| {
        SvgIconSet::new(
            FILES,
            SvgStyle {
                current_color: "white",
                stroke_width: None,
            },
            "bytes://gcodekit5/icons/tabler/",
            "help-circle",
        )
    })
}

impl Icon {
    /// The embedded SVG source of this icon (before `currentColor` is replaced).
    pub fn svg(self) -> &'static [u8] {
        FILES
            .iter()
            .find(|(name, _)| *name == self.name())
            .map_or(&[], |(_, bytes)| bytes)
    }

    /// The icon as an untinted image, `ICON_SIZE` square. Tint it with [`tint`] (the theme's
    /// text colour) before drawing on a themed background.
    pub fn image(self) -> Image<'static> {
        Image::new(set().source(self.name()))
            .fit_to_exact_size(Vec2::splat(ICON_SIZE))
            .tint(Color32::WHITE)
    }

    /// The icon tinted with the theme's text colour for `ui`. Installs the SVG loader on
    /// `ui`'s context if it is missing (one lookup), so a pane drawn outside the app, as in
    /// a test harness, still shows its icons.
    pub fn themed(self, ui: &Ui) -> Image<'static> {
        install(ui.ctx());
        self.image().tint(tint(ui))
    }
}

/// The colour icons are drawn in: the theme's text colour.
pub fn tint(ui: &Ui) -> Color32 {
    ui.visuals().text_color()
}

/// Installs egui_extras' image loaders (SVG) on `ctx`; safe to call more than once.
pub fn install(ctx: &egui::Context) {
    egui_extras::install_image_loaders(ctx);
}

/// A button with an icon and a text label; the label is its accessible name.
///
/// # Arguments
/// * `ui` - Where to draw.
/// * `icon` - The icon, tinted with the theme's text colour.
/// * `label` - The button text.
pub fn icon_button(ui: &mut Ui, icon: Icon, label: &str) -> Response {
    ui.add(egui::Button::image_and_text(icon.themed(ui), label))
}

/// A button with only an icon; `tooltip` is both its hover text and its accessible name.
///
/// # Arguments
/// * `ui` - Where to draw.
/// * `icon` - The icon, tinted with the theme's text colour.
/// * `tooltip` - The hover text and accessible name.
pub fn icon_only_button(ui: &mut Ui, icon: Icon, tooltip: &str) -> Response {
    let response = ui.add(egui::Button::image(icon.themed(ui)).sense(Sense::click()));
    let enabled = response.enabled();
    response.widget_info(|| egui::WidgetInfo::labeled(egui::Role::Button, enabled, tooltip));
    response.on_hover_text(tooltip)
}

/// The app SVGs drawn in their own colours: they have coloured fills (or black strokes on a
/// white fill) that a tint would spoil.
const UNTINTED_APP_ICONS: [&str; 7] = [
    "boxes.svg",
    "gear.svg",
    "move_to_bottom.svg",
    "move_to_top.svg",
    "puzzle-piece.svg",
    "scroll_bars.svg",
    "top_view.svg",
];

/// The grey line colours of the app SVGs, drawn white so the tint gives the text colour.
const APP_ICON_GREYS: [&str; 2] = ["#9d9d9d", "#999999"];

/// An app SVG's file name and its prepared bytes.
type PreparedIcon = (&'static str, std::sync::Arc<[u8]>);

/// Every app SVG with its grey lines made white, prepared once.
fn app_icon_bytes(name: &str) -> Option<std::sync::Arc<[u8]>> {
    static PREPARED: OnceLock<Vec<PreparedIcon>> = OnceLock::new();
    PREPARED
        .get_or_init(|| {
            crate::resources::ICONS
                .iter()
                .map(|(file, bytes)| {
                    let mut svg = String::from_utf8_lossy(bytes).into_owned();
                    if !UNTINTED_APP_ICONS.contains(file) {
                        for grey in APP_ICON_GREYS {
                            svg = svg.replace(grey, "#ffffff");
                        }
                    }
                    (*file, std::sync::Arc::from(svg.into_bytes()))
                })
                .collect()
        })
        .iter()
        .find(|(file, _)| *file == name)
        .map(|(_, bytes)| std::sync::Arc::clone(bytes))
}

/// The app's own SVG `name` (a file in `resources/icons/`, such as `"circle.svg"`) as an
/// image of `size` points square. Grey line art is drawn in the theme's text colour like an
/// [`Icon`]; the files with coloured fills keep their colours. Installs the SVG loader if
/// it is missing.
///
/// # Arguments
/// * `ui` - The ui the image goes into; its theme decides the tint.
/// * `name` - The file name.
/// * `size` - The edge length in points.
///
/// # Returns
/// The image, or `None` when no such file is embedded.
pub fn app_icon(ui: &Ui, name: &str, size: f32) -> Option<Image<'static>> {
    let bytes = app_icon_bytes(name)?;
    install(ui.ctx());
    let image = Image::from_bytes(
        format!("bytes://gcodekit5/icons/app/{name}"),
        egui::load::Bytes::Shared(bytes),
    )
    .fit_to_exact_size(Vec2::splat(size));
    Some(if UNTINTED_APP_ICONS.contains(&name) {
        image
    } else {
        image.tint(tint(ui))
    })
}

/// A square button showing app SVG `name` (see [`app_icon`]), drawn selected when
/// `selected`; `tooltip` is both its hover text and its accessible name. A missing file
/// falls back to the tooltip as text.
///
/// # Arguments
/// * `ui` - Where to draw.
/// * `name` - The app SVG's file name.
/// * `side` - The button's edge length in points.
/// * `selected` - Whether to draw it selected (the active tool).
/// * `tooltip` - The hover text and accessible name.
pub fn app_icon_button(
    ui: &mut Ui,
    name: &str,
    side: f32,
    selected: bool,
    tooltip: &str,
) -> Response {
    let button = match app_icon(ui, name, side * 0.5) {
        Some(image) => egui::Button::image(image),
        None => egui::Button::new(tooltip),
    };
    let response = ui.add(
        button
            .selected(selected)
            .min_size(Vec2::splat(side))
            .sense(Sense::click()),
    );
    let enabled = response.enabled();
    response
        .widget_info(|| egui::WidgetInfo::selected(egui::Role::Button, enabled, selected, tooltip));
    response.on_hover_text(tooltip)
}
