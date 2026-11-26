//! The help topics' markdown, read into blocks the help window draws.
//!
//! The topics are not strict CommonMark: they mix markdown with HTML (`<img width>`, `<li>`,
//! `<p>`) and write sub-items as `-     [x](help:y)`, which CommonMark reads as indented code.
//! So, like the GTK help browser, this reads them line by line: ATX headings, `-`/`*`/`+`
//! and numbered list items (nested by indentation), `<li>` items, `<img>` tags, `---` rules
//! and paragraphs (consecutive lines joined). Inside a line it reads `**strong**`,
//! `` `code` ``, `[text](help:topic)` and `[text](https://...)` links and bare `http(s)://`
//! URLs; other HTML tags are dropped.
//!
//! Dependencies: none.

/// Where a link goes.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LinkTarget {
    /// Another help topic, by id (`help:<id>`).
    Topic(String),
    /// A web address, opened in the browser.
    External(String),
}

/// A run of text inside a block.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Span {
    /// Plain text.
    Text(String),
    /// `**strong**` text.
    Strong(String),
    /// `` `code` `` text.
    Code(String),
    /// A link.
    Link {
        /// The text shown.
        text: String,
        /// Where it goes.
        target: LinkTarget,
    },
}

/// A block of a help topic.
#[derive(Debug, Clone, PartialEq)]
pub enum Block {
    /// A heading, `level` 1 to 6.
    Heading {
        /// The number of `#`.
        level: u8,
        /// The heading text.
        spans: Vec<Span>,
    },
    /// A paragraph.
    Paragraph(Vec<Span>),
    /// A list item.
    ListItem {
        /// The nesting depth, 0 for a top-level item.
        depth: usize,
        /// The number of a numbered item (`"1."`); `None` for a bullet.
        marker: Option<String>,
        /// The item text.
        spans: Vec<Span>,
    },
    /// An image.
    Image {
        /// The help image file name (`"designer.png"`).
        src: String,
        /// The alt text; the file name when the tag has none.
        alt: String,
        /// The width the topic asks for, in points.
        width: f32,
    },
    /// A horizontal rule.
    Rule,
}

/// The width an `<img>` without a `width` gets (as in GTK).
const DEFAULT_IMAGE_WIDTH: f32 = 300.0;

/// The deepest list nesting kept.
const MAX_DEPTH: usize = 4;

/// Reads a help topic into blocks.
///
/// # Arguments
/// * `md` - The topic's markdown.
pub fn parse(md: &str) -> Vec<Block> {
    let mut blocks = Vec::new();
    let mut paragraph = String::new();
    for line in md.lines() {
        let text = line.trim();
        if text.is_empty() {
            flush(&mut paragraph, &mut blocks);
            continue;
        }
        if let Some(block) = heading(text)
            .or_else(|| (text == "---" || text == "***").then_some(Block::Rule))
            .or_else(|| list_item(line))
        {
            flush(&mut paragraph, &mut blocks);
            blocks.push(block);
            continue;
        }
        if text.contains("<img") {
            flush(&mut paragraph, &mut blocks);
            blocks.extend(images(text));
            continue;
        }
        if text.starts_with("<li") {
            flush(&mut paragraph, &mut blocks);
            let spans = inlines(&strip_tags(text));
            if !spans.is_empty() {
                blocks.push(Block::ListItem {
                    depth: 0,
                    marker: None,
                    spans,
                });
            }
            continue;
        }
        if text.starts_with('<') && strip_tags(text).trim().is_empty() {
            flush(&mut paragraph, &mut blocks);
            continue;
        }
        if !paragraph.is_empty() {
            paragraph.push(' ');
        }
        paragraph.push_str(text);
    }
    flush(&mut paragraph, &mut blocks);
    blocks
}

/// Ends the paragraph being collected, if any.
fn flush(paragraph: &mut String, blocks: &mut Vec<Block>) {
    if !paragraph.is_empty() {
        let spans = inlines(paragraph);
        if !spans.is_empty() {
            blocks.push(Block::Paragraph(spans));
        }
        paragraph.clear();
    }
}

/// An ATX heading (`# ` to `###### `).
fn heading(text: &str) -> Option<Block> {
    let hashes = text.bytes().take_while(|&b| b == b'#').count();
    let rest = text.get(hashes..)?;
    if !(1..=6).contains(&hashes) || !rest.starts_with(' ') {
        return None;
    }
    Some(Block::Heading {
        level: u8::try_from(hashes).ok()?,
        spans: inlines(rest.trim().trim_end_matches('#').trim_end()),
    })
}

/// A `-`, `*`, `+` or numbered list item; the depth comes from the indentation (two
/// spaces a level), plus one when the text after the marker is itself indented.
fn list_item(line: &str) -> Option<Block> {
    let indent: usize = line
        .chars()
        .take_while(|c| c.is_whitespace())
        .map(|c| if c == '\t' { 4 } else { 1 })
        .sum();
    let text = line.trim_start();
    let (marker, rest) =
        if let Some(rest) = ["- ", "* ", "+ "].iter().find_map(|m| text.strip_prefix(m)) {
            (None, rest)
        } else {
            let digits = text.bytes().take_while(u8::is_ascii_digit).count();
            let rest = text.get(digits..)?.strip_prefix(". ")?;
            if digits == 0 {
                return None;
            }
            (Some(format!("{}.", &text[..digits])), rest)
        };
    // Sub-items are written with the text pushed right of the marker ("-     x").
    let nested = rest.chars().take_while(|c| c.is_whitespace()).count() >= 3;
    // The index writes sub-items as "-     \u{25aa}\u{fe0f} [x](...)"; the indentation
    // already shows them as nested, so the square glyph is dropped.
    let rest = rest
        .trim()
        .trim_start_matches(['\u{25aa}', '\u{fe0f}'])
        .trim_start();
    let depth = (indent / 2 + usize::from(nested)).min(MAX_DEPTH);
    Some(Block::ListItem {
        depth,
        marker,
        spans: inlines(&strip_tags(rest)),
    })
}

