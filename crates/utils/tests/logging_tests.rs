use gcodekit_utils::logging;

#[test]
fn test_init_logging_does_not_panic() {
    // Calling init_logging twice in the same process will return an error
    // if the subscriber was already set; we ignore that and treat it as
    // success for test purposes.
    let _ = logging::init_logging();
}
