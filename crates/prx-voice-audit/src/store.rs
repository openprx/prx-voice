//! Append-only audit store.
//! In production this would be backed by a durable database.
//! Phase 3 uses in-memory for now.

use crate::record::{AuditAction, AuditRecord};
use chrono::{DateTime, Utc};
use parking_lot::RwLock;
use prx_voice_types::ids::TenantId;

/// Query filters for audit records.
#[derive(Debug, Default)]
pub struct AuditQuery {
    pub tenant_id: Option<TenantId>,
    pub principal_id: Option<String>,
    pub action: Option<AuditAction>,
    pub target_type: Option<String>,
    pub target_id: Option<String>,
    pub from: Option<DateTime<Utc>>,
    pub to: Option<DateTime<Utc>>,
    pub limit: Option<usize>,
}

/// Append-only audit log store.
pub struct AuditStore {
    records: RwLock<Vec<AuditRecord>>,
}

impl AuditStore {
    pub fn new() -> Self {
        Self {
            records: RwLock::new(Vec::new()),
        }
    }

    /// Append a record. Records are immutable once written.
    pub fn append(&self, record: AuditRecord) {
        self.records.write().push(record);
    }

    /// Query records with filters.
    pub fn query(&self, q: &AuditQuery) -> Vec<AuditRecord> {
        let records = self.records.read();
        let mut results: Vec<&AuditRecord> = records.iter().collect();

        if let Some(tid) = q.tenant_id {
            results.retain(|r| r.tenant_id == Some(tid));
        }
        if let Some(ref pid) = q.principal_id {
            results.retain(|r| &r.principal_id == pid);
        }
        if let Some(ref tt) = q.target_type {
            results.retain(|r| &r.target_type == tt);
        }
        if let Some(ref ti) = q.target_id {
            results.retain(|r| &r.target_id == ti);
        }
        if let Some(from) = q.from {
            results.retain(|r| r.timestamp >= from);
        }
        if let Some(to) = q.to {
            results.retain(|r| r.timestamp <= to);
        }

        // Most recent first
        results.reverse();

        if let Some(limit) = q.limit {
            results.truncate(limit);
        }

        results.into_iter().cloned().collect()
    }

    /// Total record count.
    pub fn count(&self) -> usize {
        self.records.read().len()
    }
}

impl Default for AuditStore {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::record::{AuditResult, PrincipalType};

    fn make_record(action: AuditAction, tenant: Option<TenantId>) -> AuditRecord {
        let mut r = AuditRecord::new(
            "user-1",
            PrincipalType::User,
            action,
            "session",
            "sess-1",
            AuditResult::Success,
        );
        if let Some(tid) = tenant {
            r = r.with_tenant(tid);
        }
        r
    }

    #[test]
    fn append_and_count() {
        let store = AuditStore::new();
        store.append(make_record(AuditAction::SessionCreated, None));
        store.append(make_record(AuditAction::SessionClosed, None));
        assert_eq!(store.count(), 2);
    }

    #[test]
    fn query_by_tenant() {
        let store = AuditStore::new();
        let t1 = TenantId::new();
        let t2 = TenantId::new();
        store.append(make_record(AuditAction::SessionCreated, Some(t1)));
        store.append(make_record(AuditAction::SessionCreated, Some(t2)));
        store.append(make_record(AuditAction::SessionClosed, Some(t1)));

        let results = store.query(&AuditQuery {
            tenant_id: Some(t1),
            ..Default::default()
        });
        assert_eq!(results.len(), 2);
    }

    #[test]
    fn query_with_limit() {
        let store = AuditStore::new();
        for _ in 0..10 {
            store.append(make_record(AuditAction::SessionCreated, None));
        }
        let results = store.query(&AuditQuery {
            limit: Some(3),
            ..Default::default()
        });
        assert_eq!(results.len(), 3);
    }

    #[test]
    fn query_returns_most_recent_first() {
        let store = AuditStore::new();
        store.append(make_record(AuditAction::SessionCreated, None));
        store.append(make_record(AuditAction::SessionClosed, None));
        let results = store.query(&AuditQuery::default());
        // Most recent (SessionClosed) should be first
        assert!(matches!(results[0].action, AuditAction::SessionClosed));
    }
}
