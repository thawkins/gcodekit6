//! Draws a help topic's blocks: scaled bold headings (as GTK's 1.6/1.4/1.2), wrapped
//! paragraphs and list items with strong, code and link spans, the help images at the
//! width the topic asks for (never wider than the page), and rules.
//!
//! Dependencies: `egui`, `image` (decodes the embedded PNGs).

use std::collections::HashMap;

use super::HelpTopic;
use super::markdown::{Block, LinkTarget, Span};
use crate::resources;

/// Width of one list nesting level, in points.
const INDENT: i8 = 16;

/// Draws `blocks`; returns the topic of a help link clicked this frame.
///
/// # Arguments
/// * `ui` - The page.
/// * `blocks` - The topic's blocks.
/// * `textures` - The help images decoded so far, by file name; new ones are added.
pub(super) fn blocks(
    ui: &mut egui::Ui,
    blocks: &[Block],
    textures: &mut HashMap<String, egui::TextureHandle>,
) -> Option<HelpTopic> {
    let body = egui::TextStyle::Body.resolve(ui.style()).size;
    let mut clicked = None;
    for block in blocks {
        let link = match block {
            Block::Heading { level, spans } => {
                ui.add_space(body * 0.5);
                let scale = match level {
                    1 => 1.6,
                    2 => 1.4,
                    3 => 1.2,
                    _ => 1.1,
                };
                let link = paragraph(ui, None, spans, Some(body * scale));
                ui.add_space(body * 0.25);
                link
            }
            Block::Paragraph(spans) => {
                let link = paragraph(ui, None, spans, None);
                ui.add_space(body * 0.5);
                link
            }
            Block::ListItem {
                depth,
                marker,
                spans,
            } => {
                let levels = i8::try_from(*depth + 1).unwrap_or(i8::MAX);
                let margin = egui::Margin {
                    left: INDENT.saturating_mul(levels),
                    ..egui::Margin::ZERO
                };
                let bullet = format!("{} ", marker.as_deref().unwrap_or("\u{2022}"));
                egui::Frame::NONE
                    .inner_margin(margin)
                    .show(ui, |ui| paragraph(ui, Some(&bullet), spans, None))
                    .inner
            }
            Block::Image { src, alt, width } => {
                image(ui, src, alt, *width, textures);
                None
            }
            Block::Rule => {
                ui.separator();
                None
            }
        };
        clicked = clicked.or(link);
    }
    clicked
}

/// Draws `spans` wrapped after a list `bullet`, if any; headings pass their text `size`
/// (drawn strong). Returns the topic of a help link clicked this frame.
fn paragraph(
    ui: &mut egui::Ui,
    bullet: Option<&str>,
    spans: &[Span],
    size: Option<f32>,
) -> Option<HelpTopic> {
    let style = |text: &str| {
        let text = egui::RichText::new(text);
        match size {
            Some(size) => text.size(size).strong(),
            None => text,
        }
    };
    ui.horizontal_wrapped(|ui| {
        ui.spacing_mut().item_spacing.x = 0.0;
        if let Some(bullet) = bullet {
            ui.label(bullet);
        }
        let mut clicked = None;
        for span in spans {
            match span {
                Span::Text(text) => {
                    ui.label(style(text));
                }
                Span::Strong(text) => {
                    ui.label(style(text).strong());
                }
                Span::Code(text) => {
                    ui.label(style(text).code());
                }
                Span::Link {
                    text,
                    target: LinkTarget::External(url),
                } => {
                    ui.hyperlink_to(style(text), url);
                }
                Span::Link {
                    text,
                    target: LinkTarget::Topic(id),
                } => {
                    if ui.link(style(text)).clicked() {
                        match HelpTopic::from_id(id) {
                            Some(topic) => clicked = Some(topic),
                            None => tracing::warn!(id, "help link to an unknown topic"),
                        }
                    }
                }
            }
        }
        clicked
    })
    .inner
}

/// Draws help image `src` at `width` (or the page width when narrower), keeping its
/// aspect ratio; decodes it on first use.
fn image(
    ui: &mut egui::Ui,
    src: &str,
    alt: &str,
    width: f32,
    textures: &mut HashMap<String, egui::TextureHandle>,
) {
    if !textures.contains_key(src) {
        let Some(texture) = load(ui.ctx(), src) else {
            return;
        };
        textures.insert(src.to_string(), texture);
    }
    let Some(texture) = textures.get(src) else {
        return;
    };
    let [w, h] = texture.size().map(|n| n as f32);
    if w <= 0.0 {
        return;
    }
    let shown = width.min(ui.available_width()).max(1.0);
    let size = egui::vec2(shown, shown * h / w);
    ui.add(egui::Image::new((texture.id(), size)).alt_text(alt));
}

/// Decodes embedded help image `src` into a texture.
fn load(ctx: &egui::Context, src: &str) -> Option<egui::TextureHandle> {
    let Some(bytes) = resources::help_image(src) else {
        tracing::warn!(src, "help image not embedded");
        return None;
    };
    match image::load_from_memory(bytes) {
        Ok(decoded) => {
            let rgba = decoded.to_rgba8();
            let size = [rgba.width() as usize, rgba.height() as usize];
            let pixels = egui::ColorImage::from_rgba_unmultiplied(size, rgba.as_raw());
            Some(ctx.load_texture(
                format!("help-image-{src}"),
                pixels,
                egui::TextureOptions::LINEAR,
            ))
        }
        Err(e) => {
            tracing::warn!(src, error = %e, "help image could not be decoded");
            None
        }
    }
}
