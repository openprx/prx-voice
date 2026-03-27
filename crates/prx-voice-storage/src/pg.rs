//! PostgreSQL implementations of storage traits.
//! Activated with `cargo build --features postgres`.

use crate::models::*;
use crate::traits::*;
use chrono::{DateTime, Utc};
use sqlx::PgPool;
use uuid::Uuid;

/// PostgreSQL session repository.
pub struct PgSessionRepo {
    pool: PgPool,
}

impl PgSessionRepo {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }
}

#[async_trait::async_trait]
impl SessionRepository for PgSessionRepo {
    async fn create(&self, record: SessionRecord) -> Result<(), StorageError> {
        sqlx::query(
            r#"INSERT INTO sessions (session_id, tenant_id, state, channel, direction, language, total_turns, created_at, updated_at, metadata)
               VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10)"#
        )
        .bind(record.session_id)
        .bind(record.tenant_id)
        .bind(&record.state)
        .bind(&record.channel)
        .bind(&record.direction)
        .bind(&record.language)
        .bind(record.total_turns)
        .bind(record.created_at)
        .bind(record.updated_at)
        .bind(&record.metadata)
        .execute(&self.pool)
        .await
        .map_err(|e| StorageError::Database(e.to_string()))?;
        Ok(())
    }

    async fn get(&self, session_id: Uuid) -> Result<Option<SessionRecord>, StorageError> {
        let row = sqlx::query_as::<_, PgSessionRow>(
            "SELECT session_id, tenant_id, state, channel, direction, language, total_turns, created_at, updated_at, closed_at, close_reason, metadata FROM sessions WHERE session_id = $1"
        )
        .bind(session_id)
        .fetch_optional(&self.pool)
        .await
        .map_err(|e| StorageError::Database(e.to_string()))?;

        Ok(row.map(Into::into))
    }

    async fn update_state(
        &self,
        session_id: Uuid,
        state: &str,
        updated_at: DateTime<Utc>,
    ) -> Result<(), StorageError> {
        let result =
            sqlx::query("UPDATE sessions SET state = $1, updated_at = $2 WHERE session_id = $3")
                .bind(state)
                .bind(updated_at)
                .bind(session_id)
                .execute(&self.pool)
                .await
                .map_err(|e| StorageError::Database(e.to_string()))?;

        if result.rows_affected() == 0 {
            return Err(StorageError::NotFound(session_id.to_string()));
        }
        Ok(())
    }

    async fn close(
        &self,
        session_id: Uuid,
        reason: &str,
        closed_at: DateTime<Utc>,
    ) -> Result<(), StorageError> {
        let result = sqlx::query(
            "UPDATE sessions SET state = 'Closed', close_reason = $1, closed_at = $2, updated_at = $2 WHERE session_id = $3"
        )
        .bind(reason)
        .bind(closed_at)
        .bind(session_id)
        .execute(&self.pool)
        .await
        .map_err(|e| StorageError::Database(e.to_string()))?;

        if result.rows_affected() == 0 {
            return Err(StorageError::NotFound(session_id.to_string()));
        }
        Ok(())
    }

    async fn list_by_tenant(
        &self,
        tenant_id: Uuid,
        cursor: PageCursor,
    ) -> Result<PageResult<SessionRecord>, StorageError> {
        let total: (i64,) = sqlx::query_as("SELECT COUNT(*) FROM sessions WHERE tenant_id = $1")
            .bind(tenant_id)
            .fetch_one(&self.pool)
            .await
            .map_err(|e| StorageError::Database(e.to_string()))?;

        let rows: Vec<PgSessionRow> = sqlx::query_as(
            "SELECT session_id, tenant_id, state, channel, direction, language, total_turns, created_at, updated_at, closed_at, close_reason, metadata FROM sessions WHERE tenant_id = $1 ORDER BY created_at DESC LIMIT $2 OFFSET $3"
        )
        .bind(tenant_id)
        .bind(cursor.limit)
        .bind(cursor.offset)
        .fetch_all(&self.pool)
        .await
        .map_err(|e| StorageError::Database(e.to_string()))?;

        let items: Vec<SessionRecord> = rows.into_iter().map(Into::into).collect();
        let has_more = cursor.offset + cursor.limit < total.0;

        Ok(PageResult {
            items,
            total: total.0,
            has_more,
        })
    }
}

/// Internal row type for sqlx mapping.
#[derive(sqlx::FromRow)]
struct PgSessionRow {
    session_id: Uuid,
    tenant_id: Uuid,
    state: String,
    channel: String,
    direction: String,
    language: String,
    total_turns: i32,
    created_at: DateTime<Utc>,
    updated_at: DateTime<Utc>,
    closed_at: Option<DateTime<Utc>>,
    close_reason: Option<String>,
    metadata: Option<serde_json::Value>,
}

