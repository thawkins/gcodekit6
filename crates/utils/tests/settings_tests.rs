use gcodekit_utils::settings::network_timeout;
use std::env;

#[test]
fn test_network_timeout_default() {
    env::remove_var("GCK_NETWORK_TIMEOUT_SECS");
    let d = network_timeout();
    assert_eq!(d.as_secs(), 30);
}

#[test]
fn test_network_timeout_env_override() {
    env::set_var("GCK_NETWORK_TIMEOUT_SECS", "5");
    let d = network_timeout();
    assert_eq!(d.as_secs(), 5);
    env::remove_var("GCK_NETWORK_TIMEOUT_SECS");
}
