//! Unified error codes for the PRX Voice Engine.
//!
//! Error codes are stable identifiers. They follow the pattern:
//! `{CATEGORY}_{SPECIFIC}` — e.g., `ASR_TIMEOUT`, `SESSION_NOT_FOUND`.

use serde::{Deserialize, Serialize};
use std::fmt;

/// Top-level error category.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum ErrorCategory {
    Session,
    Transport,
    Asr,
    Agent,
    Tts,
    Playback,
    Quota,
    Policy,
    Auth,
    Internal,
}

/// Session failure codes (from state machine spec §D / event schema Appendix C).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum SessionFailureCode {
    SessionTimeout,
    AdapterInitTimeout,
    AdapterInitFailed,
    TransportDisconnected,
    TransportTimeout,
    QuotaExceeded,
    PolicyViolation,
    InternalError,
    StateCorruption,
    ResourceExhaustion,
}

/// ASR error codes.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum AsrErrorCode {
    AsrTimeout,
    AsrRateLimited,
    AsrAuthFailed,
    AsrModelUnavailable,
    AsrLanguageUnsupported,
    AsrAudioTooShort,
    AsrInternalError,
}

/// Agent error codes.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum AgentErrorCode {
    AgentTimeout,
    AgentRateLimited,
    AgentTokenLimit,
    AgentContentFiltered,
    AgentContextOverflow,
    AgentAuthFailed,
    AgentInternalError,
}

/// TTS error codes.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum TtsErrorCode {
    TtsSynthesisFailed,
    TtsRateLimited,
    TtsVoiceUnavailable,
    TtsTextTooLong,
    TtsAuthFailed,
    TtsInternalError,
}

/// Audio drop cause codes.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum AudioDropCause {
    BufferOverflow,
    CodecError,
    NetworkJitter,
    SampleRateMismatch,
    SilenceSuppression,
}

/// Whether a failure is recoverable.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct Recoverable(pub bool);

/// A structured error with code, message, and recoverability.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VoiceError {
    pub code: String,
    pub message: String,
    pub retryable: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub retry_after_ms: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub doc_url: Option<String>,
}

impl fmt::Display for VoiceError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "[{}] {}", self.code, self.message)
    }
}

impl std::error::Error for VoiceError {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn session_failure_code_serializes_screaming_snake() {
        let code = SessionFailureCode::AdapterInitTimeout;
        let json = serde_json::to_string(&code).unwrap();
        assert_eq!(json, r#""ADAPTER_INIT_TIMEOUT""#);
    }

    #[test]
    fn asr_error_code_roundtrip() {
        let code = AsrErrorCode::AsrTimeout;
        let json = serde_json::to_string(&code).unwrap();
        let back: AsrErrorCode = serde_json::from_str(&json).unwrap();
        assert_eq!(code, back);
    }

    #[test]
    fn voice_error_display() {
        let err = VoiceError {
            code: "ASR_TIMEOUT".into(),
            message: "ASR streaming timed out".into(),
            retryable: true,
            retry_after_ms: Some(5000),
            doc_url: None,
        };
        assert_eq!(err.to_string(), "[ASR_TIMEOUT] ASR streaming timed out");
    }
}