impl From<PgSessionRow> for SessionRecord {
    fn from(row: PgSessionRow) -> Self {
        Self {
            session_id: row.session_id,
            tenant_id: row.tenant_id,
            state: row.state,
            channel: row.channel,
            direction: row.direction,
            language: row.language,
            total_turns: row.total_turns,
            created_at: row.created_at,
            updated_at: row.updated_at,
            closed_at: row.closed_at,
            close_reason: row.close_reason,
            metadata: row.metadata,
        }
    }
}

/// PostgreSQL event repository.
pub struct PgEventRepo {
    pool: PgPool,
}

impl PgEventRepo {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }
}

#[async_trait::async_trait]
impl EventRepository for PgEventRepo {
    async fn append(&self, record: EventRecord) -> Result<(), StorageError> {
        sqlx::query(
            r#"INSERT INTO session_events (event_id, session_id, tenant_id, turn_id, seq, event_type, severity, payload, created_at)
               VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9)"#
        )
        .bind(record.event_id)
        .bind(record.session_id)
        .bind(record.tenant_id)
        .bind(record.turn_id)
        .bind(record.seq)
        .bind(&record.event_type)
        .bind(&record.severity)
        .bind(&record.payload)
        .bind(record.created_at)
        .execute(&self.pool)
        .await
        .map_err(|e| StorageError::Database(e.to_string()))?;
        Ok(())
    }

    async fn list_by_session(
        &self,
        session_id: Uuid,
        cursor: PageCursor,
    ) -> Result<PageResult<EventRecord>, StorageError> {
        let total: (i64,) =
            sqlx::query_as("SELECT COUNT(*) FROM session_events WHERE session_id = $1")
                .bind(session_id)
                .fetch_one(&self.pool)
                .await
                .map_err(|e| StorageError::Database(e.to_string()))?;

        let rows: Vec<PgEventRow> = sqlx::query_as(
            "SELECT event_id, session_id, tenant_id, turn_id, seq, event_type, severity, payload, created_at FROM session_events WHERE session_id = $1 ORDER BY seq ASC LIMIT $2 OFFSET $3"
        )
        .bind(session_id)
        .bind(cursor.limit)
        .bind(cursor.offset)
        .fetch_all(&self.pool)
        .await
        .map_err(|e| StorageError::Database(e.to_string()))?;

        let items: Vec<EventRecord> = rows.into_iter().map(Into::into).collect();
        let has_more = cursor.offset + cursor.limit < total.0;

        Ok(PageResult {
            items,
            total: total.0,
            has_more,
        })
    }

    async fn get_latest_seq(&self, session_id: Uuid) -> Result<i64, StorageError> {
        let row: (i64,) = sqlx::query_as(
            "SELECT COALESCE(MAX(seq), 0) FROM session_events WHERE session_id = $1",
        )
        .bind(session_id)
        .fetch_one(&self.pool)
        .await
        .map_err(|e| StorageError::Database(e.to_string()))?;
        Ok(row.0)
    }
}

#[derive(sqlx::FromRow)]
struct PgEventRow {
    event_id: Uuid,
    session_id: Uuid,
    tenant_id: Uuid,
    turn_id: Option<i32>,
    seq: i64,
    event_type: String,
    severity: String,
    payload: serde_json::Value,
    created_at: DateTime<Utc>,
}

impl From<PgEventRow> for EventRecord {
    fn from(row: PgEventRow) -> Self {
        Self {
            event_id: row.event_id,
            session_id: row.session_id,
            tenant_id: row.tenant_id,
            turn_id: row.turn_id,
            seq: row.seq,
            event_type: row.event_type,
            severity: row.severity,
            payload: row.payload,
            created_at: row.created_at,
        }
    }
}

/// PostgreSQL audit repository.
pub struct PgAuditRepo {
    pool: PgPool,
}

impl PgAuditRepo {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }
}

#[async_trait::async_trait]
impl AuditRepository for PgAuditRepo {
    async fn append(&self, record: AuditRecord) -> Result<(), StorageError> {
        sqlx::query(
            r#"INSERT INTO audit_records (audit_id, tenant_id, principal_id, principal_type, action, target_type, target_id, result, reason, correlation_id, details, created_at)
               VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12)"#
        )
        .bind(record.audit_id)
        .bind(record.tenant_id)
        .bind(&record.principal_id)
        .bind(&record.principal_type)
        .bind(&record.action)
        .bind(&record.target_type)
        .bind(&record.target_id)
        .bind(&record.result)
        .bind(&record.reason)
        .bind(&record.correlation_id)
        .bind(&record.details)
        .bind(record.created_at)
        .execute(&self.pool)
        .await
        .map_err(|e| StorageError::Database(e.to_string()))?;
        Ok(())
    }

    async fn query(
        &self,
        tenant_id: Option<Uuid>,
        limit: i64,
    ) -> Result<Vec<AuditRecord>, StorageError> {
        let rows: Vec<PgAuditRow> = if let Some(tid) = tenant_id {
            sqlx::query_as(
                "SELECT audit_id, tenant_id, principal_id, principal_type, action, target_type, target_id, result, reason, correlation_id, details, created_at FROM audit_records WHERE tenant_id = $1 ORDER BY created_at DESC LIMIT $2"
            )
            .bind(tid)
            .bind(limit)
            .fetch_all(&self.pool)
            .await
        } else {
            sqlx::query_as(
                "SELECT audit_id, tenant_id, principal_id, principal_type, action, target_type, target_id, result, reason, correlation_id, details, created_at FROM audit_records ORDER BY created_at DESC LIMIT $1"
            )
            .bind(limit)
            .fetch_all(&self.pool)
            .await
        }
        .map_err(|e| StorageError::Database(e.to_string()))?;

        Ok(rows.into_iter().map(Into::into).collect())
    }
}

