//! Minimal G-code parser for streaming: yields clean command lines

/// Parse G-code text into an iterator of lines suitable for sending to device.
pub fn parse_lines(input: &str) -> Vec<String> {
    input
        .lines()
        .map(|l| {
            // Strip comments after ';' or '('
            let mut s = l.split(';').next().unwrap_or("").to_string();
            if let Some(idx) = s.find('(') {
                s.truncate(idx);
            }
            s.trim().to_string()
        })
        .filter(|s| !s.is_empty())
        .collect()
}
