//! The Speeds and Feeds calculator: a tool and a material from the libraries, the cutting
//! parameters, and the active device's spindle give the spindle speed, feed, surface speed,
//! chip load, removal rate, power and deflection.
//!
//! The calculation is GTK's: `SpeedsFeedsCalculator` on a device with the active spindle's
//! maximum RPM, the RPM clamped to that maximum and the feed scaled with it, the values
//! formatted as GTK's results panel showed them. GTK's panel defaults: slotting, 3.0 mm
//! depth, 1.0 mm stepover (0 = the operation's default), 25 mm stick-out, coolant as the
//! device has it, conservative on. Lengths are millimetres whatever the display units, as
//! in GTK.
//!
//! GTK warned in a dialog when the device had no spindle or its RPM or power was unset;
//! here that, an unchosen tool or material, and values outside GTK's spin ranges are shown
//! on the page and Calculate waits.
//!
//! Dependencies: `egui`, `gcodekit5-camtools` (calculator), `gcodekit5-core` (tool and
//! material libraries), `gcodekit5-devicedb` (device profile), `gcodekit5-app`
//! (translations).

mod page;

pub use page::show;

use gcodekit5_camtools::speeds_feeds::{CalculationInput, OperationType, SpeedsFeedsCalculator};
use gcodekit5_core::data::materials::{Material, MaterialId};
use gcodekit5_core::data::tools::{Tool, ToolId};
use gcodekit5_devicedb::model::DeviceProfile;

use super::common::{self, FieldError, check, parse_number};

/// Field label msgids.
pub(super) const TOOL: &str = n_!("Tool:");
pub(super) const MATERIAL: &str = n_!("Material:");
pub(super) const DEPTH: &str = n_!("Depth of Cut (mm):");
pub(super) const STEPOVER: &str = n_!("Stepover (mm):");
pub(super) const STICK_OUT: &str = n_!("Tool Stick-out (mm):");
/// Message msgids.
pub(super) const NO_TOOL: &str = n_!("No tool selected");
pub(super) const NO_MATERIAL: &str = n_!("No material selected");
const NO_SPINDLE: &str = n_!(
    "The active device does not have a spindle configured. Please configure a spindle in the Device Manager."
);
const SPINDLE_UNSET: &str = n_!("Set the spindle's maximum RPM and power in the Device Manager.");

/// The msgids the calculator shows, for the translation check.
pub fn msgids() -> Vec<&'static str> {
    let mut ids = vec![
        TOOL,
        MATERIAL,
        DEPTH,
        STEPOVER,
        STICK_OUT,
        NO_TOOL,
        NO_MATERIAL,
        NO_SPINDLE,
        SPINDLE_UNSET,
    ];
    ids.extend(page::msgids());
    ids
}

/// The calculated values as the results panel shows them (GTK's formatting).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SpeedsFeedsResults {
    /// Spindle speed (RPM), with the unclamped value when it was clamped.
    pub spindle_speed: String,
    /// Feed rate (mm/min), marked when it was scaled with a clamped RPM.
    pub feed_rate: String,
    /// Surface speed.
    pub surface_speed: String,
    /// Chip load (mm/tooth).
    pub chip_load: String,
    /// Material removal rate (mm3/min).
    pub mrr: String,
    /// Power (W).
    pub power: String,
    /// Estimated deflection (mm).
    pub deflection: String,
}

/// Which library the selection dialog lists.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum PickerKind {
    /// The tool library.
    Tool,
    /// The material library.
    Material,
}

/// The open selection dialog.
#[derive(Clone, Debug, PartialEq)]
pub(super) struct Picker {
    /// The library it lists.
    pub(super) kind: PickerKind,
    /// The search text.
    pub(super) search: String,
    /// The row chosen, as an index into the library.
    pub(super) chosen: Option<usize>,
}

/// The calculator's input and its last results.
#[derive(Clone, Debug)]
pub struct SpeedsFeedsParams {
    /// The tool library, in GTK's order (numbered names first).
    pub tools: Vec<Tool>,
    /// The material library, in GTK's order.
    pub materials: Vec<Material>,
    /// The active device profile, if any.
    pub device: Option<DeviceProfile>,
    /// The chosen tool.
    pub tool: Option<Tool>,
    /// The chosen material.
    pub material: Option<Material>,
    /// The operation.
    pub operation: OperationType,
    /// Depth of cut (mm), 0.1 to 50.
    pub depth_of_cut: String,
    /// Stepover (mm), 0 to 100; 0 uses the operation's default.
    pub stepover: String,
    /// Tool stick-out (mm), 5 to 100.
    pub stick_out: String,
    /// Coolant on (only offered when the device has coolant).
    pub coolant: bool,
    /// Conservative calculation (20 % lower values).
    pub conservative: bool,
    /// The last results, with the input they were calculated for ([`Self::input_key`]).
    results: Option<(String, SpeedsFeedsResults)>,
    /// The open selection dialog.
    pub(super) picker: Option<Picker>,
}

