//! Session configuration.

use prx_voice_state::machine::StateMachineConfig;
use prx_voice_state::timer::TimeoutConfig;
use serde::{Deserialize, Serialize};

/// Full session configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionConfig {
    /// State machine config.
    pub state_machine: StateMachineConfig,
    /// Timeout config.
    pub timeouts: TimeoutConfig,
    /// Channel type.
    pub channel: String,
    /// Session direction.
    pub direction: String,
    /// Language code.
    pub language: String,
    /// VAD sensitivity (0.0 - 1.0).
    pub vad_sensitivity: f64,
    /// Max session duration in seconds.
    pub max_duration_sec: u64,
    /// Max number of turns.
    pub max_turns: u32,
}

impl Default for SessionConfig {
    fn default() -> Self {
        Self {
            state_machine: StateMachineConfig::default(),
            timeouts: TimeoutConfig::default(),
            channel: "web_console".into(),
            direction: "inbound".into(),
            language: "en-US".into(),
            vad_sensitivity: 0.5,
            max_duration_sec: 1800,
            max_turns: 100,
        }
    }
}
