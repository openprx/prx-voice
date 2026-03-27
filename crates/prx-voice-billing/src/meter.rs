//! Usage metering types per billing spec.

use serde::{Deserialize, Serialize};

/// What resource is being metered.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MeterType {
    SessionDurationMs,
    AsrAudioSeconds,
    AgentInputTokens,
    AgentOutputTokens,
    TtsCharacters,
    TtsAudioSeconds,
    StorageBytes,
    ConcurrentSessions,
    HandoffEvents,
}

/// Unit for the metered resource.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum MeterUnit {
    Milliseconds,
    Seconds,
    Tokens,
    Characters,
    Bytes,
    Count,
}

impl MeterType {
    /// Default unit for this meter type.
    pub fn unit(&self) -> MeterUnit {
        match self {
            Self::SessionDurationMs => MeterUnit::Milliseconds,
            Self::AsrAudioSeconds | Self::TtsAudioSeconds => MeterUnit::Seconds,
            Self::AgentInputTokens | Self::AgentOutputTokens => MeterUnit::Tokens,
            Self::TtsCharacters => MeterUnit::Characters,
            Self::StorageBytes => MeterUnit::Bytes,
            Self::ConcurrentSessions | Self::HandoffEvents => MeterUnit::Count,
        }
    }
}
