//! Find in the editor's text: the next or previous match of a query, wrapping around, and
//! the number of matches. Matching ignores ASCII case (G-code is ASCII; `x10` finds `X10`),
//! as the GTK editor's search did by default.
//!
//! Ranges are byte ranges into the text. ASCII case folding keeps every byte where it is, so
//! a match in the folded text is the same range in the original.
//!
//! Dependencies: none.

use std::ops::Range;

/// Every match of `needle` in `haystack`, case-insensitively, as byte offsets of its start.
fn starts<'a>(haystack: &'a str, needle: &'a str) -> impl Iterator<Item = usize> + 'a {
    let needle = needle.as_bytes();
    let hay = haystack.as_bytes();
    let first = needle.first().map(u8::to_ascii_lowercase);
    (0..hay.len().saturating_sub(needle.len().saturating_sub(1))).filter(move |&i| {
        first.is_some_and(|f| hay[i].to_ascii_lowercase() == f)
            && hay[i..i + needle.len()].eq_ignore_ascii_case(needle)
            && haystack.is_char_boundary(i)
    })
}

/// The first match of `needle` at or after byte `from`, else the first in the text.
///
/// # Arguments
/// * `text` - The text to search.
/// * `needle` - The query; an empty one finds nothing.
/// * `from` - Byte offset to search from.
///
/// # Returns
/// The match's byte range, or `None` when there is none.
pub fn find_next(text: &str, needle: &str, from: usize) -> Option<Range<usize>> {
    let mut all = starts(text, needle);
    let start = all
        .by_ref()
        .find(|&i| i >= from)
        .or_else(|| starts(text, needle).next())?;
    Some(start..start + needle.len())
}

/// The last match of `needle` that starts before byte `before`, else the last in the text.
///
/// # Arguments
/// * `text` - The text to search.
/// * `needle` - The query; an empty one finds nothing.
/// * `before` - Byte offset the match must start before.
///
/// # Returns
/// The match's byte range, or `None` when there is none.
pub fn find_previous(text: &str, needle: &str, before: usize) -> Option<Range<usize>> {
    let mut last_before = None;
    let mut last = None;
    for i in starts(text, needle) {
        if i < before {
            last_before = Some(i);
        }
        last = Some(i);
    }
    let start = last_before.or(last)?;
    Some(start..start + needle.len())
}

/// How many times `needle` occurs in `text` (overlapping matches count).
///
/// # Arguments
/// * `text` - The text to search.
/// * `needle` - The query; an empty one occurs nowhere.
pub fn count(text: &str, needle: &str) -> usize {
    starts(text, needle).count()
}
