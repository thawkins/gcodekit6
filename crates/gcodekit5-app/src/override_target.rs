//! Moving a feed or spindle override to an exact percentage (the "Set %" button).
//!
//! GRBL has no "set override to N %" command, only one-byte steps (reset to 100 %, +/-10 %,
//! +/-1 %), and it may merge identical bytes that arrive back to back. So a target is
//! reached one step at a time: a step is sent only once the controller's `Ov:` report shows
//! that the previous one took effect (the report value differs from the one before it). A
//! step not confirmed within [`STEP_TIMEOUT`], or more than [`MAX_STEPS`] steps, ends the
//! sequence as failed. The step choice is `next_override_step` from
//! `gcodekit5-communication`.
//!
//! The type does no I/O: the front end calls [`OverrideTarget::step`] with the latest
//! reported value and sends the realtime command it returns.
//!
//! Dependencies: `gcodekit5-communication` (override step codes and choice).

use std::time::{Duration, Instant};

use gcodekit5_communication::firmware::grbl::status_parser::OverrideState;
use gcodekit5_communication::firmware::grbl::{
    FEED_OVERRIDE_CODES, MAX_OVERRIDE_PERCENT, MIN_OVERRIDE_PERCENT, SPINDLE_OVERRIDE_CODES,
    next_override_step,
};

use crate::machine::RealtimeCommand;

/// How long the controller has to report a step (and, at the start, any `Ov:` value).
pub const STEP_TIMEOUT: Duration = Duration::from_secs(2);
/// Upper bound on the steps of one target (a reset plus 9 x 10 % and 9 x 1 % is 19).
pub const MAX_STEPS: u32 = 32;

/// Which override a target adjusts.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OverrideKind {
    /// Feed override.
    Feed,
    /// Spindle override.
    Spindle,
}

impl OverrideKind {
    /// The value of this override in a controller's `Ov:` report.
    pub fn reported(self, overrides: &OverrideState) -> u16 {
        match self {
            OverrideKind::Feed => overrides.feed,
            OverrideKind::Spindle => overrides.spindle,
        }
    }

    /// The step from `current` toward `goal`, if they differ.
    fn next(self, current: u16, goal: u16) -> Option<RealtimeCommand> {
        let commands = match self {
            OverrideKind::Feed => [
                RealtimeCommand::FeedOverrideReset,
                RealtimeCommand::FeedOverridePlus10,
                RealtimeCommand::FeedOverrideMinus10,
                RealtimeCommand::FeedOverridePlus1,
                RealtimeCommand::FeedOverrideMinus1,
            ],
            OverrideKind::Spindle => [
                RealtimeCommand::SpindleOverrideReset,
                RealtimeCommand::SpindleOverridePlus10,
                RealtimeCommand::SpindleOverrideMinus10,
                RealtimeCommand::SpindleOverridePlus1,
                RealtimeCommand::SpindleOverrideMinus1,
            ],
        };
        let codes = match self {
            OverrideKind::Feed => FEED_OVERRIDE_CODES,
            OverrideKind::Spindle => SPINDLE_OVERRIDE_CODES,
        };
        let byte = next_override_step(current, goal, codes)?;
        commands.into_iter().find(|c| c.byte() == byte)
    }
}

/// What the front end does next for a target.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TargetStep {
    /// Send this realtime command.
    Send(RealtimeCommand),
    /// Wait for the controller's next report.
    Wait,
    /// The override is at the target (the reported value).
    Done(u16),
    /// The controller did not confirm a step in time, or the step limit was reached.
    Failed,
}

/// An override on its way to a target percentage.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OverrideTarget {
    kind: OverrideKind,
    goal: u16,
    /// The value before the last step, and when the controller must have reported a change.
    awaiting: Option<(u16, Instant)>,
    /// When a first `Ov:` value must have been reported.
    first_report_by: Instant,
    steps: u32,
}

impl OverrideTarget {
    /// A target of `goal_percent` for `kind`, clamped to GRBL's 10-200 %.
    ///
    /// # Arguments
    /// * `kind` - The override to adjust.
    /// * `goal_percent` - The percentage to reach.
    /// * `now` - The current time.
    pub fn new(kind: OverrideKind, goal_percent: u16, now: Instant) -> Self {
        Self {
            kind,
            goal: goal_percent.clamp(MIN_OVERRIDE_PERCENT, MAX_OVERRIDE_PERCENT),
            awaiting: None,
            first_report_by: now + STEP_TIMEOUT,
            steps: 0,
        }
    }

    /// The override this target adjusts.
    pub fn kind(&self) -> OverrideKind {
        self.kind
    }

    /// The percentage to reach.
    pub fn goal(&self) -> u16 {
        self.goal
    }

    /// The next thing to do, given the controller's last reported overrides.
    ///
    /// # Arguments
    /// * `reported` - The last `Ov:` report, if any arrived.
    /// * `now` - The current time.
    pub fn step(&mut self, reported: Option<&OverrideState>, now: Instant) -> TargetStep {
        let Some(current) = reported.map(|o| self.kind.reported(o)) else {
            return if now < self.first_report_by {
                TargetStep::Wait
            } else {
                TargetStep::Failed
            };
        };
        if let Some((before, deadline)) = self.awaiting {
            if current == before {
                return if now < deadline {
                    TargetStep::Wait
                } else {
                    TargetStep::Failed
                };
            }
            self.awaiting = None;
        }
        let Some(command) = self.kind.next(current, self.goal) else {
            return TargetStep::Done(current);
        };
        if self.steps >= MAX_STEPS {
            return TargetStep::Failed;
        }
        self.steps += 1;
        self.awaiting = Some((current, now + STEP_TIMEOUT));
        TargetStep::Send(command)
    }
}
