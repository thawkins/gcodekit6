//! Toolkit-free model of the firmware settings page, shared by the GTK and egui front ends:
//! the GRBL and Repetier setting definitions, the category of each setting, the list filter,
//! the Repetier `EPR:` line parser, the command that writes a setting, and when a settings
//! command may be sent.
//!
//! Settings live in a `SettingsManager` of `gcodekit5-communication`; Repetier EEPROM
//! positions are stored as numbers from [`REPETIER_OFFSET`] up, so both firmwares share one
//! table type.
//!
//! Dependencies: `gcodekit5-communication` (`Setting`, `SettingsManager`).

mod grbl_definitions;
mod repetier_definitions;

use gcodekit5_communication::firmware::grbl::settings::{Setting, SettingsManager};

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
        21..=32 => "Homing",
        33..=39 => "Spindle",
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

    let _data_type = parts.next()?;
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
    })
}

/// The line that writes `value` to setting `number`: `$110=5000` for GRBL, or Repetier's
/// `M206 T3 P<pos> X<value>` (a value with a decimal point) / `M206 T2 P<pos> S<value>`.
///
/// # Arguments
/// * `number` - Setting number (Repetier positions offset by [`REPETIER_OFFSET`]).
/// * `value` - The new value, as typed.
pub fn write_command(number: u16, value: &str) -> String {
    if number >= REPETIER_OFFSET {
        let pos = number - REPETIER_OFFSET;
        if value.contains('.') {
            format!("M206 T3 P{pos} X{value}")
        } else {
            format!("M206 T2 P{pos} S{value}")
        }
    } else {
        format!("${number}={value}")
    }
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
///   state does not block, as in the GTK front end.
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
