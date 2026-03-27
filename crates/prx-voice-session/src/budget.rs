//! Per-session resource budget enforcement.
//!
//! From the production plan: each session has bounded memory budgets for
//! input buffers, partial transcripts, agent output, TTS segments, and traces.

use serde::{Deserialize, Serialize};
use std::sync::atomic::{AtomicU64, Ordering};

/// Resource budget configuration for a single session.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResourceBudget {
    /// Max input audio buffer size (bytes).
    pub max_input_buffer_bytes: u64,
    /// Max partial transcript buffer (bytes).
    pub max_partial_transcript_bytes: u64,
    /// Max agent pending output (bytes).
    pub max_agent_output_bytes: u64,
    /// Max TTS queued segments.
    pub max_tts_queued_segments: u32,
    /// Max events retained in session trace.
    pub max_trace_events: u32,
    /// Max conversation history turns retained.
    pub max_history_turns: u32,
}

impl Default for ResourceBudget {
    fn default() -> Self {
        Self {
            max_input_buffer_bytes: 1024 * 1024,     // 1 MB
            max_partial_transcript_bytes: 64 * 1024, // 64 KB
            max_agent_output_bytes: 256 * 1024,      // 256 KB
            max_tts_queued_segments: 10,
            max_trace_events: 1000,
            max_history_turns: 50,
        }
    }
}

/// Policy when a budget limit is exceeded.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum OverLimitPolicy {
    /// Discard oldest data.
    Discard,
    /// Truncate to fit.
    Truncate,
    /// Degrade service quality.
    Degrade,
    /// End the session.
    EndSession,
    /// Request human handoff.
    Handoff,
}

/// Tracks resource usage against budget.
#[derive(Debug)]
pub struct ResourceTracker {
    budget: ResourceBudget,
    input_buffer_used: AtomicU64,
    partial_transcript_used: AtomicU64,
    agent_output_used: AtomicU64,
    tts_segments_queued: AtomicU64,
    trace_events_count: AtomicU64,
    history_turns: AtomicU64,
}

/// Result of a budget check.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BudgetCheckResult {
    /// Within budget.
    Ok,
    /// Over limit for the specified resource.
    OverLimit {
        resource: String,
        current: u64,
        limit: u64,
    },
}

impl ResourceTracker {
    /// Create a new tracker with the given budget.
    pub fn new(budget: ResourceBudget) -> Self {
        Self {
            budget,
            input_buffer_used: AtomicU64::new(0),
            partial_transcript_used: AtomicU64::new(0),
            agent_output_used: AtomicU64::new(0),
            tts_segments_queued: AtomicU64::new(0),
            trace_events_count: AtomicU64::new(0),
            history_turns: AtomicU64::new(0),
        }
    }

    /// Record input buffer usage. Returns check result.
    pub fn record_input_buffer(&self, bytes: u64) -> BudgetCheckResult {
        let new_val = self.input_buffer_used.fetch_add(bytes, Ordering::Relaxed) + bytes;
        if new_val > self.budget.max_input_buffer_bytes {
            BudgetCheckResult::OverLimit {
                resource: "input_buffer".into(),
                current: new_val,
                limit: self.budget.max_input_buffer_bytes,
            }
        } else {
            BudgetCheckResult::Ok
        }
    }

    /// Release input buffer usage.
    pub fn release_input_buffer(&self, bytes: u64) {
        self.input_buffer_used.fetch_sub(
            bytes.min(self.input_buffer_used.load(Ordering::Relaxed)),
            Ordering::Relaxed,
        );
    }

    /// Record partial transcript usage.
    pub fn record_partial_transcript(&self, bytes: u64) -> BudgetCheckResult {
        let new_val = self
            .partial_transcript_used
            .fetch_add(bytes, Ordering::Relaxed)
            + bytes;
        if new_val > self.budget.max_partial_transcript_bytes {
            BudgetCheckResult::OverLimit {
                resource: "partial_transcript".into(),
                current: new_val,
                limit: self.budget.max_partial_transcript_bytes,
            }
        } else {
            BudgetCheckResult::Ok
        }
    }

    /// Reset partial transcript (e.g., after final transcript received).
    pub fn reset_partial_transcript(&self) {
        self.partial_transcript_used.store(0, Ordering::Relaxed);
    }

    /// Record agent output bytes.
    pub fn record_agent_output(&self, bytes: u64) -> BudgetCheckResult {
        let new_val = self.agent_output_used.fetch_add(bytes, Ordering::Relaxed) + bytes;
        if new_val > self.budget.max_agent_output_bytes {
            BudgetCheckResult::OverLimit {
                resource: "agent_output".into(),
                current: new_val,
                limit: self.budget.max_agent_output_bytes,
            }
        } else {
            BudgetCheckResult::Ok
        }
    }

