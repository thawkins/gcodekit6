//! Toolkit-free model of the Device Manager, shared by the GTK and egui front ends.
//!
//! - [`ProfileForm`] holds a device profile's fields as the edit form shows them (lengths
//!   and feed in the user's units) and [`ProfileForm::apply`] checks them and writes them
//!   back into a [`DeviceProfile`], with the GTK Device Manager's Save rules and messages.
//! - [`ReportedLimits`] reads the values a `$$` dump gives a profile (GTK's Sync from Device)
//!   and [`store_reported_settings`] keeps a dump in a profile (GTK's Device Config page).
//! - [`matches_search`] is the device list's search, [`delete_refusal`] the delete rule.
//!
//! Profiles are stored by `gcodekit5-devicedb`'s `DeviceManager` (`devices.json`, the GTK
//! build's file). Depends on `gcodekit5-core` (units), `gcodekit5-devicedb` (profile model)
//! and `gcodekit5-communication` (which controllers are supported).

use std::collections::HashMap;

use gcodekit5_core::units::{
    FeedRateUnits, MeasurementSystem, format_feed_rate, format_length, parse_feed_rate,
    parse_length,
};
use gcodekit5_devicedb::{ControllerType, DeviceProfile, DeviceType};

/// The device types the form offers, in GTK's order.
pub const DEVICE_TYPES: [DeviceType; 5] = [
    DeviceType::CncMill,
    DeviceType::CncLathe,
    DeviceType::LaserCutter,
    DeviceType::ThreeDPrinter,
    DeviceType::Plotter,
];

/// The controllers the form offers with their labels (msgids), in GTK's order.
pub const CONTROLLER_CHOICES: [(ControllerType, &str); 8] = [
    (ControllerType::Grbl, "GRBL"),
    (ControllerType::Repetier, "Repetier"),
    (ControllerType::FluidNC, n_!("FluidNC (supported via GRBL)")),
    (ControllerType::GrblHal, n_!("grblHAL (supported via GRBL)")),
    (ControllerType::TinyG, n_!("TinyG (not supported)")),
    (ControllerType::G2Core, n_!("g2core (not supported)")),
    (
        ControllerType::Smoothieware,
        n_!("Smoothieware (not supported)"),
    ),
    (ControllerType::Marlin, n_!("Marlin (not supported)")),
];

/// The connection types the form offers (stored as written; "Serial" is translated).
pub const CONNECTION_TYPES: [&str; 3] = [n_!("Serial"), "TCP/IP", "WebSocket"];

/// The baud rates the form offers.
pub const BAUD_RATES: [u32; 6] = [9600, 19200, 38400, 57600, 115200, 250000];

/// The label (msgid) of a device type, as GTK lists it.
///
/// # Arguments
/// * `device_type` - The device type.
pub fn device_type_label(device_type: &DeviceType) -> &'static str {
    match device_type {
        DeviceType::CncMill => n_!("CNC Mill"),
        DeviceType::CncLathe => n_!("CNC Lathe"),
        DeviceType::LaserCutter => n_!("Laser Cutter"),
        DeviceType::ThreeDPrinter => n_!("3D Printer"),
        DeviceType::Plotter => n_!("Plotter"),
    }
}

/// The device type a GTK device-type label names, if any.
///
/// # Arguments
/// * `label` - An untranslated label from [`device_type_label`].
pub fn device_type_from_label(label: &str) -> Option<DeviceType> {
    DEVICE_TYPES
        .into_iter()
        .find(|t| device_type_label(t) == label)
}

/// The label (msgid) of a controller, as GTK lists it.
///
/// # Arguments
/// * `controller` - The controller type.
pub fn controller_label(controller: &ControllerType) -> &'static str {
    CONTROLLER_CHOICES
        .iter()
        .find(|(c, _)| c == controller)
        .map_or("GRBL", |(_, label)| label)
}

/// Why the form could not be saved: a title and a detail (both msgids), the detail's `{}`
/// filled with [`FormError::arg`] when there is one.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FormError {
    /// The title, naming the field (`"Invalid TCP Port"`).
    pub title: &'static str,
    /// What is wrong (`"TCP Port must be a number"`).
    pub detail: &'static str,
    /// The value for the detail's `{}`, if it has one.
    pub arg: Option<String>,
}

