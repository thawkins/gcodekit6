//! Speeds and Feeds Calculator - Database Integration Services
//!
//! Provides service layers for integrating the feeds and speeds calculator
//! with the Devices database.

use gcodekit6_devicedb::model::DeviceProfile;

/// Service for querying device/machine capabilities
#[derive(Debug)]
pub struct SpeedsFeedsDeviceService {
    device: DeviceProfile,
}

impl SpeedsFeedsDeviceService {
    /// Create a new device service
    pub fn new(device: DeviceProfile) -> Self {
        Self { device }
    }

    /// Get device limits relevant to feeds and speeds
    pub fn get_limits(&self) -> DeviceLimits {
        DeviceLimits {
            max_rpm: self.device.max_spindle_speed_rpm,
            max_feed_rate: self.device.max_feed_rate as f32,
            max_power_kw: self.device.cnc_spindle_watts as f32 / 1000.0,
            num_axes: self.device.num_axes,
            has_coolant: self.device.has_coolant,
        }
    }

    /// Validate RPM against device limits
    pub fn validate_rpm(&self, rpm: u32) -> Result<u32, String> {
        if rpm > self.device.max_spindle_speed_rpm {
            Err(format!(
                "RPM {} exceeds device maximum {}",
                rpm, self.device.max_spindle_speed_rpm
            ))
        } else if rpm < 1000 {
            Err(format!("RPM {} below minimum 1000", rpm))
        } else {
            Ok(rpm)
        }
    }

    /// Validate feed rate against device limits
    pub fn validate_feed_rate(&self, feed_rate: f32) -> Result<f32, String> {
        if feed_rate > self.device.max_feed_rate as f32 {
            Err(format!(
                "Feed rate {:.1} exceeds device maximum {:.1}",
                feed_rate, self.device.max_feed_rate
            ))
        } else if feed_rate <= 0.0 {
            Err("Feed rate must be positive".to_string())
        } else {
            Ok(feed_rate)
        }
    }

    /// Check if device can handle the calculated power requirement
    pub fn can_handle_power(&self, required_power_kw: f32) -> Result<(), String> {
        let available_power = self.device.cnc_spindle_watts as f32 / 1000.0;
        if required_power_kw > available_power {
            Err(format!(
                "Required power {:.2}kW exceeds available power {:.2}kW",
                required_power_kw, available_power
            ))
        } else {
            Ok(())
        }
    }

    /// Get recommended spindle RPM range
    pub fn recommended_rpm_range(&self, tool_diameter: f32) -> (u32, u32) {
        let min_rpm = 1000u32;
        let max_rpm = self.device.max_spindle_speed_rpm;

        // Adjust based on tool diameter (larger tools need lower RPM)
        let diameter_factor = (6.0 / tool_diameter).clamp(0.5, 2.0);
        let adjusted_max = ((max_rpm as f32 * diameter_factor).min(max_rpm as f32)) as u32;

        (min_rpm, adjusted_max)
    }

    /// Check if coolant is available
    pub fn has_coolant(&self) -> bool {
        self.device.has_coolant
    }

    /// Get a copy of the device profile
    pub fn device(&self) -> &DeviceProfile {
        &self.device
    }

    /// Get device-specific recommendations
    pub fn get_recommendations(&self) -> DeviceRecommendations {
        DeviceRecommendations {
            max_rigorous_doc_percent: if self.device.device_type
                == gcodekit6_devicedb::model::DeviceType::CncMill
            {
                100.0
            } else {
                50.0
            },
            max_adaptive_engagement: if self.device.num_axes >= 3 {
                20.0
            } else {
                15.0
            },
            recommended_stepover_percent: match self.device.device_type {
                gcodekit6_devicedb::model::DeviceType::CncMill => 50.0,
                gcodekit6_devicedb::model::DeviceType::CncLathe => 80.0,
                _ => 40.0,
            },
        }
    }
}

/// Device limits for feeds and speeds
#[derive(Debug, Clone, Copy)]
pub struct DeviceLimits {
    /// Maximum spindle RPM
    pub max_rpm: u32,
    /// Maximum feed rate in mm/min
    pub max_feed_rate: f32,
    /// Maximum spindle power in kW
    pub max_power_kw: f32,
    /// Number of axes
    pub num_axes: u8,
    /// Has coolant system
    pub has_coolant: bool,
}

/// Device-specific recommendations
#[derive(Debug, Clone, Copy)]
pub struct DeviceRecommendations {
    /// Maximum depth of cut as percentage of diameter
    pub max_rigorous_doc_percent: f32,
    /// Maximum radial engagement for adaptive (percentage)
    pub max_adaptive_engagement: f32,
    /// Recommended stepover percentage
    pub recommended_stepover_percent: f32,
}

/// Data context for the calculator: the active device, if any
#[derive(Debug, Default)]
pub struct SpeedsFeedsDataContext {
    /// Device service for machine capabilities
    pub device_service: Option<SpeedsFeedsDeviceService>,
}

impl SpeedsFeedsDataContext {
    /// Create a new data context for `device`
    pub fn new(device: Option<DeviceProfile>) -> Self {
        Self {
            device_service: device.map(SpeedsFeedsDeviceService::new),
        }
    }

    /// Set the active device
    pub fn with_device(mut self, device: DeviceProfile) -> Self {
        self.device_service = Some(SpeedsFeedsDeviceService::new(device));
        self
    }

    /// Get device limits (or defaults if no device set)
    pub fn get_device_limits(&self) -> DeviceLimits {
        self.device_service
            .as_ref()
            .map(|s| s.get_limits())
            .unwrap_or_else(|| DeviceLimits {
                max_rpm: 12000,
                max_feed_rate: 1000.0,
                max_power_kw: 0.5,
                num_axes: 3,
                has_coolant: false,
            })
    }

    /// Check if all services are ready
    pub fn is_ready(&self) -> bool {
        // Device is optional but recommended
        true
    }

    /// Get validation warnings if context is incomplete
    pub fn get_warnings(&self) -> Vec<String> {
        let mut warnings = Vec::new();

        if self.device_service.is_none() {
            warnings.push("No device profile selected. Using default limits.".to_string());
        }

        warnings
    }
}

/// Pre-calculated lookup table for common material/tool combinations
pub struct FeedsSpeedsLookupTable {
    /// Cached results by (tool_id, material_id, operation)
    cache: std::collections::HashMap<(String, String, String), super::CalculationResult>,
}

impl FeedsSpeedsLookupTable {
    /// Create a new empty lookup table
    pub fn new() -> Self {
        Self {
            cache: std::collections::HashMap::new(),
        }
    }

    /// Get a cached result
    pub fn get(
        &self,
        tool_id: &str,
        material_id: &str,
        operation: &str,
    ) -> Option<&super::CalculationResult> {
        self.cache.get(&(
            tool_id.to_string(),
            material_id.to_string(),
            operation.to_string(),
        ))
    }

    /// Store a result in the cache
    pub fn cache(
        &mut self,
        tool_id: &str,
        material_id: &str,
        operation: &str,
        result: super::CalculationResult,
    ) {
        self.cache.insert(
            (
                tool_id.to_string(),
                material_id.to_string(),
                operation.to_string(),
            ),
            result,
        );
    }

    /// Clear the cache
    pub fn clear(&mut self) {
        self.cache.clear();
    }

    /// Get cache size
    pub fn len(&self) -> usize {
        self.cache.len()
    }

    /// Check if cache is empty
    pub fn is_empty(&self) -> bool {
        self.cache.is_empty()
    }
}

impl Default for FeedsSpeedsLookupTable {
    fn default() -> Self {
        Self::new()
    }
}
