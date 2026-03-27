//! Storage models — database-friendly versions of domain types.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// Persisted session record.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionRecord {
    pub session_id: Uuid,
    pub tenant_id: Uuid,
    pub state: String,
    pub channel: String,
    pub direction: String,
    pub language: String,
    pub total_turns: i32,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub closed_at: Option<DateTime<Utc>>,
    pub close_reason: Option<String>,
    pub metadata: Option<serde_json::Value>,
}

/// Persisted turn record.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TurnRecord {
    pub turn_id: Uuid,
    pub session_id: Uuid,
    pub tenant_id: Uuid,
    pub sequence_no: i32,
    pub user_transcript: Option<String>,
    pub agent_response: Option<String>,
    pub asr_latency_ms: Option<i64>,
    pub agent_latency_ms: Option<i64>,
    pub tts_latency_ms: Option<i64>,
    pub interrupted: bool,
    pub started_at: DateTime<Utc>,
    pub completed_at: Option<DateTime<Utc>>,
}

/// Persisted event record.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EventRecord {
    pub event_id: Uuid,
    pub session_id: Uuid,
    pub tenant_id: Uuid,
    pub turn_id: Option<i32>,
    pub seq: i64,
    pub event_type: String,
    pub severity: String,
    pub payload: serde_json::Value,
    pub created_at: DateTime<Utc>,
}

/// Persisted audit record.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuditRecord {
    pub audit_id: Uuid,
    pub tenant_id: Option<Uuid>,
    pub principal_id: String,
    pub principal_type: String,
    pub action: String,
    pub target_type: String,
    pub target_id: String,
    pub result: String,
    pub reason: Option<String>,
    pub correlation_id: Option<String>,
    pub details: Option<serde_json::Value>,
    pub created_at: DateTime<Utc>,
}

/// Persisted billing ledger entry.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BillingEntry {
    pub entry_id: Uuid,
    pub idempotency_key: String,
    pub tenant_id: Uuid,
    pub session_id: Option<Uuid>,
    pub meter_type: String,
    pub quantity: f64,
    pub unit: String,
    pub provider: Option<String>,
    pub entry_type: String,
    pub created_at: DateTime<Utc>,
}

/// Pagination cursor.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PageCursor {
    pub limit: i64,
    pub offset: i64,
}

impl Default for PageCursor {
    fn default() -> Self {
        Self {
            limit: 20,
            offset: 0,
        }
    }
}

/// Paginated result.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PageResult<T> {
    pub items: Vec<T>,
    pub total: i64,
    pub has_more: bool,
}