/// GTK's order: names that start with a number first, by that number, then by name.
fn gtk_order(a: &str, b: &str) -> std::cmp::Ordering {
    let number = |name: &str| {
        name.chars()
            .take_while(char::is_ascii_digit)
            .collect::<String>()
            .parse::<u32>()
            .ok()
    };
    match (number(a), number(b)) {
        (Some(x), Some(y)) => x.cmp(&y).then_with(|| a.cmp(b)),
        (Some(_), None) => std::cmp::Ordering::Less,
        (None, Some(_)) => std::cmp::Ordering::Greater,
        (None, None) => a.cmp(b),
    }
}

impl SpeedsFeedsParams {
    /// GTK's panel defaults over the libraries and the active device.
    ///
    /// # Arguments
    /// * `tools` - The tool library.
    /// * `materials` - The material library.
    /// * `device` - The active device profile, if any.
    pub fn new(
        mut tools: Vec<Tool>,
        mut materials: Vec<Material>,
        device: Option<DeviceProfile>,
    ) -> Self {
        tools.sort_by(|a, b| gtk_order(&a.name, &b.name));
        materials.sort_by(|a, b| gtk_order(&a.name, &b.name));
        let coolant = device.as_ref().is_some_and(|d| d.has_coolant);
        Self {
            tools,
            materials,
            device,
            tool: None,
            material: None,
            operation: OperationType::Slotting,
            depth_of_cut: "3.0".into(),
            stepover: "1.0".into(),
            stick_out: "25".into(),
            coolant,
            conservative: true,
            results: None,
            picker: None,
        }
    }

    /// Back to the panel defaults (GTK's Reset), keeping the libraries and the device.
    pub fn reset(&mut self) {
        *self = Self::new(
            std::mem::take(&mut self.tools),
            std::mem::take(&mut self.materials),
            self.device.take(),
        );
    }

    /// Chooses the library tool `id`.
    ///
    /// # Returns
    /// Whether the library has it.
    pub fn select_tool(&mut self, id: &ToolId) -> bool {
        self.tool = self.tools.iter().find(|t| &t.id == id).cloned();
        self.tool.is_some()
    }

    /// Chooses the library material `id`.
    ///
    /// # Returns
    /// Whether the library has it.
    pub fn select_material(&mut self, id: &MaterialId) -> bool {
        self.material = self.materials.iter().find(|m| &m.id == id).cloned();
        self.material.is_some()
    }

    /// The last results, while the input is still the one they were calculated for.
    pub fn results(&self) -> Option<&SpeedsFeedsResults> {
        self.results
            .as_ref()
            .filter(|(key, _)| *key == self.input_key())
            .map(|(_, results)| results)
    }

    /// Keeps `results` as the results of the current input.
    ///
    /// # Arguments
    /// * `results` - The results of [`SpeedsFeedsParams::calculate`].
    pub fn store_results(&mut self, results: SpeedsFeedsResults) {
        self.results = Some((self.input_key(), results));
    }

    /// Everything the calculation reads, to tell when results are out of date.
    fn input_key(&self) -> String {
        let device = self.device.as_ref().map(|d| {
            (
                d.has_spindle,
                d.max_spindle_speed_rpm,
                d.cnc_spindle_watts.to_bits(),
            )
        });
        format!(
            "{:?}",
            (
                self.tool.as_ref().map(|t| (&t.id, t.diameter.to_bits())),
                self.material.as_ref().map(|m| &m.id),
                self.operation,
                &self.depth_of_cut,
                &self.stepover,
                &self.stick_out,
                self.coolant,
                self.conservative,
                device,
            )
        )
    }

    /// Sets the open selection dialog's search text.
    ///
    /// # Arguments
    /// * `text` - The search text.
    pub fn set_picker_search(&mut self, text: &str) {
        if let Some(picker) = self.picker.as_mut() {
            picker.search = text.to_string();
            picker.chosen = None;
        }
    }

