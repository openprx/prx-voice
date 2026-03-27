//! Typed event payloads for all PRX Voice event categories.

use prx_voice_types::error::{
    AgentErrorCode, AsrErrorCode, AudioDropCause, SessionFailureCode, TtsErrorCode,
};
use serde::{Deserialize, Serialize};

// ── Session Events ──

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionCreatedPayload {
    pub session_id: String,
    pub tenant_id: String,
    pub channel: String,
    pub direction: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub from_uri: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub to_uri: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionStateChangedPayload {
    pub previous_state: String,
    pub new_state: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub trigger_event_id: Option<String>,
    pub trigger_reason: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionClosedPayload {
    pub reason: String,
    pub close_code: u16,
    pub initiated_by: String,
    pub duration_ms: u64,
    pub total_turns: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionFailedPayload {
    pub failure_code: SessionFailureCode,
    pub failure_stage: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub provider: Option<String>,
    pub recoverable: bool,
    pub message: String,
}

// ── Media Events ──

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VadStartedPayload {
    pub audio_offset_ms: u64,
    pub energy_db: f64,
    pub vad_model: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VadEndedPayload {
    pub audio_offset_ms: u64,
    pub speech_duration_ms: u64,
    pub silence_duration_ms: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AudioDroppedPayload {
    pub cause_code: AudioDropCause,
    pub dropped_frames: u32,
    pub dropped_duration_ms: u64,
    pub buffer_watermark_pct: u8,
}

// ── ASR Events ──

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TranscriptPartialPayload {
    pub speech_id: String,
    pub transcript: String,
    pub confidence: f64,
    pub stability: f64,
    pub language: String,
    pub is_final: bool,
    pub revision: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TranscriptFinalPayload {
    pub speech_id: String,
    pub transcript: String,
    pub confidence: f64,
    pub language: String,
    pub asr_latency_ms: u64,
    pub audio_duration_ms: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AsrErrorPayload {
    pub error_code: AsrErrorCode,
    pub provider: String,
    pub model: String,
    pub message: String,
    pub retryable: bool,
}

// ── Agent Events ──

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentThinkingStartedPayload {
    pub provider: String,
    pub model: String,
    pub prompt_tokens: u32,
    pub input_transcript: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentTokenStreamPayload {
    pub token_index: u32,
    pub token: String,
    pub cumulative_text: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub finish_reason: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentResponseCompletePayload {
    pub response_text: String,
    pub prompt_tokens: u32,
    pub completion_tokens: u32,
    pub total_tokens: u32,
    pub first_token_latency_ms: u64,
    pub total_latency_ms: u64,
    pub finish_reason: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentErrorPayload {
    pub error_code: AgentErrorCode,
    pub provider: String,
    pub model: String,
    pub message: String,
    pub retryable: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub retry_after_ms: Option<u64>,
}

// ── TTS Events ──

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TtsSegmentQueuedPayload {
    pub segment_id: String,
    pub text: String,
    pub provider: String,
    pub voice: String,
    pub estimated_duration_ms: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TtsChunkReadyPayload {
    pub segment_id: String,
    pub chunk_index: u32,
    pub audio_length_ms: u64,
    pub encoding: String,
    pub sample_rate: u32,
    pub is_final: bool,
    pub synthesis_latency_ms: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TtsStoppedPayload {
    pub segment_id: String,
    pub reason: String,
    pub played_duration_ms: u64,
    pub total_duration_ms: u64,
    pub completion_pct: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TtsErrorPayload {
    pub error_code: TtsErrorCode,
    pub provider: String,
    pub voice: String,
    pub message: String,
    pub retryable: bool,
}

// ── Playback Events ──

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PlaybackStartedPayload {
    pub segment_id: String,
    pub buffer_depth_ms: u64,
    pub first_byte_latency_ms: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PlaybackCompletedPayload {
    pub segment_id: String,
    pub duration_ms: u64,
    pub interrupted: bool,
    pub underrun_count: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PlaybackFlushedPayload {
    pub flushed_segments: Vec<String>,
    pub flushed_duration_ms: u64,
    pub reason: String,
}

// ── Interrupt Events ──

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InterruptIssuedPayload {
    pub interrupted_turn_id: u32,
    pub interrupted_segment_id: String,
    pub trigger: String,
    pub playback_position_ms: u64,
    pub playback_completion_pct: f64,
    pub new_speech_id: String,
}

// ── Governance Events ──

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QuotaConsumedPayload {
    pub resource_type: String,
    pub quantity: u64,
    pub unit: String,
    pub quota_remaining: u64,
    pub quota_limit: u64,
    pub billing_period: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QuotaExceededPayload {
    pub resource_type: String,
    pub current_value: u64,
    pub limit: u64,
    pub enforcement_action: String,
}

/// All known event type constants.
pub mod event_types {
    // Session
    pub const SESSION_CREATED: &str = "prx.voice.session.created";
    pub const SESSION_STATE_CHANGED: &str = "prx.voice.session.state_changed";
    pub const SESSION_CLOSED: &str = "prx.voice.session.closed";
    pub const SESSION_FAILED: &str = "prx.voice.session.failed";

    // Media
    pub const VAD_STARTED: &str = "prx.voice.media.vad_started";
    pub const VAD_ENDED: &str = "prx.voice.media.vad_ended";
    pub const AUDIO_DROPPED: &str = "prx.voice.media.audio_dropped";

    // ASR
    pub const TRANSCRIPT_PARTIAL: &str = "prx.voice.asr.transcript_partial";
    pub const TRANSCRIPT_FINAL: &str = "prx.voice.asr.transcript_final";
    pub const ASR_ERROR: &str = "prx.voice.asr.error";

    // Agent
    pub const AGENT_THINKING_STARTED: &str = "prx.voice.agent.thinking_started";
    pub const AGENT_TOKEN_STREAM: &str = "prx.voice.agent.token_stream";
    pub const AGENT_RESPONSE_COMPLETE: &str = "prx.voice.agent.response_complete";
    pub const AGENT_ERROR: &str = "prx.voice.agent.error";

    // TTS
    pub const TTS_SEGMENT_QUEUED: &str = "prx.voice.tts.segment_queued";
    pub const TTS_CHUNK_READY: &str = "prx.voice.tts.chunk_ready";
    pub const TTS_STOPPED: &str = "prx.voice.tts.stopped";
    pub const TTS_ERROR: &str = "prx.voice.tts.error";

    // Playback
    pub const PLAYBACK_STARTED: &str = "prx.voice.playback.started";
    pub const PLAYBACK_COMPLETED: &str = "prx.voice.playback.completed";
    pub const PLAYBACK_FLUSHED: &str = "prx.voice.playback.flushed";

    // Interrupt
    pub const INTERRUPT_ISSUED: &str = "prx.voice.interrupt.issued";

    // Session lifecycle
    pub const SESSION_PAUSED: &str = "prx.voice.session.paused";
    pub const SESSION_RESUMED: &str = "prx.voice.session.resumed";

    // Adapter lifecycle
    pub const ADAPTER_STATE_CHANGED: &str = "prx.voice.adapter.state_changed";
    pub const ADAPTER_HEALTH_CHECK: &str = "prx.voice.adapter.health_check";

    // Governance
    pub const QUOTA_CONSUMED: &str = "prx.voice.governance.quota_consumed";
    pub const QUOTA_EXCEEDED: &str = "prx.voice.governance.quota_exceeded";
    pub const USAGE_RECORDED: &str = "prx.voice.governance.usage_recorded";
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn session_created_payload_serializes() {
        let p = SessionCreatedPayload {
            session_id: "sess-123".into(),
            tenant_id: "tenant-abc".into(),
            channel: "web_console".into(),
            direction: "inbound".into(),
            from_uri: None,
            to_uri: None,
        };
        let json = serde_json::to_value(&p).unwrap();
        assert_eq!(json["channel"], "web_console");
        assert!(json.get("from_uri").is_none()); // skip_serializing_if
    }

    #[test]
    fn transcript_final_roundtrip() {
        let p = TranscriptFinalPayload {
            speech_id: "utt-001".into(),
            transcript: "Hello world".into(),
            confidence: 0.96,
            language: "en-US".into(),
            asr_latency_ms: 320,
            audio_duration_ms: 2300,
        };
        let json = serde_json::to_string(&p).unwrap();
        let back: TranscriptFinalPayload = serde_json::from_str(&json).unwrap();
        assert_eq!(back.transcript, "Hello world");
        assert_eq!(back.confidence, 0.96);
    }

    #[test]
    fn event_type_constants_follow_naming() {
        assert!(event_types::SESSION_CREATED.starts_with("prx.voice."));
        assert!(event_types::TRANSCRIPT_FINAL.starts_with("prx.voice.asr."));
        assert!(event_types::INTERRUPT_ISSUED.starts_with("prx.voice.interrupt."));
    }
}
