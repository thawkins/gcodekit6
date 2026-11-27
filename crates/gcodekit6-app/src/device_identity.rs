//! Toolkit-free model of the device info panel: what the controller says about itself
//! (startup banner, `$I` build info, Repetier's `M115` reply) and the two settings the panel
//! shows as capabilities (`$22` homing, `$32` laser mode), plus the device-type selector's
//! rule for following `$32`.
//!
//! Dependencies: `gcodekit6-communication` (`$n=v` line parser).

use gcodekit6_communication::firmware::grbl::settings::SettingsManager;

/// What the connected controller reported about itself. `None` until reported.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct DeviceIdentity {
    /// Machine name (`[Machine:...]`, the `$I` build-info string, Repetier `MACHINE_TYPE:`).
    pub name: Option<String>,
    /// Firmware (`Grbl`, `GrblHAL`, `Repetier`, ...).
    pub firmware: Option<String>,
    /// Firmware version (`1.1h`).
    pub version: Option<String>,
    /// Build date of a `[VER:1.1h.20190825:]` report (`20190825`).
    pub build: Option<String>,
    /// Compile-time options of a `[OPT:VNM,15,128]` report (`VNM,15,128`).
    pub options: Option<String>,
    /// Homing cycle enabled (`$22`).
    pub homing: Option<bool>,
    /// Laser mode enabled (`$32`).
    pub laser: Option<bool>,
}

/// The value of a `[KEY:value]` line, if `line` is one with that key.
fn bracket_value<'a>(line: &'a str, key: &str) -> Option<&'a str> {
    line.strip_prefix('[')?
        .strip_suffix(']')?
        .strip_prefix(key)?
        .strip_prefix(':')
}

/// Splits a `$I` version (`1.1h.20190825`) into version and an eight-digit build date.
fn split_build(version: &str) -> (String, Option<String>) {
    match version.rsplit_once('.') {
        Some((v, b)) if b.len() == 8 && b.bytes().all(|c| c.is_ascii_digit()) => {
            (v.to_string(), Some(b.to_string()))
        }
        _ => (version.to_string(), None),
    }
}

impl DeviceIdentity {
    /// Takes what `line` says about the controller.
    ///
    /// # Arguments
    /// * `line` - One received line.
    ///
    /// # Returns
    /// Whether anything changed.
    pub fn apply_line(&mut self, line: &str) -> bool {
        let before = self.clone();
        let line = line.trim();
        if let Some(rest) = bracket_value(line, "VER") {
            let (version, info) = rest.split_once(':').unwrap_or((rest, ""));
            let (version, build) = split_build(version);
            self.version = Some(version);
            self.build = build;
            if !info.trim().is_empty() {
                self.name = Some(info.trim().to_string());
            }
            self.firmware.get_or_insert_with(|| "Grbl".to_string());
        } else if let Some(options) = bracket_value(line, "OPT") {
            self.options = Some(options.to_string());
        } else if let Some(name) = bracket_value(line, "Machine") {
            self.name = Some(name.to_string());
        } else if let Some(software) = bracket_value(line, "Software") {
            self.firmware = Some(format!("Grbl Ext ({software})"));
        } else if let Some(firmware) = bracket_value(line, "FIRMWARE") {
            self.firmware = Some(firmware.to_string());
        } else if line.starts_with("Grbl") && line.contains(" for help") {
            // Startup banner: `Grbl 1.1h ['$' for help]`, `GrblHAL 1.1f ['$' or '$HELP' ...]`.
            let mut words = line.split_whitespace();
            if let (Some(firmware), Some(version)) = (words.next(), words.next()) {
                self.firmware = Some(firmware.to_string());
                self.version = Some(version.to_string());
            }
        } else if let Some(reply) = line.split("FIRMWARE_NAME:").nth(1) {
            self.apply_m115(line, reply);
        } else if let Some((number, value)) = SettingsManager::parse_setting_line(line) {
            let value = value.trim().parse::<f64>().ok();
            match number {
                // `$22` is a bitmask on grblHAL: any bit set enables homing.
                22 => self.homing = value.map(|v| v >= 1.0),
                // `$32` is a mode on grblHAL (0 normal, 1 laser, 2 lathe): only 1 is laser.
                32 => self.laser = value.map(|v| v == 1.0),
                _ => {}
            }
        }
        *self != before
    }

    /// Takes a Repetier `M115` reply (`FIRMWARE_NAME:Repetier_1.0.6 ... MACHINE_TYPE:...`).
    fn apply_m115(&mut self, line: &str, reply: &str) {
        let token = reply.split_whitespace().next().unwrap_or("Repetier");
        match token.split_once('_') {
            Some((name, version)) => {
                self.firmware = Some(name.to_string());
                self.version = Some(version.to_string());
            }
            None => self.firmware = Some(token.to_string()),
        }
        if let Some(machine) = line.split("MACHINE_TYPE:").nth(1) {
            let name = machine.split("EXTRUDER_COUNT:").next().unwrap_or("").trim();
            if !name.is_empty() {
                self.name = Some(name.to_string());
            }
        }
    }

    /// Whether the controller said it runs Repetier firmware.
    pub fn is_repetier(&self) -> bool {
        self.firmware
            .as_deref()
            .is_some_and(|f| f.to_lowercase().contains("repetier"))
    }
}

/// The device type selector's choices.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum DeviceType {
    /// Spindle machining: laser mode off (`$32=0`).
    Cnc,
    /// Laser cutter or engraver: laser mode on (`$32=1`).
    Laser,
    /// Anything else; `$32` is left as it is.
    #[default]
    Other,
}

impl DeviceType {
    /// The `$32` value choosing this type writes, or `None` for [`DeviceType::Other`].
    pub fn laser_mode_value(self) -> Option<&'static str> {
        match self {
            DeviceType::Cnc => Some("0"),
            DeviceType::Laser => Some("1"),
            DeviceType::Other => None,
        }
    }

    /// The type the selector should switch to when the controller reports `$32`, or `None`
    /// to keep `current`.
    ///
    /// The first report after connecting (`initialised` false) selects Laser for `$32=1`
    /// and CNC for `$32=0`. Later reports only correct a selection that contradicts `$32`,
    /// so Other (which also means laser mode off) is kept.
    ///
    /// # Arguments
    /// * `current` - The selected type.
    /// * `initialised` - Whether the selection already followed `$32` on this connection.
    /// * `laser` - Whether `$32` reports laser mode on.
    pub fn follow_laser_mode(
        current: DeviceType,
        initialised: bool,
        laser: bool,
    ) -> Option<DeviceType> {
        match (laser, initialised, current) {
            (true, _, DeviceType::Laser) => None,
            (true, _, DeviceType::Cnc | DeviceType::Other) => Some(DeviceType::Laser),
            (false, false, _) => Some(DeviceType::Cnc),
            (false, true, DeviceType::Laser) => Some(DeviceType::Cnc),
            (false, true, DeviceType::Cnc | DeviceType::Other) => None,
        }
    }
}
