//! State transition types and triggers.

use serde::{Deserialize, Serialize};
use std::fmt;

/// The 12 session states.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub enum SessionState {
    Idle,
    Connecting,
    Listening,
    UserSpeaking,
    AsrProcessing,
    Thinking,
    Speaking,
    Interrupted,
    Paused,
    HandoffPending,
    Closed,
    Failed,
}

impl SessionState {
    /// Whether this is a terminal state.
    pub fn is_terminal(self) -> bool {
        matches!(self, Self::Closed | Self::Failed)
    }

    /// Whether this state can be paused (per spec: Listening, UserSpeaking, Speaking).
    pub fn is_pausable(self) -> bool {
        matches!(self, Self::Listening | Self::UserSpeaking | Self::Speaking)
    }

    /// Whether handoff can be requested from this state (per spec: Listening, Speaking, Thinking).
    pub fn is_handoffable(self) -> bool {
        matches!(self, Self::Listening | Self::Speaking | Self::Thinking)
    }
}

impl fmt::Display for SessionState {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Idle => write!(f, "Idle"),
            Self::Connecting => write!(f, "Connecting"),
            Self::Listening => write!(f, "Listening"),
            Self::UserSpeaking => write!(f, "UserSpeaking"),
            Self::AsrProcessing => write!(f, "AsrProcessing"),
            Self::Thinking => write!(f, "Thinking"),
            Self::Speaking => write!(f, "Speaking"),
            Self::Interrupted => write!(f, "Interrupted"),
            Self::Paused => write!(f, "Paused"),
            Self::HandoffPending => write!(f, "HandoffPending"),
            Self::Closed => write!(f, "Closed"),
            Self::Failed => write!(f, "Failed"),
        }
    }
}

/// Events that trigger state transitions.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Trigger {
    /// External request to create session.
    SessionCreate,
    /// All required adapters report ready.
    AdaptersReady,
    /// VAD detects speech energy above threshold.
    VadStarted,
    /// VAD detects silence (endpoint).
    VadEnded,
    /// ASR produces final transcript (non-empty).
    TranscriptFinal { is_empty: bool },
    /// Agent begins streaming response.
    AgentResponseStart,
    /// All TTS segments played back completely.
    PlaybackCompleted,
    /// VAD during Speaking with interrupt_enabled=true.
    InterruptDetected,
    /// Interrupt resolution complete.
    InterruptResolved,
    /// Explicit pause request.
    PauseRequest,
    /// Resume from pause.
    ResumeRequest,
    /// Handoff to human agent requested.
    HandoffRequest,
    /// Handoff confirmed by external system.
    HandoffConfirmed,
    /// Explicit close request.
    CloseRequest { force: bool },
    /// Adapter initialization failed.
    AdapterInitFailed,
    /// Adapter initialization timed out.
    AdapterInitTimeout,
    /// Transport disconnect detected.
    TransportDisconnect,
    /// Resource exhaustion detected.
    ResourceExhaustion,
    /// Unrecoverable error.
    UnrecoverableError { reason: String },
    /// State-specific timeout expired.
    Timeout { state: SessionState },
    /// ASR timeout (no TranscriptFinal in time).
    AsrTimeout,
    /// Agent timeout (no response in time).
    AgentTimeout,
    /// Handoff timed out.
    HandoffTimeout,
    /// Agent decides to handoff.
    AgentHandoffDecision,
    /// TTS error during playback.
    TtsError,
}

/// Result of a state transition attempt.
#[derive(Debug, Clone, PartialEq)]
pub enum TransitionResult {
    /// Transition succeeded.
    Transitioned {
        from: SessionState,
        to: SessionState,
        trigger: Trigger,
    },
    /// Transition was rejected (invalid from current state).
    Rejected {
        current: SessionState,
        trigger: Trigger,
        reason: String,
    },
    /// Already in a terminal state; no transition possible.
    AlreadyTerminal { current: SessionState },
}

impl TransitionResult {
    /// Returns the new state if transition succeeded.
    pub fn new_state(&self) -> Option<SessionState> {
        match self {
            Self::Transitioned { to, .. } => Some(*to),
            _ => None,
        }
    }

    /// Whether the transition succeeded.
    pub fn is_success(&self) -> bool {
        matches!(self, Self::Transitioned { .. })
    }
}
