//! GRBL Error and Alarm Code Decoder
//! Converts numeric error and alarm codes to human-readable messages.
//!
//! Codes 1-38 (errors) and 1-9 (alarms) are the GRBL 1.1 texts. Higher codes are grblHAL's
//! extensions, transcribed from grblHAL/core at commit
//! a53a16a4d756f12a27006152ba3a99b5ec46b2c7 (`errors.h`/`errors.c`, `alarms.h`/`alarms.c`).

/// Decode GRBL error code to human-readable message
pub fn decode_error(code: u8) -> String {
    match code {
        1 => "G-code words consist of a letter and a value. Letter was not found.".to_string(),
        2 => "Numeric value format is not valid or missing an expected value.".to_string(),
        3 => "Grbl '$' system command was not recognized or supported.".to_string(),
        4 => "Negative value received for an expected positive value.".to_string(),
        5 => "Homing cycle is not enabled via settings.".to_string(),
        6 => "Minimum step pulse time must be greater than 3µs.".to_string(),
        7 => "EEPROM read failed. Reset and restored to default values.".to_string(),
        8 => "Grbl '$' command cannot be used unless Grbl is IDLE. Ensures smooth operation during a job.".to_string(),
        9 => "G-code locked out during alarm or jog state.".to_string(),
        10 => "Soft limits cannot be enabled without homing also enabled.".to_string(),
        11 => "Max characters per line exceeded. Line was not processed and executed.".to_string(),
        12 => "Grbl '$' setting value exceeds the maximum step rate supported.".to_string(),
        13 => "Safety door detected as opened and door state initiated.".to_string(),
        14 => "Build info or startup line exceeded EEPROM line length limit.".to_string(),
        15 => "Jog target exceeds machine travel. Command ignored.".to_string(),
        16 => "Jog command with no '=' or contains prohibited g-code.".to_string(),
        17 => "Laser mode requires PWM output.".to_string(),
        20 => "Unsupported or invalid g-code command found in block.".to_string(),
        21 => "More than one g-code command from same modal group found in block.".to_string(),
        22 => "Feed rate has not yet been set or is undefined.".to_string(),
        23 => "G-code command in block requires an integer value.".to_string(),
        24 => "Two G-code commands that both require the use of the XYZ axis words were detected in the block.".to_string(),
        25 => "A G-code word was repeated in the block.".to_string(),
        26 => "A G-code command implicitly or explicitly requires XYZ axis words in the block, but none were detected.".to_string(),
        27 => "N line number value is not within the valid range of 1 - 9,999,999.".to_string(),
        28 => "A G-code command was sent, but is missing some required P or L value words in the line.".to_string(),
        29 => "Grbl supports six work coordinate systems G54-G59. G59.1, G59.2, and G59.3 are not supported.".to_string(),
        30 => "The G53 G-code command requires either a G0 seek or G1 feed motion mode to be active.".to_string(),
        31 => "There are unused axis words in the block and G80 motion mode cancel is active.".to_string(),
        32 => "A G2 or G3 arc was commanded but there are no XYZ axis words in the selected plane to trace the arc.".to_string(),
        33 => "The motion command has an invalid target. G2, G3, and G38.2 generates this error if the arc is impossible to generate or if the probe target is the current position.".to_string(),
        34 => "A G2 or G3 arc, traced with the radius definition, had a mathematical error when computing the arc geometry.".to_string(),
        35 => "A G2 or G3 arc, traced with the offset definition, is missing the IJK offset word in the selected plane to trace the arc.".to_string(),
        36 => "There are unused, leftover G-code words that aren't used by any command in the block.".to_string(),
        37 => "The G43.1 dynamic tool length offset command cannot apply an offset to an axis other than its configured axis.".to_string(),
        38 => "Tool number greater than max supported value.".to_string(),

        // grblHAL extended status codes (`status_code_t`, errors.h; texts from errors.c at
        // grblHAL/core a53a16a4d756f12a27006152ba3a99b5ec46b2c7). Codes upstream defines
        // without a description (59-70, 85, 89) fall through to "Unknown error code".
        39 => "Value out of range.".to_string(),
        40 => "G-code command not allowed when tool change is pending.".to_string(),
        41 => "Spindle not running when motion commanded in CSS or spindle sync mode.".to_string(),
        42 => "Plane must be ZX for threading and lathe cycles.".to_string(),
        43 => "Max. feed rate exceeded.".to_string(),
        44 => "RPM out of range.".to_string(),
        45 => "Only homing is allowed when a limit switch is engaged.".to_string(),
        46 => "Home machine to continue.".to_string(),
        47 => "ATC: current tool is not set. Set current tool with M61.".to_string(),
        48 => "Value word conflict.".to_string(),
        49 => "Power on self test failed. A hard reset is required.".to_string(),
        50 => "Emergency stop active.".to_string(),
        51 => "Motor fault.".to_string(),
        52 => "Setting value is out of range.".to_string(),
        53 => "Setting is not available, possibly due to limited driver support.".to_string(),
        54 => "Retract position is less than drill depth.".to_string(),
        55 => "Attempt to home two auto squared axes at the same time.".to_string(),
        56 => "Coordinate system is locked.".to_string(),
        57 => "Unexpected file demarcation.".to_string(),
        58 => "Port is not available".to_string(),
        71 => "Unknown operation found in expression.".to_string(),
        72 => "Divide by zero in expression attempted.".to_string(),
        73 => "Too large or too small argrument provided.".to_string(),
        74 => "Argument is not valid for the operation".to_string(),
        75 => "Expression is not valid.".to_string(),
        76 => "Either NAN (not a number) or infinity was returned from expression.".to_string(),
        77 => "Authentication required.".to_string(),
        78 => "Access denied.".to_string(),
        79 => "Not allowed while critical event is active.".to_string(),
        80 => "Flow statement only allowed in filesystem macro.".to_string(),
        81 => "Unknown or invalid flow statement.".to_string(),
        82 => "Stack overflow while executing flow statement.".to_string(),
        83 => "Out of memory while executing flow statement.".to_string(),
        84 => "Could not open file.".to_string(),
        86 => "Port is not usable.".to_string(),
        87 => "Tool in spindle.".to_string(),
        88 => "No tool in spindle.".to_string(),
        90 => "Cutter compensation is already active.".to_string(),
        91 => "Command not allowed while cutter compensation is active.".to_string(),
        92 => "Cutter compensation is not valid for current move.".to_string(),
        253 => "User defined error occured.".to_string(),

        _ => format!("Unknown error code: {}", code),
    }
}