impl FormError {
    const fn new(title: &'static str, detail: &'static str) -> Self {
        Self {
            title,
            detail,
            arg: None,
        }
    }

    /// The detail with its `{}` filled in.
    ///
    /// # Arguments
    /// * `detail` - [`FormError::detail`], translated.
    pub fn fill(&self, detail: &str) -> String {
        match &self.arg {
            Some(arg) => detail.replacen("{}", arg, 1),
            None => detail.to_string(),
        }
    }
}

/// A device profile's fields as the edit form shows them; lengths and feed rate are in the
/// user's units.
#[derive(Debug, Clone, PartialEq)]
pub struct ProfileForm {
    /// Name.
    pub name: String,
    /// Description.
    pub description: String,
    /// Device type.
    pub device_type: DeviceType,
    /// Controller.
    pub controller_type: ControllerType,
    /// One of [`CONNECTION_TYPES`].
    pub connection_type: String,
    /// Serial port.
    pub port: String,
    /// Serial baud rate.
    pub baud_rate: u32,
    /// Network host.
    pub tcp_host: String,
    /// Network port.
    pub tcp_port: String,
    /// Connection timeout in ms.
    pub timeout_ms: String,
    /// Reconnect when the connection drops.
    pub auto_reconnect: bool,
    /// X minimum.
    pub x_min: String,
    /// X maximum.
    pub x_max: String,
    /// Y minimum.
    pub y_min: String,
    /// Y maximum.
    pub y_max: String,
    /// Z minimum.
    pub z_min: String,
    /// Z maximum.
    pub z_max: String,
    /// Has a spindle.
    pub has_spindle: bool,
    /// Has a laser.
    pub has_laser: bool,
    /// Has coolant.
    pub has_coolant: bool,
    /// Repetier pen-plotter mode.
    pub pen_mode: bool,
    /// Number of axes (1 to 6).
    pub num_axes: String,
    /// Maximum feed rate.
    pub max_feed_rate: String,
    /// Maximum S value.
    pub max_s_value: String,
    /// Spindle power in W.
    pub spindle_watts: String,
    /// Maximum spindle speed in RPM.
    pub max_spindle_speed_rpm: String,
    /// Laser power in W.
    pub laser_watts: String,
}

/// Rounds `value` to two decimals, as GTK stores a length.
fn round2(value: f32) -> f64 {
    format!("{value:.2}").parse().unwrap_or(f64::from(value))
}

/// Parses a plain number field.
fn number<T: std::str::FromStr>(text: &str, error: FormError) -> Result<T, FormError> {
    text.trim().parse().map_err(|_| error)
}

/// Parses a length field into mm, rounded to two decimals.
fn length(text: &str, units: MeasurementSystem, title: &'static str) -> Result<f64, FormError> {
    parse_length(text, units)
        .map(round2)
        .map_err(|_| FormError::new(title, n_!("Enter a number")))
}

/// Orders a minimum and maximum (GTK swaps reversed limits).
fn ordered(min: f64, max: f64) -> (f64, f64) {
    if min > max { (max, min) } else { (min, max) }
}

impl ProfileForm {
    /// The form for `profile`, with lengths in `units` and the feed rate in `feed_units`.
    ///
    /// # Arguments
    /// * `profile` - The stored profile.
    /// * `units` - The user's length units.
    /// * `feed_units` - The user's feed rate units.
    pub fn from_profile(
        profile: &DeviceProfile,
        units: MeasurementSystem,
        feed_units: FeedRateUnits,
    ) -> Self {
        let len = |mm: f64| format_length(mm as f32, units);
        Self {
            name: profile.name.clone(),
            description: profile.description.clone(),
            device_type: profile.device_type.clone(),
            controller_type: profile.controller_type.clone(),
            connection_type: profile.connection_type.clone(),
            port: profile.port.clone(),
            baud_rate: profile.baud_rate,
            tcp_host: profile.tcp_host.trim().to_string(),
            tcp_port: profile.tcp_port.to_string(),
            timeout_ms: profile.timeout_ms.to_string(),
            auto_reconnect: profile.auto_reconnect,
            x_min: len(profile.x_axis.min),
            x_max: len(profile.x_axis.max),
            y_min: len(profile.y_axis.min),
            y_max: len(profile.y_axis.max),
            z_min: len(profile.z_axis.min),
            z_max: len(profile.z_axis.max),
            has_spindle: profile.has_spindle,
            has_laser: profile.has_laser,
            has_coolant: profile.has_coolant,
            pen_mode: profile.pen_mode,
            num_axes: profile.num_axes.to_string(),
            max_feed_rate: format_feed_rate(profile.max_feed_rate as f32, feed_units),
            max_s_value: format!("{:.0}", profile.max_s_value),
            spindle_watts: format!("{:.0}", profile.cnc_spindle_watts),
            max_spindle_speed_rpm: profile.max_spindle_speed_rpm.to_string(),
            laser_watts: format!("{:.0}", profile.laser_watts),
        }
    }

