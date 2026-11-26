//! Moving an override to an exact percentage one confirmed step at a time (GTK's "Set %").

use std::time::{Duration, Instant};

use gcodekit5_app::machine::RealtimeCommand;
use gcodekit5_app::override_target::{
    MAX_STEPS, OverrideKind, OverrideTarget, STEP_TIMEOUT, TargetStep,
};
use gcodekit5_communication::firmware::grbl::status_parser::OverrideState;

fn ov(feed: u16, spindle: u16) -> OverrideState {
    OverrideState {
        feed,
        rapid: 100,
        spindle,
    }
}

/// Runs `target` against a controller that applies every step, starting at `feed`/`spindle`;
/// returns the commands sent and the final step.
fn run(mut target: OverrideTarget, mut state: OverrideState) -> (Vec<RealtimeCommand>, TargetStep) {
    let now = Instant::now();
    let mut sent = Vec::new();
    for _ in 0..100 {
        match target.step(Some(&state), now) {
            TargetStep::Send(command) => {
                sent.push(command);
                let apply = |v: u16| -> u16 {
                    use RealtimeCommand as R;
                    match command {
                        R::FeedOverrideReset | R::SpindleOverrideReset => 100,
                        R::FeedOverridePlus10 | R::SpindleOverridePlus10 => v + 10,
                        R::FeedOverrideMinus10 | R::SpindleOverrideMinus10 => v - 10,
                        R::FeedOverridePlus1 | R::SpindleOverridePlus1 => v + 1,
                        R::FeedOverrideMinus1 | R::SpindleOverrideMinus1 => v - 1,
                        _ => v,
                    }
                };
                match target.kind() {
                    OverrideKind::Feed => state.feed = apply(state.feed),
                    OverrideKind::Spindle => state.spindle = apply(state.spindle),
                }
            }
            other => return (sent, other),
        }
    }
    panic!("never finished");
}

#[test]
fn test_feed_target_reached_with_feed_bytes() {
    let target = OverrideTarget::new(OverrideKind::Feed, 123, Instant::now());
    let (sent, end) = run(target, ov(100, 100));
    assert_eq!(end, TargetStep::Done(123));
    assert_eq!(
        sent,
        [
            RealtimeCommand::FeedOverridePlus10,
            RealtimeCommand::FeedOverridePlus10,
            RealtimeCommand::FeedOverridePlus1,
            RealtimeCommand::FeedOverridePlus1,
            RealtimeCommand::FeedOverridePlus1,
        ]
    );
}

#[test]
fn test_spindle_target_uses_reset_when_shorter() {
    let target = OverrideTarget::new(OverrideKind::Spindle, 95, Instant::now());
    let (sent, end) = run(target, ov(100, 187));
    assert_eq!(end, TargetStep::Done(95));
    assert_eq!(sent[0], RealtimeCommand::SpindleOverrideReset);
    assert!(sent.iter().all(|c| matches!(
        c,
        RealtimeCommand::SpindleOverrideReset
            | RealtimeCommand::SpindleOverrideMinus1
            | RealtimeCommand::SpindleOverrideMinus10
    )));
}

#[test]
fn test_goal_clamped_to_grbl_range() {
    let now = Instant::now();
    assert_eq!(OverrideTarget::new(OverrideKind::Feed, 5, now).goal(), 10);
    assert_eq!(
        OverrideTarget::new(OverrideKind::Feed, 250, now).goal(),
        200
    );
}

#[test]
fn test_next_step_waits_for_the_report_then_fails_after_timeout() {
    let start = Instant::now();
    let mut target = OverrideTarget::new(OverrideKind::Feed, 110, start);
    assert_eq!(
        target.step(Some(&ov(100, 100)), start),
        TargetStep::Send(RealtimeCommand::FeedOverridePlus10)
    );
    // Not confirmed yet: no second byte.
    assert_eq!(target.step(Some(&ov(100, 100)), start), TargetStep::Wait);
    let late = start + STEP_TIMEOUT + Duration::from_millis(1);
    assert_eq!(target.step(Some(&ov(100, 100)), late), TargetStep::Failed);
}

#[test]
fn test_no_report_fails_after_timeout() {
    let start = Instant::now();
    let mut target = OverrideTarget::new(OverrideKind::Feed, 110, start);
    assert_eq!(target.step(None, start), TargetStep::Wait);
    let late = start + STEP_TIMEOUT + Duration::from_millis(1);
    assert_eq!(target.step(None, late), TargetStep::Failed);
}

#[test]
fn test_step_limit_ends_the_sequence() {
    let now = Instant::now();
    let mut target = OverrideTarget::new(OverrideKind::Feed, 200, now);
    // A controller whose value keeps changing but never nears the goal.
    for i in 0..MAX_STEPS {
        let value = if i % 2 == 0 { 50 } else { 51 };
        assert!(matches!(
            target.step(Some(&ov(value, 100)), now),
            TargetStep::Send(_)
        ));
    }
    let value = if MAX_STEPS % 2 == 0 { 50 } else { 51 };
    assert_eq!(target.step(Some(&ov(value, 100)), now), TargetStep::Failed);
}
