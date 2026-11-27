//! The Gerber page's presets: GTK's file is the converter's `GerberParameters` as pretty
//! JSON (with the project folder), so a preset saved by either front end loads in the other.
//!
//! Dependencies: `gcodekit5-camtools` (the parameters), `serde_json`.

use std::path::Path;

use gcodekit5_app::i18n::Translator;
use gcodekit5_camtools::gerber::GerberParameters;

use super::GerberParams;
use crate::panes::cam_tools::common;

impl GerberParams {
    /// The page's input as GTK's preset file.
    ///
    /// # Arguments
    /// * `tr` - Translator for the current UI language.
    ///
    /// # Errors
    /// The first field that does not parse (GTK refused to save then too).
    pub fn preset_json(&self, tr: &Translator) -> Result<String, String> {
        common::preset_text(self.build(false).map_err(common::first_error), tr)
    }

    /// Sets the page from a preset file in GTK's format ([`Self::preset_json`]); the preset's
    /// project folder is taken (and its layers found again) when it still exists, as in GTK;
    /// the axis count is the device's, not the file's.
    ///
    /// # Arguments
    /// * `json` - The file's text.
    ///
    /// # Errors
    /// The text is not a Gerber preset; the page is left as it was.
    pub fn apply_preset(&mut self, json: &str) -> Result<(), String> {
        let p: GerberParameters = serde_json::from_str(json).map_err(|e| e.to_string())?;
        let units = self.units;
        let len = |mm: f32| common::format_len(f64::from(mm), units);
        self.layer_type = p.layer_type;
        self.feed_rate = p.feed_rate.to_string();
        self.spindle_speed = p.spindle_speed.to_string();
        self.board_width = len(p.board_width);
        self.board_height = len(p.board_height);
        self.offset_x = len(p.offset_x);
        self.offset_y = len(p.offset_y);
        self.generate_alignment_holes = p.generate_alignment_holes;
        self.alignment_hole_diameter = len(p.alignment_hole_diameter);
        self.alignment_hole_margin = len(p.alignment_hole_margin);
        self.alignment_hole_depth = len(p.alignment_hole_depth);
        self.cut_depth = len(p.cut_depth);
        self.outline_step_down = len(p.outline_step_down);
        self.safe_z = len(p.safe_z);
        self.tool_diameter = len(p.tool_diameter);
        self.isolation_width = len(p.isolation_width);
        self.rubout = p.rubout;
        self.use_board_outline = p.use_board_outline;
        if let Some(directory) = p.directory.as_deref().map(Path::new)
            && directory.exists()
        {
            self.set_directory(directory);
        }
        Ok(())
    }
}
