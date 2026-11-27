//! Locks the single owner of mm-per-inch and the feed-rate conversions.

use gcodekit6_core::units::{
    FeedRateUnits, MM_PER_INCH, MM_PER_INCH_F32, feed_rate_from_mm_per_min, feed_rate_to_mm_per_min,
};

#[test]
fn test_mm_per_inch_constants_agree() {
    assert_eq!(MM_PER_INCH, 25.4_f64);
    assert_eq!(MM_PER_INCH_F32, 25.4_f32);
    assert_eq!(MM_PER_INCH as f32, MM_PER_INCH_F32);
}

#[test]
fn test_feed_rate_conversions_use_the_constant() {
    assert_eq!(
        feed_rate_from_mm_per_min(254.0, FeedRateUnits::InPerMin),
        254.0 / MM_PER_INCH
    );
    assert_eq!(
        feed_rate_from_mm_per_min(254.0, FeedRateUnits::InPerSec),
        254.0 / MM_PER_INCH / 60.0
    );
    assert_eq!(
        feed_rate_to_mm_per_min(10.0, FeedRateUnits::InPerMin),
        10.0 * MM_PER_INCH
    );
    assert_eq!(
        feed_rate_to_mm_per_min(2.0, FeedRateUnits::InPerSec),
        2.0 * MM_PER_INCH * 60.0
    );
    assert_eq!(
        feed_rate_from_mm_per_min(120.0, FeedRateUnits::MmPerSec),
        2.0
    );
    assert_eq!(feed_rate_to_mm_per_min(2.0, FeedRateUnits::MmPerSec), 120.0);
}
