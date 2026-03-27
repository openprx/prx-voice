//! Storage repository traits.
//! Implementations can be in-memory (dev/test) or PostgreSQL (production).

use crate::models::*;
use chrono::{DateTime, Utc};
use uuid::Uuid;

/// Session repository errors.
#[derive(Debug, thiserror::Error)]
pub enum StorageError {
    #[error("Not found: {0}")]
    NotFound(String),
    #[error("Duplicate: {0}")]
    Duplicate(String),
    #[error("Database error: {0}")]
    Database(String),
}

/// Session repository trait.
#[async_trait::async_trait]
pub trait SessionRepository: Send + Sync {
    async fn create(&self, record: SessionRecord) -> Result<(), StorageError>;
    async fn get(&self, session_id: Uuid) -> Result<Option<SessionRecord>, StorageError>;
    async fn update_state(
        &self,
        session_id: Uuid,
        state: &str,
        updated_at: DateTime<Utc>,
    ) -> Result<(), StorageError>;
    async fn close(
        &self,
        session_id: Uuid,
        reason: &str,
        closed_at: DateTime<Utc>,
    ) -> Result<(), StorageError>;
    async fn list_by_tenant(
        &self,
        tenant_id: Uuid,
        cursor: PageCursor,
    ) -> Result<PageResult<SessionRecord>, StorageError>;
}

/// Turn repository trait.
#[async_trait::async_trait]
pub trait TurnRepository: Send + Sync {
    async fn create(&self, record: TurnRecord) -> Result<(), StorageError>;
    async fn get(&self, turn_id: Uuid) -> Result<Option<TurnRecord>, StorageError>;
    async fn list_by_session(
        &self,
        session_id: Uuid,
        cursor: PageCursor,
    ) -> Result<PageResult<TurnRecord>, StorageError>;
    async fn complete(
        &self,
        turn_id: Uuid,
        agent_response: &str,
        completed_at: DateTime<Utc>,
    ) -> Result<(), StorageError>;
}

/// Event repository trait.
#[async_trait::async_trait]
pub trait EventRepository: Send + Sync {
    async fn append(&self, record: EventRecord) -> Result<(), StorageError>;
    async fn list_by_session(
        &self,
        session_id: Uuid,
        cursor: PageCursor,
    ) -> Result<PageResult<EventRecord>, StorageError>;
    async fn get_latest_seq(&self, session_id: Uuid) -> Result<i64, StorageError>;
}

/// Audit repository trait.
#[async_trait::async_trait]
pub trait AuditRepository: Send + Sync {
    async fn append(&self, record: AuditRecord) -> Result<(), StorageError>;
    async fn query(
        &self,
        tenant_id: Option<Uuid>,
        limit: i64,
    ) -> Result<Vec<AuditRecord>, StorageError>;
}

/// Billing repository trait.
#[async_trait::async_trait]
pub trait BillingRepository: Send + Sync {
    async fn record(&self, entry: BillingEntry) -> Result<(), StorageError>;
    async fn list_by_tenant(&self, tenant_id: Uuid) -> Result<Vec<BillingEntry>, StorageError>;
    async fn check_idempotency(&self, key: &str) -> Result<bool, StorageError>;
}
