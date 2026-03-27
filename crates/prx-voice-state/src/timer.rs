//! Per-state timeout configuration.
//!
//! From the state machine spec timeout table.
//! Configurable timeouts have min/max bounds; non-configurable are fixed.

use crate::transition::SessionState;
use serde::{Deserialize, Serialize};
use std::time::Duration;

/// Timeout configuration for a single session.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TimeoutConfig {
    /// Idle state timeout. Fixed 60s.
    pub idle: Duration,
    /// Connecting timeout. Default 10s, range 5-30s.
    pub connecting: Duration,
    /// Listening (no speech) timeout. Default 30s, range 10-120s.
    pub listening_idle: Duration,
    /// Max utterance duration. Default 60s, range 10-300s.
    pub max_utterance: Duration,
    /// ASR processing timeout. Default 10s, range 5-30s.
    pub asr_processing: Duration,
    /// Agent thinking timeout. Default 30s, range 10-60s.
    pub thinking: Duration,
    /// Max TTS playback duration. Default 300s, range 30-600s.
    pub speaking_max: Duration,
    /// Interrupt resolution timeout. Fixed 5s.
    pub interrupt_resolution: Duration,
    /// Pause timeout. Default 300s, range 60-1800s.
    pub pause: Duration,
    /// Handoff pending timeout. Default 120s, range 30-600s.
    pub handoff_pending: Duration,
}

impl Default for TimeoutConfig {
    fn default() -> Self {
        Self {
            idle: Duration::from_secs(60),
            connecting: Duration::from_secs(10),
            listening_idle: Duration::from_secs(30),
            max_utterance: Duration::from_secs(60),
            asr_processing: Duration::from_secs(10),
            thinking: Duration::from_secs(30),
            speaking_max: Duration::from_secs(300),
            interrupt_resolution: Duration::from_secs(5),
            pause: Duration::from_secs(300),
            handoff_pending: Duration::from_secs(120),
        }
    }
}

impl TimeoutConfig {
    /// Clamp all configurable timeouts to their spec-defined bounds.
    pub fn clamp_to_bounds(&mut self) {
        // Fixed values (not configurable)
        self.idle = Duration::from_secs(60);
        self.interrupt_resolution = Duration::from_secs(5);

        self.connecting = clamp(self.connecting, 5, 30);
        self.listening_idle = clamp(self.listening_idle, 10, 120);
        self.max_utterance = clamp(self.max_utterance, 10, 300);
        self.asr_processing = clamp(self.asr_processing, 5, 30);
        self.thinking = clamp(self.thinking, 10, 60);
        self.speaking_max = clamp(self.speaking_max, 30, 600);
        self.pause = clamp(self.pause, 60, 1800);
        self.handoff_pending = clamp(self.handoff_pending, 30, 600);
    }

    /// Get the timeout duration for a given state.
    pub fn timeout_for_state(&self, state: SessionState) -> Option<Duration> {
        match state {
            SessionState::Idle => Some(self.idle),
            SessionState::Connecting => Some(self.connecting),
            SessionState::Listening => Some(self.listening_idle),
            SessionState::UserSpeaking => Some(self.max_utterance),
            SessionState::AsrProcessing => Some(self.asr_processing),
            SessionState::Thinking => Some(self.thinking),
            SessionState::Speaking => Some(self.speaking_max),
            SessionState::Interrupted => Some(self.interrupt_resolution),
            SessionState::Paused => Some(self.pause),
            SessionState::HandoffPending => Some(self.handoff_pending),
            SessionState::Closed | SessionState::Failed => None,
        }
    }
}

fn clamp(val: Duration, min_secs: u64, max_secs: u64) -> Duration {
    let min = Duration::from_secs(min_secs);
    let max = Duration::from_secs(max_secs);
    if val < min {
        min
    } else if val > max {
        max
    } else {
        val
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_timeouts_match_spec() {
        let cfg = TimeoutConfig::default();
        assert_eq!(cfg.idle, Duration::from_secs(60));
        assert_eq!(cfg.connecting, Duration::from_secs(10));
        assert_eq!(cfg.interrupt_resolution, Duration::from_secs(5));
        assert_eq!(cfg.pause, Duration::from_secs(300));
    }

    #[test]
    fn clamp_enforces_bounds() {
        let mut cfg = TimeoutConfig {
            connecting: Duration::from_secs(1), // below min 5
            thinking: Duration::from_secs(999), // above max 60
            ..Default::default()
        };
        cfg.clamp_to_bounds();
        assert_eq!(cfg.connecting, Duration::from_secs(5));
        assert_eq!(cfg.thinking, Duration::from_secs(60));
    }

    #[test]
    fn timeout_for_terminal_is_none() {
        let cfg = TimeoutConfig::default();
        assert!(cfg.timeout_for_state(SessionState::Closed).is_none());
        assert!(cfg.timeout_for_state(SessionState::Failed).is_none());
    }

    #[test]
    fn timeout_for_listening() {
        let cfg = TimeoutConfig::default();
        assert_eq!(
            cfg.timeout_for_state(SessionState::Listening),
            Some(Duration::from_secs(30))
        );
    }
}
