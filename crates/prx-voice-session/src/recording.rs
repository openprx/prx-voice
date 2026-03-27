//! Session recording policy and metadata management.
//!
//! Per the data model spec, recording metadata lives in the relational store
//! while audio payloads go to object storage. Phase 4 implements metadata only.

use chrono::{DateTime, Utc};
use parking_lot::RwLock;
use prx_voice_types::ids::{SessionId, TenantId};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use uuid::Uuid;

/// Recording ID.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct RecordingId(Uuid);

impl RecordingId {
    pub fn new() -> Self {
        Self(Uuid::new_v4())
    }
}

impl Default for RecordingId {
    fn default() -> Self {
        Self::new()
    }
}

/// Recording policy — determines what gets recorded.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RecordingPolicy {
    /// Whether recording is enabled at all.
    pub enabled: bool,
    /// Record user audio input.
    pub record_user_input: bool,
    /// Record system audio output.
    pub record_system_output: bool,
    /// Record mixed (both sides).
    pub record_mixed: bool,
    /// Retention class for recordings.
    pub retention_class: RetentionClass,
    /// Max recording duration (seconds). 0 = unlimited.
    pub max_duration_sec: u64,
}

impl Default for RecordingPolicy {
    fn default() -> Self {
        Self {
            enabled: false,
            record_user_input: true,
            record_system_output: true,
            record_mixed: false,
            retention_class: RetentionClass::StandardOperational,
            max_duration_sec: 7200,
        }
    }
}

/// Retention class per data model spec.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RetentionClass {
    Transient,
    StandardOperational,
    BillingCritical,
    AuditCritical,
    ComplianceLocked,
    LegalHold,
}

/// Stream role in a recording.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum StreamRole {
    UserInput,
    SystemOutput,
    Mixed,
}

/// Recording status.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RecordingStatus {
    Active,
    Completed,
    Failed,
    Deleted,
}

/// Recording metadata (audio stored separately in object storage).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RecordingMetadata {
    pub recording_id: RecordingId,
    pub session_id: SessionId,
    pub tenant_id: TenantId,
    pub stream_role: StreamRole,
    pub codec: String,
    pub sample_rate: u32,
    pub channel_count: u16,
    pub duration_ms: u64,
    pub status: RecordingStatus,
    pub retention_class: RetentionClass,
    pub storage_uri: Option<String>,
    pub created_at: DateTime<Utc>,
    pub completed_at: Option<DateTime<Utc>>,
}

/// In-memory recording metadata store.
pub struct RecordingStore {
    recordings: RwLock<HashMap<RecordingId, RecordingMetadata>>,
    session_index: RwLock<HashMap<SessionId, Vec<RecordingId>>>,
}

impl RecordingStore {
    pub fn new() -> Self {
        Self {
            recordings: RwLock::new(HashMap::new()),
            session_index: RwLock::new(HashMap::new()),
        }
    }

    /// Start a new recording for a session.
    pub fn start_recording(
        &self,
        session_id: SessionId,
        tenant_id: TenantId,
        stream_role: StreamRole,
        codec: impl Into<String>,
        sample_rate: u32,
        retention_class: RetentionClass,
    ) -> RecordingMetadata {
        let meta = RecordingMetadata {
            recording_id: RecordingId::new(),
            session_id,
            tenant_id,
            stream_role,
            codec: codec.into(),
            sample_rate,
            channel_count: 1,
            duration_ms: 0,
            status: RecordingStatus::Active,
            retention_class,
            storage_uri: None,
            created_at: Utc::now(),
            completed_at: None,
        };

        self.recordings
            .write()
            .insert(meta.recording_id, meta.clone());
        self.session_index
            .write()
            .entry(session_id)
            .or_default()
            .push(meta.recording_id);

        meta
    }

    /// Complete a recording.
    pub fn complete_recording(
        &self,
        recording_id: RecordingId,
        duration_ms: u64,
        storage_uri: Option<String>,
    ) -> Option<RecordingMetadata> {
        let mut recordings = self.recordings.write();
        let meta = recordings.get_mut(&recording_id)?;
        meta.status = RecordingStatus::Completed;
        meta.duration_ms = duration_ms;
        meta.storage_uri = storage_uri;
        meta.completed_at = Some(Utc::now());
        Some(meta.clone())
    }

    /// Get recordings for a session.
    pub fn get_by_session(&self, session_id: SessionId) -> Vec<RecordingMetadata> {
        let ids = self
            .session_index
            .read()
            .get(&session_id)
            .cloned()
            .unwrap_or_default();
        let recordings = self.recordings.read();
        ids.iter()
            .filter_map(|id| recordings.get(id).cloned())
            .collect()
    }

    /// Get a single recording.
    pub fn get(&self, recording_id: RecordingId) -> Option<RecordingMetadata> {
        self.recordings.read().get(&recording_id).cloned()
    }
}

impl Default for RecordingStore {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn start_and_complete_recording() {
        let store = RecordingStore::new();
        let sid = SessionId::new();
        let tid = TenantId::new();

        let meta = store.start_recording(
            sid,
            tid,
            StreamRole::UserInput,
            "pcm16",
            16000,
            RetentionClass::StandardOperational,
        );
        assert_eq!(meta.status, RecordingStatus::Active);
        assert_eq!(meta.duration_ms, 0);

        let completed = store
            .complete_recording(
                meta.recording_id,
                45000,
                Some("s3://bucket/recording.wav".into()),
            )
            .unwrap();
        assert_eq!(completed.status, RecordingStatus::Completed);
        assert_eq!(completed.duration_ms, 45000);
        assert!(completed.storage_uri.is_some());
        assert!(completed.completed_at.is_some());
    }

    #[test]
    fn get_recordings_by_session() {
        let store = RecordingStore::new();
        let sid = SessionId::new();
        let tid = TenantId::new();

        store.start_recording(
            sid,
            tid,
            StreamRole::UserInput,
            "pcm16",
            16000,
            RetentionClass::StandardOperational,
        );
        store.start_recording(
            sid,
            tid,
            StreamRole::SystemOutput,
            "pcm16",
            16000,
            RetentionClass::StandardOperational,
        );

        let recordings = store.get_by_session(sid);
        assert_eq!(recordings.len(), 2);
    }

    #[test]
    fn default_policy_disabled() {
        let policy = RecordingPolicy::default();
        assert!(!policy.enabled);
        assert!(policy.record_user_input);
    }

    #[test]
    fn retention_class_serializes() {
        let json = serde_json::to_string(&RetentionClass::LegalHold).unwrap();
        assert_eq!(json, "\"legal_hold\"");
    }
}
