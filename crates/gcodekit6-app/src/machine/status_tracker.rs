//! Status-query tracking on the worker: tagging `?` queries, timing replies, and forgetting
//! queries whose replies were lost.

use std::time::{Duration, Instant};

use super::types::RealtimeCommand;
use super::worker::WorkerState;
use super::{HALT_MAX_WAIT, MAX_QUERY_TAGS, MAX_UNANSWERED_QUERIES, QUERY_REPLY_TIMEOUT};

impl WorkerState {
    /// Writes a `?` and tags it with the state lines written so far.
    pub(super) fn write_status_query(&mut self) -> Result<(), String> {
        self.write(&[RealtimeCommand::StatusQuery.byte()])?;
        if self.query_tags.len() >= MAX_QUERY_TAGS
            && let Some(oldest) = self.query_tags.pop_front()
            && let Some(next) = self.query_tags.front_mut()
        {
            *next = (*next).min(oldest);
        }
        self.query_tags.push_back(self.written_seq);
        self.forget_lost_queries();
        if self.unanswered_queries.len() >= MAX_UNANSWERED_QUERIES {
            self.unanswered_queries.pop_front();
        }
        self.unanswered_queries.push_back(Instant::now());
        Ok(())
    }

    /// How long a `?` may stay unanswered before its reply is presumed lost: twice the
    /// slowest recent reply, at least [`QUERY_REPLY_TIMEOUT`], at most [`HALT_MAX_WAIT`].
    fn lost_reply_timeout(&self) -> Duration {
        (self.reply_latency * 2).clamp(QUERY_REPLY_TIMEOUT, HALT_MAX_WAIT)
    }

    /// Drops the unanswered `?`s older than [`Self::lost_reply_timeout`]: their replies are
    /// presumed lost. Logged, once per silence (or always while halting).
    pub(super) fn forget_lost_queries(&mut self) {
        let now = Instant::now();
        let timeout = self.lost_reply_timeout();
        let before = self.unanswered_queries.len();
        while self
            .unanswered_queries
            .front()
            .is_some_and(|at| now.duration_since(*at) >= timeout)
        {
            self.unanswered_queries.pop_front();
        }
        let forgotten = before - self.unanswered_queries.len();
        if forgotten > 0 && (self.halt.is_some() || !self.lost_logged) {
            tracing::warn!(
                forgotten,
                ?timeout,
                halting = self.halt.is_some(),
                "status replies presumed lost"
            );
            self.lost_logged = true;
        }
    }

    /// Takes the oldest unanswered `?` as answered by a status report, and learns the reply
    /// latency from it.
    pub(super) fn answer_query(&mut self) {
        self.lost_logged = false;
        let decayed = self.reply_latency.saturating_sub(self.reply_latency / 16);
        if let Some(at) = self.unanswered_queries.pop_front() {
            self.reply_latency = decayed.max(at.elapsed());
        } else {
            self.reply_latency = decayed;
        }
    }
}
