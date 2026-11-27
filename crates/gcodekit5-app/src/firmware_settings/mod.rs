//! Toolkit-free model of the firmware settings page:
//! the GRBL and Repetier setting definitions, the category of each setting, the list filter,
//! the Repetier `EPR:` line parser, the command that writes a setting, and when a settings
//! command may be sent.
//!
//! Settings live in a `SettingsManager` of `gcodekit5-communication`; Repetier EEPROM
//! positions are stored as numbers from [`REPETIER_OFFSET`] up, so both firmwares share one
//! table type.
//!
//! Dependencies: `gcodekit5-communication` (`Setting`, `SettingsManager`),
//! `gcodekit5-devicedb` (the device profile's `ControllerType`).

mod grbl_definitions;
mod repetier_definitions;

use gcodekit5_communication::firmware::grbl::settings::{Setting, SettingsManager};
use gcodekit5_devicedb::ControllerType;

use crate::device_identity::DeviceIdentity;

pub use grbl_definitions::grbl_definitions;
pub use repetier_definitions::repetier_definitions;

/// One known setting: number, name, description, read-only flag and unit (empty for none).
pub type SettingDefinition = (u16, &'static str, &'static str, bool, &'static str);

/// Repetier EEPROM position `p` is stored as setting number `REPETIER_OFFSET + p`.
pub const REPETIER_OFFSET: u16 = 1000;

/// The category filter's choices, which are also the category ids; "All" matches every
/// category. Each is an English msgid, translated where it is shown through a variable, so
/// each is marked with `n_!` for `scripts/update-po.sh` to extract.
pub const SETTING_CATEGORIES: &[&str] = &[
    n_!("All"),
    n_!("System"),
    n_!("Motion"),
    n_!("Steps Per Unit"),
    n_!("Max Rate"),
    n_!("Acceleration"),
    n_!("Max Travel"),
    n_!("Homing"),
    n_!("Spindle"),
    n_!("Network"),
    n_!("Drivers"),
    n_!("Tool"),
    n_!("I/O"),
    n_!("Startup"),
    n_!("Plasma"),
    n_!("Encoder"),
    n_!("Modbus"),
    n_!("RS485"),
    n_!("Repetier EEPROM"),
    n_!("Other"),
];

/// The category (an id of [`SETTING_CATEGORIES`]) a setting number belongs to.
///
/// # Arguments
/// * `number` - Setting number (Repetier positions offset by [`REPETIER_OFFSET`]).
pub fn category_of(number: u16) -> &'static str {
    match number {
        0..=20 => "System",
        21..=29 => "Homing",
        30..=39 => "Spindle",
        40..=65 => "System",
        70..=75 => "Network",
        100..=105 => "Steps Per Unit",
        110..=115 => "Max Rate",
        120..=125 => "Acceleration",
        130..=135 => "Max Travel",
        200..=210 => "Drivers",
        300..=321 => "Network",
        340..=344 => "Tool",
        370..=372 => "I/O",
        384..=385 => "Motion",
        395..=397 => "Startup",
        481 => "System",
        550..=560 => "Plasma",
        600..=603 => "Encoder",
        650..=651 => "Modbus",
        680 => "RS485",
        1000..=1999 => "Repetier EEPROM",
        _ => "Other",
    }
}

/// How a setting number is shown and searched: `$110`, or `EPR:100` for a Repetier
/// EEPROM position.
///
/// # Arguments
/// * `number` - Setting number (Repetier positions offset by [`REPETIER_OFFSET`]).
pub fn setting_id(number: u16) -> String {
    if number >= REPETIER_OFFSET {
        format!("EPR:{}", number - REPETIER_OFFSET)
    } else {
        format!("${number}")
    }
}

/// One row of the settings list.
#[derive(Debug, Clone, PartialEq)]
pub struct ConfigSettingRow {
    /// Setting number (Repetier positions offset by [`REPETIER_OFFSET`]).
    pub number: u16,
    /// Setting name.
    pub name: String,
    /// Current value, as the controller reported it.
    pub value: String,
    /// Unit, empty for none.
    pub unit: String,
    /// Description.
    pub description: String,
    /// Category id (one of [`SETTING_CATEGORIES`]).
    pub category: String,
    /// Whether the setting cannot be written.
    pub read_only: bool,
}