    /// The calculator's input, or every problem with it.
    ///
    /// # Errors
    /// No tool or material, a tool without a diameter, values outside GTK's ranges, and a
    /// device without a configured spindle.
    pub fn input(&self) -> Result<CalculationInput, Vec<FieldError>> {
        let mut errors = Vec::new();
        let depth = check(
            &mut errors,
            DEPTH,
            parse_number(&self.depth_of_cut),
            |v| match v {
                v if v <= 0.0 => Some(common::MUST_BE_POSITIVE),
                v if !(0.1..=50.0).contains(&v) => Some(common::OUT_OF_RANGE),
                _ => None,
            },
        );
        let stepover = check(
            &mut errors,
            STEPOVER,
            parse_number(&self.stepover),
            |v| match v {
                v if v < 0.0 => Some(common::MUST_NOT_BE_NEGATIVE),
                v if v > 100.0 => Some(common::OUT_OF_RANGE),
                _ => None,
            },
        );
        let stick_out = check(
            &mut errors,
            STICK_OUT,
            parse_number(&self.stick_out),
            |v| match v {
                v if v <= 0.0 => Some(common::MUST_BE_POSITIVE),
                v if !(5.0..=100.0).contains(&v) => Some(common::OUT_OF_RANGE),
                _ => None,
            },
        );
        match &self.tool {
            None => errors.push(FieldError::field(TOOL, NO_TOOL)),
            Some(tool) if tool.diameter.is_nan() || tool.diameter <= 0.0 => {
                errors.push(FieldError::field(TOOL, common::MUST_BE_POSITIVE));
            }
            Some(_) => {}
        }
        if self.material.is_none() {
            errors.push(FieldError::field(MATERIAL, NO_MATERIAL));
        }
        let (max_rpm, has_spindle, spindle_power_kw) = self
            .device
            .as_ref()
            .map(|d| {
                (
                    d.max_spindle_speed_rpm,
                    d.has_spindle,
                    d.cnc_spindle_watts as f32 / 1000.0,
                )
            })
            .unwrap_or((0, false, 0.0));
        if !has_spindle {
            errors.push(FieldError::page(NO_SPINDLE));
        } else if max_rpm == 0 || spindle_power_kw == 0.0 {
            errors.push(FieldError::page(SPINDLE_UNSET));
        }
        let (Some(tool), Some(material), true) =
            (self.tool.clone(), self.material.clone(), errors.is_empty())
        else {
            return Err(errors);
        };
        let stepover = stepover as f32;
        Ok(CalculationInput {
            material,
            tool,
            // GTK's calculation device: the spindle's maximum RPM, a 5000 mm/min feed.
            device: DeviceProfile {
                max_spindle_speed_rpm: max_rpm,
                max_feed_rate: 5000.0,
                ..Default::default()
            },
            operation: self.operation,
            depth_of_cut: depth as f32,
            width_of_cut: (stepover > 0.0).then_some(stepover),
            tool_stick_out: stick_out as f32,
            coolant_enabled: self.coolant,
            conservative: self.conservative,
        })
    }

    /// Calculates, as GTK's Calculate did.
    ///
    /// # Errors
    /// The input's problems ([`SpeedsFeedsParams::input`]).
    pub fn calculate(&self) -> Result<SpeedsFeedsResults, Vec<FieldError>> {
        let input = self.input()?;
        let max_rpm = input.device.max_spindle_speed_rpm;
        let result = SpeedsFeedsCalculator::calculate(&input);
        // Clamp the RPM to the spindle's maximum and scale the feed with it.
        let clamped_rpm = result.rpm.min(max_rpm);
        let rpm_ratio = if result.rpm > 0 {
            clamped_rpm as f32 / result.rpm as f32
        } else {
            1.0
        };
        let (spindle_speed, feed_rate) = if clamped_rpm != result.rpm {
            (
                format!("{} (clamped from {})", clamped_rpm, result.rpm),
                format!("{:.1} (adjusted)", result.feed_rate * rpm_ratio),
            )
        } else {
            (clamped_rpm.to_string(), format!("{:.1}", result.feed_rate))
        };
        Ok(SpeedsFeedsResults {
            spindle_speed,
            feed_rate,
            surface_speed: format!("{:.1}", result.surface_speed_m_min),
            chip_load: format!("{:.4}", result.chip_load_mm),
            mrr: format!("{:.1}", result.material_removal_rate),
            power: format!("{:.0}", result.power_required_kw * 1000.0),
            deflection: format!("{:.4}", result.estimated_deflection_mm),
        })
    }
}