    /// Whether the connection is serial (port and baud rate apply) rather than over the
    /// network (host and port apply).
    pub fn is_serial(&self) -> bool {
        self.connection_type != "TCP/IP" && self.connection_type != "WebSocket"
    }

    /// Checks the form and writes it into a copy of `base` (which keeps its id, rotary
    /// axes and stored settings), with the GTK Device Manager's rules: lengths rounded to
    /// 0.01 mm and reversed limits swapped, 1 to 6 axes, spindle and laser values checked
    /// only when present (saved as 0 otherwise), a network connection needs a host without
    /// spaces, and an unsupported controller is refused.
    ///
    /// # Arguments
    /// * `base` - The stored profile.
    /// * `units` - The units the lengths are in.
    /// * `feed_units` - The units the feed rate is in.
    ///
    /// # Errors
    /// The first field that cannot be saved.
    pub fn apply(
        &self,
        base: &DeviceProfile,
        units: MeasurementSystem,
        feed_units: FeedRateUnits,
    ) -> Result<DeviceProfile, FormError> {
        let mut p = base.clone();
        p.name = self.name.clone();
        p.description = self.description.clone();
        p.device_type = self.device_type.clone();
        p.controller_type = self.controller_type.clone();
        p.connection_type = self.connection_type.clone();
        p.port = self.port.clone();
        p.baud_rate = self.baud_rate;
        p.tcp_port = number(
            &self.tcp_port,
            FormError::new(n_!("Invalid TCP Port"), n_!("TCP Port must be a number")),
        )?;
        p.timeout_ms = number::<u32>(
            &self.timeout_ms,
            FormError::new(n_!("Invalid Timeout"), n_!("Timeout must be a number (ms)")),
        )?
        .into();
        let host = self.tcp_host.trim();
        if host.chars().any(char::is_whitespace) {
            return Err(FormError::new(
                n_!("Invalid TCP Host"),
                n_!("TCP Host must not contain spaces"),
            ));
        }
        if host.is_empty() {
            if !self.is_serial() {
                return Err(FormError::new(
                    n_!("Invalid TCP Host"),
                    n_!("TCP Host must not be empty for network connections"),
                ));
            }
            // Serial profiles keep the host stored before.
        } else {
            p.tcp_host = host.to_string();
        }
        p.auto_reconnect = self.auto_reconnect;

        let (x_min, x_max) = ordered(
            length(&self.x_min, units, n_!("Invalid X Min"))?,
            length(&self.x_max, units, n_!("Invalid X Max"))?,
        );
        let (y_min, y_max) = ordered(
            length(&self.y_min, units, n_!("Invalid Y Min"))?,
            length(&self.y_max, units, n_!("Invalid Y Max"))?,
        );
        let (z_min, z_max) = ordered(
            length(&self.z_min, units, n_!("Invalid Z Min"))?,
            length(&self.z_max, units, n_!("Invalid Z Max"))?,
        );
        (p.x_axis.min, p.x_axis.max) = (x_min, x_max);
        (p.y_axis.min, p.y_axis.max) = (y_min, y_max);
        (p.z_axis.min, p.z_axis.max) = (z_min, z_max);

        p.has_spindle = self.has_spindle;
        p.has_laser = self.has_laser;
        p.has_coolant = self.has_coolant;
        p.pen_mode = self.pen_mode;
        p.num_axes = match self.num_axes.trim().parse::<u8>() {
            Ok(n) if (1..=6).contains(&n) => n,
            _ => {
                return Err(FormError::new(
                    n_!("Invalid No of Axes"),
                    n_!("Number of axes must be an integer between 1 and 6"),
                ));
            }
        };
        let feed = parse_feed_rate(&self.max_feed_rate, feed_units)
            .map_err(|_| FormError::new(n_!("Invalid Max Feed Rate"), n_!("Enter a number")))?;
        let max_s: f32 = number(
            &self.max_s_value,
            FormError::new(
                n_!("Invalid Max S-Value"),
                n_!("Max S-Value must be a number"),
            ),
        )?;
        let (spindle_watts, max_rpm) = if self.has_spindle {
            let watts: f32 = number(
                &self.spindle_watts,
                FormError::new(
                    n_!("Invalid Spindle Power"),
                    n_!("Spindle power must be a number (W)"),
                ),
            )?;
            let rpm: u32 = number(
                &self.max_spindle_speed_rpm,
                FormError::new(
                    n_!("Invalid Max Spindle Speed"),
                    n_!("Max spindle speed must be an integer (RPM)"),
                ),
            )?;
            (watts, rpm)
        } else {
            (0.0, 0)
        };
        let laser_watts: f32 = if self.has_laser {
            number(
                &self.laser_watts,
                FormError::new(
                    n_!("Invalid Laser Power"),
                    n_!("Laser power must be a number (W)"),
                ),
            )?
        } else {
            0.0
        };
        // GTK stores these whole.
        p.max_feed_rate = f64::from(feed).round();
        p.max_s_value = f64::from(max_s).round();
        p.cnc_spindle_watts = f64::from(spindle_watts).round();
        p.max_spindle_speed_rpm = max_rpm;
        p.laser_watts = f64::from(laser_watts).round();

        let protocol =
            gcodekit5_communication::firmware::protocol_profile(p.controller_type.as_str());
        if !protocol.is_supported {
            return Err(FormError {
                title: n_!("Unsupported controller"),
                detail: n_!(
                    "The selected controller \"{}\" is not supported yet. Please choose a supported one and try again."
                ),
                arg: Some(protocol.display_name.to_string()),
            });
        }
        Ok(p)
    }