/// Decode GRBL alarm code to human-readable message
pub fn decode_alarm(code: u8) -> String {
    match code {
        1 => "Hard limit triggered. Machine position is likely lost due to sudden and immediate halt. Re-homing is highly recommended.".to_string(),
        2 => "Soft limit: G-code motion target exceeds machine travel. Machine position safely retained. Alarm may be unlocked.".to_string(),
        3 => "Reset while in motion. Grbl cannot guarantee position. Lost steps are likely. Re-homing is highly recommended.".to_string(),
        4 => "Probe fail. The probe is not in the expected initial state before starting probe cycle, where G38.2 and G38.3 is not triggered and G38.4 and G38.5 is triggered.".to_string(),
        5 => "Probe fail. Probe did not contact the workpiece within the programmed travel for G38.2 and G38.4.".to_string(),
        6 => "Homing fail. Reset during active homing cycle.".to_string(),
        7 => "Homing fail. Safety door was opened during active homing cycle.".to_string(),
        8 => "Homing fail. Cycle failed to clear limit switch when pulling off. Try increasing pull-off setting or check wiring.".to_string(),
        9 => "Homing fail. Could not find limit switch within search distance. Defined as 1.5 * max_travel on search and 5 * pulloff on locate phases.".to_string(),

        // grblHAL extended alarm codes (`alarm_code_t`, alarms.h; texts from alarms.c at
        // grblHAL/core a53a16a4d756f12a27006152ba3a99b5ec46b2c7).
        10 => "EStop asserted. Clear and reset".to_string(),
        11 => "Homing required. Execute homing command ($H) to continue.".to_string(),
        12 => "Limit switch engaged. Clear before continuing.".to_string(),
        13 => "Probe protection triggered. Clear before continuing.".to_string(),
        14 => "Spindle at speed timeout. Clear before continuing.".to_string(),
        15 => "Homing fail. Could not find second limit switch for auto squared axis within search distances. Try increasing max travel, decreasing pull-off distance, or check wiring.".to_string(),
        16 => "Power on selftest (POS) failed.".to_string(),
        17 => "Motor fault.".to_string(),
        18 => "Homing fail. Bad configuration.".to_string(),
        19 => "Modbus exception. Timeout or message error.".to_string(),
        20 => "I/O expander communication failed.".to_string(),
        21 => "Non Volatile Storage (EEPROM) failure.".to_string(),
        22 => "Buffer overflow.".to_string(),

        _ => format!("Unknown alarm code: {}", code),
    }
}

/// Format error message with code and description
pub fn format_error(code: u8) -> String {
    format!("error:{} - {}", code, decode_error(code))
}

/// Format alarm message with code and description
pub fn format_alarm(code: u8) -> String {
    format!("ALARM:{} - {}", code, decode_alarm(code))
}
