//! Tests for `gcodekit5_camtools::speeds_feeds::service` (moved from the inline test module).

use gcodekit5_camtools::speeds_feeds::*;
use gcodekit5_devicedb::model::DeviceProfile;

#[test]
fn test_device_service_limits() {
    let device = DeviceProfile::default();
    let service = SpeedsFeedsDeviceService::new(device);
    let limits = service.get_limits();

    assert!(limits.max_rpm > 0);
    assert!(limits.max_feed_rate > 0.0);
}

#[test]
fn test_lookup_table() {
    let mut table = FeedsSpeedsLookupTable::new();
    assert!(table.is_empty());

    // Create a dummy result - need to create minimal CalculationResult
    // We can't easily create one here without importing, so just test the basic operations
    assert_eq!(table.len(), 0);
    table.clear();
    assert!(table.is_empty());
}
