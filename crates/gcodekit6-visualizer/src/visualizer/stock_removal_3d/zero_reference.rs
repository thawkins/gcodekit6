//! Stock origin conventions ([`StockZeroReference`], [`StockXyReference`])
//! and detection of the Z0 convention recorded in a program header.

/// Which face of the stock block the program's work Z0 is set on.
///
/// Most CAM output zeroes Z on the top of the stock, so the material occupies
/// `-thickness ..= 0` and cuts go below Z0. GCodeKit's own designer (CNC mode)
/// zeroes Z on the bottom of the stock, so the material occupies
/// `0 ..= thickness`. The convention cannot be told reliably from the toolpath
/// itself (a top-zeroed program that only feeds at Z0, or air-cuts above the
/// stock, looks like a bottom-zeroed one), so it is a user setting.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum StockZeroReference {
    /// Work Z0 is the top face of the stock (default; most CAM output).
    #[default]
    Top,
    /// Work Z0 is the bottom face of the stock (machine bed / spoilboard).
    Bottom,
}

impl StockZeroReference {
    /// Work Z (mm) of the stock top face for this convention.
    ///
    /// # Arguments
    /// * `thickness` - Stock thickness in mm.
    ///
    /// # Returns
    /// `0.0` for [`StockZeroReference::Top`], `thickness` for
    /// [`StockZeroReference::Bottom`].
    pub fn stock_top_z(self, thickness: f32) -> f32 {
        match self {
            Self::Top => 0.0,
            Self::Bottom => thickness,
        }
    }

    /// Stable index of this variant, for UI selectors (`Top` = 0, `Bottom` = 1).
    pub fn index(self) -> u32 {
        match self {
            Self::Top => 0,
            Self::Bottom => 1,
        }
    }

    /// Variant for a UI selector index; anything other than 1 maps to `Top`.
    ///
    /// # Arguments
    /// * `index` - Selector index as returned by [`StockZeroReference::index`].
    pub fn from_index(index: u32) -> Self {
        if index == 1 { Self::Bottom } else { Self::Top }
    }
}

/// Where on the stock the program's work X0 Y0 is set.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum StockXyReference {
    /// Work X0 Y0 is the stock's lower-left corner (default).
    #[default]
    LowerLeft,
    /// Work X0 Y0 is the centre of the stock.
    Centre,
}

impl StockXyReference {
    /// Work XY (mm) of the stock's lower-left corner for this convention.
    ///
    /// # Arguments
    /// * `width`, `height` - Stock size in mm.
    pub fn stock_origin_xy(self, width: f32, height: f32) -> (f32, f32) {
        match self {
            Self::LowerLeft => (0.0, 0.0),
            Self::Centre => (-width / 2.0, -height / 2.0),
        }
    }

    /// Stable index of this variant, for UI selectors (`LowerLeft` = 0,
    /// `Centre` = 1).
    pub fn index(self) -> u32 {
        match self {
            Self::LowerLeft => 0,
            Self::Centre => 1,
        }
    }

    /// Variant for a UI selector index; anything other than 1 maps to
    /// `LowerLeft`.
    pub fn from_index(index: u32) -> Self {
        if index == 1 {
            Self::Centre
        } else {
            Self::LowerLeft
        }
    }
}

/// Header comment emitted by the designer for CNC (non-laser) programs, which
/// are always zeroed on the stock bottom.
const DESIGNER_CNC_HEADER: &str = "Generated G-code from Designer tool";

/// Prefix of an explicit stock-origin marker comment, e.g. `; Stock Z0: top`.
const STOCK_Z0_MARKER: &str = "stock z0:";

/// Read the stock Z0 convention recorded in a program's leading comments.
///
/// Only the header (the comment and blank lines before the first command) is
/// examined, and only explicit records are trusted; the toolpath's Z values are
/// never used. Recognised records:
/// * `; Stock Z0: top` / `; Stock Z0: bottom` (also in `( ... )` comments).
/// * The designer's CNC header `; Generated G-code from Designer tool`, whose
///   programs are zeroed on the stock bottom. This is only a fallback: an
///   explicit `Stock Z0` marker anywhere in the header takes precedence.
///
/// # Arguments
/// * `gcode` - Full program text.
///
/// # Returns
/// The recorded convention, or `None` when the program does not record one
/// (the caller should then keep the user's setting).
pub fn detect_stock_zero_reference(gcode: &str) -> Option<StockZeroReference> {
    // An explicit `Stock Z0` marker anywhere in the header wins; the designer's
    // title line is only a fallback, so an edited or re-zeroed program that
    // records its own origin is honoured.
    let mut designer_fallback = None;
    for line in gcode.lines() {
        let line = line.trim();
        if line.is_empty() || line == "%" {
            continue;
        }
        let comment = if let Some(rest) = line.strip_prefix(';') {
            rest
        } else if let Some(rest) = line.strip_prefix('(') {
            rest.strip_suffix(')').unwrap_or(rest)
        } else {
            // A non-comment line is the first command: the header is over.
            break;
        };
        let comment = comment.trim();
        if comment.contains(DESIGNER_CNC_HEADER) {
            designer_fallback = Some(StockZeroReference::Bottom);
            continue;
        }
        let lower = comment.to_ascii_lowercase();
        if let Some(value) = lower.strip_prefix(STOCK_Z0_MARKER) {
            match value.trim() {
                "top" => return Some(StockZeroReference::Top),
                "bottom" => return Some(StockZeroReference::Bottom),
                other => {
                    tracing::warn!(value = other, "Unrecognised stock Z0 marker value");
                }
            }
        }
    }
    designer_fallback
}