#[derive(sqlx::FromRow)]
struct PgAuditRow {
    audit_id: Uuid,
    tenant_id: Option<Uuid>,
    principal_id: String,
    principal_type: String,
    action: String,
    target_type: String,
    target_id: String,
    result: String,
    reason: Option<String>,
    correlation_id: Option<String>,
    details: Option<serde_json::Value>,
    created_at: DateTime<Utc>,
}

impl From<PgAuditRow> for AuditRecord {
    fn from(row: PgAuditRow) -> Self {
        Self {
            audit_id: row.audit_id,
            tenant_id: row.tenant_id,
            principal_id: row.principal_id,
            principal_type: row.principal_type,
            action: row.action,
            target_type: row.target_type,
            target_id: row.target_id,
            result: row.result,
            reason: row.reason,
            correlation_id: row.correlation_id,
            details: row.details,
            created_at: row.created_at,
        }
    }
}

/// PostgreSQL billing repository.
pub struct PgBillingRepo {
    pool: PgPool,
}

impl PgBillingRepo {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }
}

#[async_trait::async_trait]
impl BillingRepository for PgBillingRepo {
    async fn record(&self, entry: BillingEntry) -> Result<(), StorageError> {
        sqlx::query(
            r#"INSERT INTO billing_entries (entry_id, idempotency_key, tenant_id, session_id, meter_type, quantity, unit, provider, entry_type, created_at)
               VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10)
               ON CONFLICT (idempotency_key) DO NOTHING"#
        )
        .bind(entry.entry_id)
        .bind(&entry.idempotency_key)
        .bind(entry.tenant_id)
        .bind(entry.session_id)
        .bind(&entry.meter_type)
        .bind(entry.quantity)
        .bind(&entry.unit)
        .bind(&entry.provider)
        .bind(&entry.entry_type)
        .bind(entry.created_at)
        .execute(&self.pool)
        .await
        .map_err(|e| StorageError::Database(e.to_string()))?;
        Ok(())
    }

    async fn list_by_tenant(&self, tenant_id: Uuid) -> Result<Vec<BillingEntry>, StorageError> {
        let rows: Vec<PgBillingRow> = sqlx::query_as(
            "SELECT entry_id, idempotency_key, tenant_id, session_id, meter_type, quantity, unit, provider, entry_type, created_at FROM billing_entries WHERE tenant_id = $1 ORDER BY created_at DESC"
        )
        .bind(tenant_id)
        .fetch_all(&self.pool)
        .await
        .map_err(|e| StorageError::Database(e.to_string()))?;

        Ok(rows.into_iter().map(Into::into).collect())
    }

    async fn check_idempotency(&self, key: &str) -> Result<bool, StorageError> {
        let row: (bool,) = sqlx::query_as(
            "SELECT EXISTS(SELECT 1 FROM billing_entries WHERE idempotency_key = $1)",
        )
        .bind(key)
        .fetch_one(&self.pool)
        .await
        .map_err(|e| StorageError::Database(e.to_string()))?;
        Ok(row.0)
    }
}

#[derive(sqlx::FromRow)]
struct PgBillingRow {
    entry_id: Uuid,
    idempotency_key: String,
    tenant_id: Uuid,
    session_id: Option<Uuid>,
    meter_type: String,
    quantity: f64,
    unit: String,
    provider: Option<String>,
    entry_type: String,
    created_at: DateTime<Utc>,
}

impl From<PgBillingRow> for BillingEntry {
    fn from(row: PgBillingRow) -> Self {
        Self {
            entry_id: row.entry_id,
            idempotency_key: row.idempotency_key,
            tenant_id: row.tenant_id,
            session_id: row.session_id,
            meter_type: row.meter_type,
            quantity: row.quantity,
            unit: row.unit,
            provider: row.provider,
            entry_type: row.entry_type,
            created_at: row.created_at,
        }
    }
}

/// Run all migrations against the database.
pub async fn run_migrations(pool: &PgPool) -> Result<(), StorageError> {
    for (name, sql) in crate::migrations::MIGRATIONS {
        tracing::info!(migration = name, "Running migration");
        sqlx::raw_sql(sql)
            .execute(pool)
            .await
            .map_err(|e| StorageError::Database(format!("Migration {name} failed: {e}")))?;
    }
    Ok(())
}