impl ConfigSettingRow {
    /// Whether the row is shown for the given search text and category filter.
    ///
    /// The display list and row activation must use this same test, so that an
    /// activated row index refers to the setting the user clicked.
    ///
    /// # Arguments
    /// * `search_text` - Lower-cased search text; empty matches everything. It matches the
    ///   name, the description or the id (`$110`, `epr:100`).
    /// * `category` - Selected category id; "All" matches every category.
    pub fn matches_filter(&self, search_text: &str, category: &str) -> bool {
        if !search_text.is_empty() {
            let id = setting_id(self.number).to_lowercase();
            let matches = self.name.to_lowercase().contains(search_text)
                || self.description.to_lowercase().contains(search_text)
                || id.contains(search_text);
            if !matches {
                return false;
            }
        }
        category == "All" || self.category == category
    }

    /// The row's id as shown: `$110` or `EPR:100`.
    pub fn id(&self) -> String {
        setting_id(self.number)
    }
}

impl From<&Setting> for ConfigSettingRow {
    fn from(s: &Setting) -> Self {
        Self {
            number: s.number,
            name: s.name.clone(),
            value: s.value.clone(),
            unit: s.unit.clone().unwrap_or_default(),
            description: s.description.clone(),
            category: category_of(s.number).to_string(),
            read_only: s.read_only,
        }
    }
}

/// The rows of `manager` that match the filter, in number order.
///
/// # Arguments
/// * `manager` - The settings table.
/// * `search` - Search text as typed (matched case-insensitively).
/// * `category` - Selected category id; "All" matches every category.
pub fn filtered_rows(
    manager: &SettingsManager,
    search: &str,
    category: &str,
) -> Vec<ConfigSettingRow> {
    let search = search.trim().to_lowercase();
    let mut rows: Vec<ConfigSettingRow> = manager
        .get_all_settings()
        .into_iter()
        .map(ConfigSettingRow::from)
        .filter(|row| row.matches_filter(&search, category))
        .collect();
    rows.sort_by_key(|row| row.number);
    rows
}

/// Replaces every setting of `manager` with `definitions`, each valued `0` until the
/// controller reports it.
///
/// # Arguments
/// * `manager` - The settings table.
/// * `definitions` - [`grbl_definitions`] or [`repetier_definitions`].
pub fn load_definitions(manager: &mut SettingsManager, definitions: Vec<SettingDefinition>) {
    manager.clear();
    for (number, name, description, read_only, unit) in definitions {
        manager.set_setting(Setting {
            number,
            name: name.to_string(),
            value: "0".to_string(),
            numeric_value: Some(0.0),
            description: description.to_string(),
            range: None,
            read_only,
            unit: (!unit.is_empty()).then(|| unit.to_string()),
            eeprom_type: None,
        });
    }
}

/// Stores `value` for setting `number` if the table knows it.
///
/// # Arguments
/// * `manager` - The settings table.
/// * `number` - Setting number.
/// * `value` - The value the controller holds.
///
/// # Returns
/// Whether the table knows the setting (and now holds `value`).
pub fn apply_value(manager: &mut SettingsManager, number: u16, value: &str) -> bool {
    let Some(setting) = manager.get_setting(number) else {
        return false;
    };
    let mut updated = setting.clone();
    updated.value = value.to_string();
    updated.numeric_value = value.parse::<f64>().ok();
    manager.set_setting(updated);
    true
}

/// Parses a Repetier `M205` reply line (`EPR:<type> <pos> <value> <description [unit]>`).
///
/// # Arguments
/// * `line` - One received line; `EPR:` may appear anywhere in it.
///
/// # Returns
/// The setting, numbered [`REPETIER_OFFSET`] + position, or `None` for another line.
pub fn parse_repetier_epr_line(line: &str) -> Option<Setting> {
    let (_, rest) = line.split_once("EPR:")?;
    let mut parts = rest.trim().splitn(4, ' ');

    let eeprom_type: u8 = parts.next()?.parse().ok()?;
    let pos: u16 = parts.next()?.parse().ok()?;
    let number = REPETIER_OFFSET.checked_add(pos)?;
    let value = parts.next()?.to_string();
    let raw_desc = parts.next().unwrap_or("Repetier Parameter").trim();

    let (name, unit) = match raw_desc.rsplit_once('[') {
        Some((n, u)) => (
            n.trim().to_string(),
            Some(u.trim_end_matches(']').to_string()),
        ),
        None => (raw_desc.to_string(), None),
    };

    Some(Setting {
        number,
        name,
        numeric_value: value.parse::<f64>().ok(),
        value,
        description: format!("EEPROM Pos {pos}: {raw_desc}"),
        range: None,
        read_only: false,
        unit,
        eeprom_type: Some(eeprom_type),
    })
}

