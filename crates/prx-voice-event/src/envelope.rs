//! CloudEvents v1.0 envelope with PRX Voice extensions.

use chrono::{DateTime, Utc};
use prx_voice_types::ids::{EventId, SessionId, SpanId, TenantId, TraceId, TurnId};
use serde::{Deserialize, Serialize};

/// CloudEvents v1.0 + PRX Voice extension envelope.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VoiceEvent {
    // --- CloudEvents standard ---
    /// Fixed: "1.0"
    pub specversion: String,
    /// Globally unique event ID (for deduplication).
    pub id: EventId,
    /// Source component: "prx-voice/{component}"
    pub source: String,
    /// Event type: "prx.voice.{category}.{event_name}"
    #[serde(rename = "type")]
    pub event_type: String,
    /// Subject: session_id
    pub subject: String,
    /// Wall-clock timestamp (ISO 8601 UTC).
    pub time: DateTime<Utc>,
    /// Fixed: "application/json"
    pub datacontenttype: String,

    // --- PRX Voice extensions ---
    /// Schema version for this event.
    pub prx_event_version: String,
    /// Tenant ID.
    pub prx_tenant_id: TenantId,
    /// Session ID.
    pub prx_session_id: SessionId,
    /// Turn ID (monotonically increasing within session).
    pub prx_turn_id: TurnId,
    /// Session-level monotonic sequence number.
    pub prx_seq: u64,
    /// Distributed trace ID.
    pub prx_trace_id: TraceId,
    /// Span ID within trace.
    pub prx_span_id: SpanId,
    /// Monotonic nanosecond timestamp (for latency measurement).
    pub prx_ts_mono_ns: u64,
    /// Wall-clock UTC (same as `time`, kept for explicit PRX field).
    pub prx_ts_wall_utc: DateTime<Utc>,
    /// Severity level.
    pub prx_severity: Severity,

    /// Event-specific payload.
    pub data: serde_json::Value,
}

/// Event severity level.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Severity {
    Debug,
    Info,
    Warn,
    Error,
    Critical,
}

impl VoiceEvent {
    /// Create a new event with standard defaults filled in.
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        source: impl Into<String>,
        event_type: impl Into<String>,
        tenant_id: TenantId,
        session_id: SessionId,
        turn_id: TurnId,
        seq: u64,
        trace_id: TraceId,
        severity: Severity,
        data: serde_json::Value,
    ) -> Self {
        let now = Utc::now();
        Self {
            specversion: "1.0".into(),
            id: EventId::new(),
            source: source.into(),
            event_type: event_type.into(),
            subject: session_id.to_string(),
            time: now,
            datacontenttype: "application/json".into(),
            prx_event_version: "1.0".into(),
            prx_tenant_id: tenant_id,
            prx_session_id: session_id,
            prx_turn_id: turn_id,
            prx_seq: seq,
            prx_trace_id: trace_id,
            prx_span_id: SpanId::new(),
            prx_ts_mono_ns: monotonic_ns(),
            prx_ts_wall_utc: now,
            prx_severity: severity,
            data,
        }
    }

    /// Event category extracted from type (e.g., "session" from "prx.voice.session.created").
    pub fn category(&self) -> Option<&str> {
        self.event_type
            .strip_prefix("prx.voice.")?
            .split('.')
            .next()
    }
}

/// Get monotonic nanosecond timestamp.
fn monotonic_ns() -> u64 {
    use std::sync::OnceLock;
    use std::time::Instant;

    static EPOCH: OnceLock<Instant> = OnceLock::new();
    let epoch = EPOCH.get_or_init(Instant::now);
    epoch.elapsed().as_nanos() as u64
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn event_serializes_to_json() {
        let evt = VoiceEvent::new(
            "prx-voice/session",
            "prx.voice.session.created",
            TenantId::new(),
            SessionId::new(),
            TurnId::first(),
            1,
            TraceId::new(),
            Severity::Info,
            serde_json::json!({"session_id": "test"}),
        );

        let json = serde_json::to_string_pretty(&evt).unwrap();
        assert!(json.contains("prx.voice.session.created"));
        assert!(json.contains("\"specversion\": \"1.0\""));
    }

    #[test]
    fn category_extraction() {
        let evt = VoiceEvent::new(
            "prx-voice/asr",
            "prx.voice.asr.transcript_final",
            TenantId::new(),
            SessionId::new(),
            TurnId::first(),
            5,
            TraceId::new(),
            Severity::Info,
            serde_json::json!({}),
        );
        assert_eq!(evt.category(), Some("asr"));
    }

    #[test]
    fn seq_is_preserved() {
        let evt = VoiceEvent::new(
            "prx-voice/test",
            "prx.voice.test.foo",
            TenantId::new(),
            SessionId::new(),
            TurnId::first(),
            42,
            TraceId::new(),
            Severity::Debug,
            serde_json::json!({}),
        );
        assert_eq!(evt.prx_seq, 42);
    }
}