    /// Fills in what a controller reported (GTK's Sync from Device): X/Y/Z maximum from
    /// `$130`-`$132`, maximum spindle speed and S value from `$30` (a spindle when above
    /// 0), the laser from `$32`.
    ///
    /// # Arguments
    /// * `limits` - The reported values.
    /// * `units` - The units the form's lengths are in.
    pub fn sync(&mut self, limits: &ReportedLimits, units: MeasurementSystem) {
        let len = |mm: f64| format_length(mm as f32, units);
        if let Some(x) = limits.x_max {
            self.x_max = len(x);
        }
        if let Some(y) = limits.y_max {
            self.y_max = len(y);
        }
        if let Some(z) = limits.z_max {
            self.z_max = len(z);
        }
        if let Some(rpm) = limits.max_rpm {
            self.max_spindle_speed_rpm = rpm.to_string();
            self.max_s_value = rpm.to_string();
            if rpm > 0 {
                self.has_spindle = true;
            }
        }
        if let Some(laser) = limits.laser {
            self.has_laser = laser;
        }
    }
}

/// What a GRBL `$$` dump says about a machine's profile.
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct ReportedLimits {
    /// X max travel in mm (`$130`).
    pub x_max: Option<f64>,
    /// Y max travel in mm (`$131`).
    pub y_max: Option<f64>,
    /// Z max travel in mm (`$132`).
    pub z_max: Option<f64>,
    /// Maximum spindle speed (`$30`).
    pub max_rpm: Option<u32>,
    /// Laser mode (`$32=1`).
    pub laser: Option<bool>,
}

impl ReportedLimits {
    /// Reads the values from a dump (setting number to reported value).
    ///
    /// # Arguments
    /// * `settings` - The reported settings.
    pub fn from_settings(settings: &HashMap<u16, String>) -> Self {
        let float = |n: u16| settings.get(&n).and_then(|v| v.trim().parse::<f64>().ok());
        let whole = |n: u16| settings.get(&n).and_then(|v| v.trim().parse::<u32>().ok());
        Self {
            x_max: float(130),
            y_max: float(131),
            z_max: float(132),
            max_rpm: whole(30),
            laser: whole(32).map(|v| v == 1),
        }
    }
}