    /// Reset agent output (after turn completes).
    pub fn reset_agent_output(&self) {
        self.agent_output_used.store(0, Ordering::Relaxed);
    }

    /// Record a TTS segment queued.
    pub fn record_tts_segment(&self) -> BudgetCheckResult {
        let new_val = self.tts_segments_queued.fetch_add(1, Ordering::Relaxed) + 1;
        if new_val > self.budget.max_tts_queued_segments as u64 {
            BudgetCheckResult::OverLimit {
                resource: "tts_segments".into(),
                current: new_val,
                limit: self.budget.max_tts_queued_segments as u64,
            }
        } else {
            BudgetCheckResult::Ok
        }
    }

    /// Release TTS segments (after playback or flush).
    pub fn release_tts_segments(&self, count: u64) {
        self.tts_segments_queued.fetch_sub(
            count.min(self.tts_segments_queued.load(Ordering::Relaxed)),
            Ordering::Relaxed,
        );
    }

    /// Record a trace event.
    pub fn record_trace_event(&self) -> BudgetCheckResult {
        let new_val = self.trace_events_count.fetch_add(1, Ordering::Relaxed) + 1;
        if new_val > self.budget.max_trace_events as u64 {
            BudgetCheckResult::OverLimit {
                resource: "trace_events".into(),
                current: new_val,
                limit: self.budget.max_trace_events as u64,
            }
        } else {
            BudgetCheckResult::Ok
        }
    }

    /// Record a conversation history turn.
    pub fn record_history_turn(&self) -> BudgetCheckResult {
        let new_val = self.history_turns.fetch_add(1, Ordering::Relaxed) + 1;
        if new_val > self.budget.max_history_turns as u64 {
            BudgetCheckResult::OverLimit {
                resource: "history_turns".into(),
                current: new_val,
                limit: self.budget.max_history_turns as u64,
            }
        } else {
            BudgetCheckResult::Ok
        }
    }

    /// Get current usage snapshot.
    pub fn snapshot(&self) -> ResourceSnapshot {
        ResourceSnapshot {
            input_buffer_bytes: self.input_buffer_used.load(Ordering::Relaxed),
            partial_transcript_bytes: self.partial_transcript_used.load(Ordering::Relaxed),
            agent_output_bytes: self.agent_output_used.load(Ordering::Relaxed),
            tts_segments_queued: self.tts_segments_queued.load(Ordering::Relaxed),
            trace_events: self.trace_events_count.load(Ordering::Relaxed),
            history_turns: self.history_turns.load(Ordering::Relaxed),
        }
    }
}

/// Snapshot of current resource usage.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResourceSnapshot {
    pub input_buffer_bytes: u64,
    pub partial_transcript_bytes: u64,
    pub agent_output_bytes: u64,
    pub tts_segments_queued: u64,
    pub trace_events: u64,
    pub history_turns: u64,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn within_budget() {
        let tracker = ResourceTracker::new(ResourceBudget::default());
        assert_eq!(tracker.record_input_buffer(1000), BudgetCheckResult::Ok);
        assert_eq!(tracker.record_trace_event(), BudgetCheckResult::Ok);
    }

    #[test]
    fn over_limit_detected() {
        let tracker = ResourceTracker::new(ResourceBudget {
            max_tts_queued_segments: 2,
            ..Default::default()
        });
        assert_eq!(tracker.record_tts_segment(), BudgetCheckResult::Ok);
        assert_eq!(tracker.record_tts_segment(), BudgetCheckResult::Ok);
        assert!(matches!(
            tracker.record_tts_segment(),
            BudgetCheckResult::OverLimit { .. }
        ));
    }

    #[test]
    fn release_frees_budget() {
        let tracker = ResourceTracker::new(ResourceBudget {
            max_input_buffer_bytes: 100,
            ..Default::default()
        });
        assert_eq!(tracker.record_input_buffer(80), BudgetCheckResult::Ok);
        tracker.release_input_buffer(80);
        assert_eq!(tracker.record_input_buffer(80), BudgetCheckResult::Ok);
    }

    #[test]
    fn reset_clears_usage() {
        let tracker = ResourceTracker::new(ResourceBudget {
            max_partial_transcript_bytes: 100,
            ..Default::default()
        });
        tracker.record_partial_transcript(90);
        tracker.reset_partial_transcript();
        let snap = tracker.snapshot();
        assert_eq!(snap.partial_transcript_bytes, 0);
    }

    #[test]
    fn snapshot_reflects_usage() {
        let tracker = ResourceTracker::new(ResourceBudget::default());
        tracker.record_input_buffer(500);
        tracker.record_trace_event();
        tracker.record_trace_event();
        tracker.record_history_turn();

        let snap = tracker.snapshot();
        assert_eq!(snap.input_buffer_bytes, 500);
        assert_eq!(snap.trace_events, 2);
        assert_eq!(snap.history_turns, 1);
    }
}