/// The line that writes `value` to setting `number`: `$110=5000` for GRBL, or Repetier's
/// `M206 T<type> P<pos> S<value>` (`X<value>` for a float, type 3). The type is the one the
/// controller reported for the position (`EPR:<type> ...`), never guessed from the value:
/// `M206` stores the value's bytes as that type, so a wrong type corrupts the position.
///
/// # Arguments
/// * `number` - Setting number (Repetier positions offset by [`REPETIER_OFFSET`]).
/// * `eeprom_type` - The Repetier position's reported type ([`Setting::eeprom_type`]).
/// * `value` - The new value, already checked.
///
/// # Returns
/// The line, or `None` for a Repetier position whose type is unknown or not 0-3.
pub fn write_command(number: u16, eeprom_type: Option<u8>, value: &str) -> Option<String> {
    if number < REPETIER_OFFSET {
        return Some(format!("${number}={value}"));
    }
    let pos = number - REPETIER_OFFSET;
    match eeprom_type? {
        3 => Some(format!("M206 T3 P{pos} X{value}")),
        t @ 0..=2 => Some(format!("M206 T{t} P{pos} S{value}")),
        _ => None,
    }
}

/// Why a typed value is not written.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ValueError {
    /// Nothing was typed.
    Empty,
    /// The setting takes a number and this is not one (`5,000`, `abc`).
    NotANumber,
    /// The Repetier position takes an integer.
    NotAWholeNumber,
    /// The integer does not fit the Repetier position's type.
    OutOfRange,
    /// GRBL refuses a negative value (`error:4`).
    Negative,
}

impl ValueError {
    /// The English msgid of the reason.
    pub fn msgid(self) -> &'static str {
        match self {
            ValueError::Empty => n_!("Enter a value"),
            ValueError::NotANumber => n_!("Enter a number"),
            ValueError::NotAWholeNumber => n_!("Enter a whole number"),
            ValueError::OutOfRange => n_!("The value is out of range"),
            ValueError::Negative => n_!("Enter a value of 0 or more"),
        }
    }
}

/// Which GRBL-family firmware the controller runs, as far as is known; it decides whether
/// the masks and switches below `$100` are bytes ([`check_value`]).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum GrblFamily {
    /// Classic GRBL (0.9, 1.1), which stores those settings in a byte.
    Classic,
    /// grblHAL: wider masks (`$10=511`) and other meanings (`$56` is a distance in mm).
    GrblHal,
    /// FluidNC: settings beyond GRBL 1.1's byte range.
    FluidNc,
    /// Nothing says which firmware it is.
    #[default]
    Unknown,
}

impl GrblFamily {
    /// The firmware from what the controller said and the active device profile.
    ///
    /// grblHAL or FluidNC named by either source wins: GRBL 1.1's `$I` reply sets the
    /// firmware to `Grbl` before grblHAL's `[FIRMWARE:grblHAL]` line, and FluidNC's banner
    /// says `Grbl`, so a plain `Grbl` is weaker evidence than a named variant. A wrong
    /// "grblHAL" profile on a classic GRBL only lifts the pre-send guard; the read-back
    /// after a write still reports a truncated value.
    ///
    /// # Arguments
    /// * `identity` - What the controller reported about itself.
    /// * `profile` - The active device profile's controller type, if a profile is active.
    pub fn detect(identity: &DeviceIdentity, profile: Option<ControllerType>) -> Self {
        let said = [&identity.firmware, &identity.name, &identity.version]
            .into_iter()
            .flatten()
            .map(|text| text.to_lowercase())
            .collect::<Vec<_>>();
        let says = |marker: &str| said.iter().any(|text| text.contains(marker));
        if says("grblhal") || profile == Some(ControllerType::GrblHal) {
            GrblFamily::GrblHal
        } else if says("fluidnc") || profile == Some(ControllerType::FluidNC) {
            GrblFamily::FluidNc
        } else if identity
            .firmware
            .as_deref()
            .is_some_and(|f| f.eq_ignore_ascii_case("grbl"))
            || profile == Some(ControllerType::Grbl)
        {
            GrblFamily::Classic
        } else {
            GrblFamily::Unknown
        }
    }

    /// Whether the masks and switches below `$100` may be bytes: classic GRBL, and an
    /// unknown firmware (fail safe).
    fn has_byte_settings(self) -> bool {
        match self {
            GrblFamily::Classic | GrblFamily::Unknown => true,
            GrblFamily::GrblHal | GrblFamily::FluidNc => false,
        }
    }
}