/// The `<img>` tags of a line.
fn images(text: &str) -> Vec<Block> {
    text.split("<img")
        .skip(1)
        .filter_map(|tag| {
            let tag = tag.split('>').next().unwrap_or(tag);
            let src = image_name(&attribute(tag, "src")?);
            let width = attribute(tag, "width")
                .and_then(|w| w.parse::<f32>().ok())
                .filter(|w| *w > 0.0)
                .unwrap_or(DEFAULT_IMAGE_WIDTH);
            let alt = attribute(tag, "alt").unwrap_or_else(|| src.clone());
            Some(Block::Image { src, alt, width })
        })
        .collect()
}

/// The value of `name="value"` in an HTML tag.
fn attribute(tag: &str, name: &str) -> Option<String> {
    let start = tag.find(&format!("{name}=\""))? + name.len() + 2;
    let len = tag.get(start..)?.find('"')?;
    Some(tag[start..start + len].to_string())
}

/// The help image file name an `<img src>` names: the last path segment, with `.png`
/// added when it has no extension (as GTK resolved them).
pub fn image_name(src: &str) -> String {
    let name = src.rsplit('/').next().unwrap_or(src);
    if name.contains('.') {
        name.to_string()
    } else {
        format!("{name}.png")
    }
}

/// `text` without its HTML tags.
fn strip_tags(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let mut in_tag = false;
    for c in text.chars() {
        match c {
            '<' => in_tag = true,
            '>' if in_tag => in_tag = false,
            _ if !in_tag => out.push(c),
            _ => {}
        }
    }
    out
}

/// Reads the spans of one block's text.
fn inlines(text: &str) -> Vec<Span> {
    let mut spans = Vec::new();
    let mut plain = String::new();
    let mut rest = text;
    while let Some(c) = rest.chars().next() {
        if let Some((span, after)) = inline_at(rest) {
            if !plain.is_empty() {
                spans.push(Span::Text(std::mem::take(&mut plain)));
            }
            spans.push(span);
            rest = after;
            continue;
        }
        if c == '\\'
            && let Some(escaped) = rest[1..].chars().next()
            && escaped.is_ascii_punctuation()
        {
            plain.push(escaped);
            rest = &rest[1 + escaped.len_utf8()..];
            continue;
        }
        if c == '<'
            && let Some(end) = rest.find('>')
        {
            rest = &rest[end + 1..];
            continue;
        }
        plain.push(c);
        rest = &rest[c.len_utf8()..];
    }
    if !plain.is_empty() {
        spans.push(Span::Text(plain));
    }
    spans
}

/// A strong, code or link span starting at the beginning of `text`, and the text after it.
fn inline_at(text: &str) -> Option<(Span, &str)> {
    if let Some(rest) = text.strip_prefix("**") {
        let end = rest.find("**").filter(|&end| end > 0)?;
        return Some((Span::Strong(strip_tags(&rest[..end])), &rest[end + 2..]));
    }
    if let Some(rest) = text.strip_prefix('`') {
        let end = rest.find('`').filter(|&end| end > 0)?;
        return Some((Span::Code(rest[..end].to_string()), &rest[end + 1..]));
    }
    if let Some(rest) = text.strip_prefix('[') {
        let close = rest.find("](")?;
        let label = &rest[..close];
        let after = &rest[close + 2..];
        let end = after.find(')')?;
        let target = link_target(after[..end].trim())?;
        let text = inlines(label)
            .into_iter()
            .map(|span| match span {
                Span::Text(t) | Span::Strong(t) | Span::Code(t) => t,
                Span::Link { text, .. } => text,
            })
            .collect();
        return Some((Span::Link { text, target }, &after[end + 1..]));
    }
    if text.starts_with("https://") || text.starts_with("http://") {
        let end = text
            .find(|c: char| c.is_whitespace() || c == '<' || c == ')')
            .unwrap_or(text.len());
        let url = text[..end].trim_end_matches(['.', ',', ';', ':', '!', '?']);
        return Some((
            Span::Link {
                text: url.to_string(),
                target: LinkTarget::External(url.to_string()),
            },
            &text[url.len()..],
        ));
    }
    None
}

/// What a `[text](target)` link goes to; `None` for a target the help cannot follow.
fn link_target(target: &str) -> Option<LinkTarget> {
    if let Some(topic) = target.strip_prefix("help:") {
        Some(LinkTarget::Topic(topic.trim().to_string()))
    } else if target.starts_with("https://")
        || target.starts_with("http://")
        || target.starts_with("mailto:")
    {
        Some(LinkTarget::External(target.to_string()))
    } else {
        None
    }
}
