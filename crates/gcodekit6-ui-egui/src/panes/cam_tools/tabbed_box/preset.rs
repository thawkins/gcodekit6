//! The Tabbed Box page's presets: the file is the generator's `BoxParameters` as pretty
//! JSON, so presets saved by earlier releases still load.
//!
//! Dependencies: `gcodekit6-camtools` (the parameters), `serde_json`.

use gcodekit6_app::i18n::Translator;
use gcodekit6_camtools::tabbed_box::BoxParameters;

use super::TabbedBoxParams;
use crate::panes::cam_tools::common;

impl TabbedBoxParams {
    /// The page's input as a preset file.
    ///
    /// # Arguments
    /// * `tr` - Translator for the current UI language.
    ///
    /// # Errors
    /// The first field that does not parse (saving is refused then).
    pub fn preset_json(&self, tr: &Translator) -> Result<String, String> {
        common::preset_text(self.build(false).map_err(common::first_error), tr)
    }

    /// Sets the page from a preset file ([`Self::preset_json`]); the axis
    /// count is the device's, not the file's.
    ///
    /// # Arguments
    /// * `json` - The file's text.
    ///
    /// # Errors
    /// The text is not a box preset; the page is left as it was.
    pub fn apply_preset(&mut self, json: &str) -> Result<(), String> {
        let p: BoxParameters = serde_json::from_str(json).map_err(|e| e.to_string())?;
        let units = self.units;
        let len = |mm: f64| common::format_len(mm, units);
        self.width = len(p.x);
        self.depth = len(p.y);
        self.height = len(p.h);
        self.outside = p.outside;
        self.thickness = len(p.thickness);
        self.burn = len(p.burn);
        self.finger_width = p.finger_joint.finger.to_string();
        self.space_width = p.finger_joint.space.to_string();
        self.surrounding_spaces = p.finger_joint.surrounding_spaces.to_string();
        self.play = len(p.finger_joint.play);
        self.extra_length = len(p.finger_joint.extra_length);
        self.finger_style = p.finger_joint.style;
        self.box_type = p.box_type;
        self.dividers_x = p.dividers_x.to_string();
        self.dividers_y = p.dividers_y.to_string();
        self.divider_keying = p.key_divider_type;
        self.optimize_layout = p.optimize_layout;
        self.passes = p.laser_passes.to_string();
        self.power = p.laser_power.to_string();
        self.feed_rate = p.feed_rate.to_string();
        self.z_step_down = len(p.z_step_down);
        self.offset_x = len(p.offset_x);
        self.offset_y = len(p.offset_y);
        self.home_before = p.home_before_job;
        Ok(())
    }
}
