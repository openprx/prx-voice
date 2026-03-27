//! Event replay — reconstruct session state from stored events.
//! Per the event schema spec: sessions can be replayed from ordered events.

use crate::envelope::VoiceEvent;
use serde::{Deserialize, Serialize};

/// Replay filter options.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ReplayFilter {
    /// Only include events of these types (empty = all).
    pub event_types: Vec<String>,
    /// Only include events from these categories (empty = all).
    pub categories: Vec<String>,
    /// Start from this sequence number (inclusive).
    pub from_seq: Option<u64>,
    /// End at this sequence number (inclusive).
    pub to_seq: Option<u64>,
    /// Only include events for this turn.
    pub turn_id: Option<u32>,
}

/// A session replay package.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReplayPackage {
    pub session_id: String,
    pub tenant_id: String,
    pub events: Vec<ReplayEvent>,
    pub total_events: usize,
    pub first_seq: u64,
    pub last_seq: u64,
    pub duration_ms: u64,
}

/// A single event in the replay stream.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReplayEvent {
    pub seq: u64,
    pub event_type: String,
    pub category: String,
    pub turn_id: u32,
    pub severity: String,
    pub timestamp: String,
    pub data: serde_json::Value,
    /// Time offset from session start (ms).
    pub offset_ms: u64,
}

/// Build a replay package from a sequence of events.
pub fn build_replay(events: Vec<VoiceEvent>, filter: &ReplayFilter) -> ReplayPackage {
    let session_id = events
        .first()
        .map(|e| e.prx_session_id.to_string())
        .unwrap_or_default();
    let tenant_id = events
        .first()
        .map(|e| e.prx_tenant_id.to_string())
        .unwrap_or_default();

    let start_time = events.first().map(|e| e.prx_ts_mono_ns).unwrap_or(0);

    let filtered: Vec<ReplayEvent> = events
        .iter()
        .filter(|e| {
            if let Some(from) = filter.from_seq {
                if e.prx_seq < from {
                    return false;
                }
            }
            if let Some(to) = filter.to_seq {
                if e.prx_seq > to {
                    return false;
                }
            }
            if let Some(turn) = filter.turn_id {
                if e.prx_turn_id.as_u32() != turn {
                    return false;
                }
            }
            if !filter.event_types.is_empty() && !filter.event_types.contains(&e.event_type) {
                return false;
            }
            if !filter.categories.is_empty() {
                let cat = e.category().unwrap_or("unknown");
                if !filter.categories.iter().any(|c| c == cat) {
                    return false;
                }
            }
            true
        })
        .map(|e| {
            let offset_ms = (e.prx_ts_mono_ns.saturating_sub(start_time)) / 1_000_000;
            ReplayEvent {
                seq: e.prx_seq,
                event_type: e.event_type.clone(),
                category: e.category().unwrap_or("unknown").to_string(),
                turn_id: e.prx_turn_id.as_u32(),
                severity: format!("{:?}", e.prx_severity),
                timestamp: e.time.to_rfc3339(),
                data: e.data.clone(),
                offset_ms,
            }
        })
        .collect();

    let total = filtered.len();
    let first_seq = filtered.first().map(|e| e.seq).unwrap_or(0);
    let last_seq = filtered.last().map(|e| e.seq).unwrap_or(0);
    let duration_ms = filtered.last().map(|e| e.offset_ms).unwrap_or(0);

    ReplayPackage {
        session_id,
        tenant_id,
        events: filtered,
        total_events: total,
        first_seq,
        last_seq,
        duration_ms,
    }
}

/// Validate replay event ordering (monotonic seq, no gaps).
pub fn validate_replay(events: &[ReplayEvent]) -> ReplayValidation {
    let mut issues = Vec::new();
    let mut prev_seq = 0u64;

    for (i, evt) in events.iter().enumerate() {
        if i > 0 {
            if evt.seq <= prev_seq {
                issues.push(format!(
                    "Non-monotonic seq at index {i}: {} <= {prev_seq}",
                    evt.seq
                ));
            }
            if evt.seq > prev_seq + 1 {
                issues.push(format!(
                    "Gap at index {i}: seq jumped from {prev_seq} to {}",
                    evt.seq
                ));
            }
        }
        prev_seq = evt.seq;
    }

    ReplayValidation {
        valid: issues.is_empty(),
        event_count: events.len(),
        issues,
    }
}

/// Replay validation result.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReplayValidation {
    pub valid: bool,
    pub event_count: usize,
    pub issues: Vec<String>,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::envelope::{Severity, VoiceEvent};
    use prx_voice_types::ids::*;

    fn make_events(n: u64) -> Vec<VoiceEvent> {
        let sid = SessionId::new();
        let tid = TenantId::new();
        let trace = TraceId::new();
        (1..=n)
            .map(|seq| {
                VoiceEvent::new(
                    "prx-voice/test",
                    if seq == 1 {
                        "prx.voice.session.created"
                    } else {
                        "prx.voice.asr.transcript_final"
                    },
                    tid,
                    sid,
                    TurnId::first(),
                    seq,
                    trace,
                    Severity::Info,
                    serde_json::json!({"test": seq}),
                )
            })
            .collect()
    }

    #[test]
    fn build_replay_all_events() {
        let events = make_events(5);
        let pkg = build_replay(events, &ReplayFilter::default());
        assert_eq!(pkg.total_events, 5);
        assert_eq!(pkg.first_seq, 1);
        assert_eq!(pkg.last_seq, 5);
    }

    #[test]
    fn build_replay_filtered_by_seq() {
        let events = make_events(10);
        let pkg = build_replay(
            events,
            &ReplayFilter {
                from_seq: Some(3),
                to_seq: Some(7),
                ..Default::default()
            },
        );
        assert_eq!(pkg.total_events, 5);
        assert_eq!(pkg.first_seq, 3);
        assert_eq!(pkg.last_seq, 7);
    }

    #[test]
    fn build_replay_filtered_by_type() {
        let events = make_events(5);
        let pkg = build_replay(
            events,
            &ReplayFilter {
                event_types: vec!["prx.voice.session.created".into()],
                ..Default::default()
            },
        );
        assert_eq!(pkg.total_events, 1);
    }

    #[test]
    fn validate_valid_replay() {
        let events: Vec<ReplayEvent> = (1..=5)
            .map(|i| ReplayEvent {
                seq: i,
                event_type: "test".into(),
                category: "test".into(),
                turn_id: 1,
                severity: "info".into(),
                timestamp: String::new(),
                data: serde_json::json!({}),
                offset_ms: i * 100,
            })
            .collect();
        let v = validate_replay(&events);
        assert!(v.valid);
        assert!(v.issues.is_empty());
    }

    #[test]
    fn validate_detects_gap() {
        let events = vec![
            ReplayEvent {
                seq: 1,
                event_type: "a".into(),
                category: "t".into(),
                turn_id: 1,
                severity: "info".into(),
                timestamp: String::new(),
                data: serde_json::json!({}),
                offset_ms: 0,
            },
            ReplayEvent {
                seq: 5,
                event_type: "b".into(),
                category: "t".into(),
                turn_id: 1,
                severity: "info".into(),
                timestamp: String::new(),
                data: serde_json::json!({}),
                offset_ms: 100,
            },
        ];
        let v = validate_replay(&events);
        assert!(!v.valid);
        assert!(!v.issues.is_empty());
    }
}