/// Checks a typed value before it is written, and trims it.
///
/// A Repetier position takes what its reported type holds (a byte, a 16- or 32-bit
/// integer, or a float). A GRBL setting whose reported value is a number takes a number of
/// 0 or more (with a decimal point, never a comma; GRBL answers a negative with `error:4`);
/// on classic GRBL (or an unknown firmware), a mask or switch below `$100`
/// ([`grbl_definitions`] unit `mask` or `bool`), which GRBL 1.1 stores in a byte, takes a
/// whole number from 0 to 255, unless its reported value is above 255 or has a fraction,
/// which proves it is not a byte on this controller. A setting reported as text (a grblHAL
/// host name) takes any text.
///
/// # Arguments
/// * `setting` - The setting as the controller reported it.
/// * `value` - The value as typed.
/// * `family` - The GRBL-family firmware the controller runs ([`GrblFamily::detect`]).
///
/// # Errors
/// The [`ValueError`] saying why the value cannot be written.
pub fn check_value(
    setting: &Setting,
    value: &str,
    family: GrblFamily,
) -> Result<String, ValueError> {
    let value = value.trim();
    if value.is_empty() {
        return Err(ValueError::Empty);
    }
    let number = || {
        value
            .parse::<f64>()
            .ok()
            .filter(|v| v.is_finite() && !value.contains(|c: char| c.is_alphabetic()))
            .ok_or(ValueError::NotANumber)
    };
    let byte = Some((0, i64::from(u8::MAX)));
    let range = match setting.eeprom_type {
        Some(0) => byte,
        Some(1) => Some((i64::from(i16::MIN), i64::from(i16::MAX))),
        Some(2) => Some((i64::from(i32::MIN), i64::from(i32::MAX))),
        Some(_) => None,
        None if setting.numeric_value.is_none() => return Ok(value.to_string()),
        None if family.has_byte_settings()
            && is_grbl_byte_setting(setting.number)
            && setting
                .numeric_value
                .is_none_or(|v| (0.0..=255.0).contains(&v) && v.fract() == 0.0) =>
        {
            byte
        }
        None => None,
    };
    let parsed = number()?;
    if setting.number < REPETIER_OFFSET && parsed < 0.0 {
        return Err(ValueError::Negative);
    }
    if let Some((min, max)) = range {
        let integer: i64 = value.parse().map_err(|_| ValueError::NotAWholeNumber)?;
        if integer < min || integer > max {
            return Err(ValueError::OutOfRange);
        }
    }
    Ok(value.to_string())
}

/// Whether GRBL setting `number` is a mask or switch below `$100` (unit `mask` or `bool` in
/// [`grbl_definitions`]), which GRBL 1.1 stores in a byte.
fn is_grbl_byte_setting(number: u16) -> bool {
    number < 100
        && grbl_definitions()
            .iter()
            .any(|&(n, _, _, _, unit)| n == number && matches!(unit, "mask" | "bool"))
}

/// Why a settings command (`$$`, `$I`, `$n=v`) must not be sent now.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SettingsBlocked {
    /// No controller is connected.
    NotConnected,
    /// A job (or a probe routine, or its halt) is streaming.
    JobRunning,
    /// An earlier command line has not been answered yet, so the reply to this one could
    /// not be told from its reply.
    Unanswered,
    /// GRBL takes `$` commands only while Idle or in Alarm; holds the reported state.
    NotIdle(String),
}

/// Why a settings command must not be sent now, or `None` when it may be.
///
/// # Arguments
/// * `connected` - Whether a controller is connected.
/// * `job_running` - Whether a program streams (the app's machine-busy predicate).
/// * `lines_answered` - Whether every command line sent so far has had its reply.
/// * `state` - The latest reported machine state (`Idle`, `Run`, ...), if any; an unknown
///   state does not block.
pub fn settings_blocked(
    connected: bool,
    job_running: bool,
    lines_answered: bool,
    state: Option<&str>,
) -> Option<SettingsBlocked> {
    if !connected {
        return Some(SettingsBlocked::NotConnected);
    }
    if job_running {
        return Some(SettingsBlocked::JobRunning);
    }
    if !lines_answered {
        return Some(SettingsBlocked::Unanswered);
    }
    let state = state.unwrap_or("").trim();
    let low = state.to_ascii_lowercase();
    if low.is_empty() || low.starts_with("idle") || low.starts_with("alarm") {
        None
    } else {
        Some(SettingsBlocked::NotIdle(state.to_string()))
    }
}