/// Keeps a reported `$$` dump in `profile`, with its maximum S value (`$30`) and laser mode
/// (`$32` at least 1), as GTK's Device Config page does.
///
/// # Arguments
/// * `profile` - The profile to update.
/// * `settings` - The reported settings.
pub fn store_reported_settings(profile: &mut DeviceProfile, settings: &HashMap<u16, String>) {
    profile.grbl_settings = settings.clone();
    let numeric = |n: u16| {
        settings
            .get(&n)
            .and_then(|v| crate::device_status::parse_numeric_prefix(v))
    };
    if let Some(max_s) = numeric(30) {
        profile.max_s_value = max_s;
    }
    if let Some(laser) = numeric(32) {
        profile.has_laser = laser >= 1.0;
    }
}

/// Whether `profile` matches the device list's search text (name, description, device
/// type, controller or connection type, ignoring case).
///
/// # Arguments
/// * `profile` - The profile.
/// * `query` - The search text; empty matches everything.
pub fn matches_search(profile: &DeviceProfile, query: &str) -> bool {
    let query = query.trim().to_lowercase();
    if query.is_empty() {
        return true;
    }
    format!(
        "{}\n{}\n{}\n{}\n{}",
        profile.name,
        profile.description,
        profile.device_type,
        profile.controller_type,
        profile.connection_type
    )
    .to_lowercase()
    .contains(&query)
}

/// Why a profile cannot be deleted: the app must always keep a valid active profile.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DeleteRefusal {
    /// It is the active profile.
    Active,
    /// It is the only profile.
    Last,
}

impl DeleteRefusal {
    /// The notice (msgid).
    pub fn msgid(self) -> &'static str {
        match self {
            Self::Active => {
                n_!("The active device cannot be deleted. Set another device active first.")
            }
            Self::Last => n_!("The only device cannot be deleted."),
        }
    }
}

/// Whether deleting profile `id` must be refused.
///
/// # Arguments
/// * `id` - The profile to delete.
/// * `active_id` - The active profile, if any.
/// * `count` - How many profiles there are.
pub fn delete_refusal(id: &str, active_id: Option<&str>, count: usize) -> Option<DeleteRefusal> {
    if active_id == Some(id) {
        Some(DeleteRefusal::Active)
    } else if count <= 1 {
        Some(DeleteRefusal::Last)
    } else {
        None
    }
}

/// The msgids of this module (form titles and details, labels, refusals).
pub fn msgids() -> Vec<&'static str> {
    let mut ids: Vec<&'static str> = vec![
        n_!("Invalid TCP Port"),
        n_!("TCP Port must be a number"),
        n_!("Invalid Timeout"),
        n_!("Timeout must be a number (ms)"),
        n_!("Invalid TCP Host"),
        n_!("TCP Host must not contain spaces"),
        n_!("TCP Host must not be empty for network connections"),
        n_!("Invalid X Min"),
        n_!("Invalid X Max"),
        n_!("Invalid Y Min"),
        n_!("Invalid Y Max"),
        n_!("Invalid Z Min"),
        n_!("Invalid Z Max"),
        n_!("Enter a number"),
        n_!("Invalid No of Axes"),
        n_!("Number of axes must be an integer between 1 and 6"),
        n_!("Invalid Max Feed Rate"),
        n_!("Invalid Max S-Value"),
        n_!("Max S-Value must be a number"),
        n_!("Invalid Spindle Power"),
        n_!("Spindle power must be a number (W)"),
        n_!("Invalid Max Spindle Speed"),
        n_!("Max spindle speed must be an integer (RPM)"),
        n_!("Invalid Laser Power"),
        n_!("Laser power must be a number (W)"),
        n_!("Unsupported controller"),
        n_!(
            "The selected controller \"{}\" is not supported yet. Please choose a supported one and try again."
        ),
        DeleteRefusal::Active.msgid(),
        DeleteRefusal::Last.msgid(),
        "Serial",
    ];
    ids.extend(DEVICE_TYPES.iter().map(device_type_label));
    ids.extend(
        CONTROLLER_CHOICES
            .iter()
            .map(|(_, label)| *label)
            .filter(|l| *l != "GRBL" && *l != "Repetier"),
    );
    ids
}
