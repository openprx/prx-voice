//! Append-only billing ledger with idempotency.
//! Per spec: entries are immutable; corrections are new entries referencing the original.

use crate::meter::{MeterType, MeterUnit};
use chrono::{DateTime, Utc};
use parking_lot::RwLock;
use prx_voice_types::ids::{SessionId, TenantId};
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use uuid::Uuid;

/// Unique ledger entry ID.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct LedgerEntryId(Uuid);

impl LedgerEntryId {
    pub fn new() -> Self {
        Self(Uuid::new_v4())
    }
}

impl Default for LedgerEntryId {
    fn default() -> Self {
        Self::new()
    }
}

/// Type of ledger entry.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EntryType {
    /// Normal usage record.
    Usage,
    /// Correction of a previous entry.
    Correction,
}

/// A single billing ledger entry (immutable once written).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LedgerEntry {
    pub entry_id: LedgerEntryId,
    pub idempotency_key: String,
    pub tenant_id: TenantId,
    pub session_id: Option<SessionId>,
    pub meter_type: MeterType,
    pub quantity: f64,
    pub unit: MeterUnit,
    pub provider: Option<String>,
    pub model: Option<String>,
    pub entry_type: EntryType,
    /// If correction, references the original entry.
    pub correction_of: Option<LedgerEntryId>,
    pub recorded_at: DateTime<Utc>,
}

/// Error from ledger operations.
#[derive(Debug, thiserror::Error)]
pub enum LedgerError {
    #[error("Duplicate idempotency key: {0}")]
    DuplicateIdempotencyKey(String),
}

/// In-memory billing ledger.
pub struct BillingLedger {
    entries: RwLock<Vec<LedgerEntry>>,
    idempotency_keys: RwLock<HashSet<String>>,
}

impl BillingLedger {
    pub fn new() -> Self {
        Self {
            entries: RwLock::new(Vec::new()),
            idempotency_keys: RwLock::new(HashSet::new()),
        }
    }

    /// Record a usage entry. Idempotency key prevents duplicates.
    pub fn record(&self, entry: LedgerEntry) -> Result<LedgerEntryId, LedgerError> {
        let mut keys = self.idempotency_keys.write();
        if keys.contains(&entry.idempotency_key) {
            return Err(LedgerError::DuplicateIdempotencyKey(
                entry.idempotency_key.clone(),
            ));
        }
        keys.insert(entry.idempotency_key.clone());
        let id = entry.entry_id;
        self.entries.write().push(entry);
        Ok(id)
    }

    /// Get all entries for a tenant in a billing period.
    pub fn entries_for_tenant(&self, tenant_id: TenantId) -> Vec<LedgerEntry> {
        self.entries
            .read()
            .iter()
            .filter(|e| e.tenant_id == tenant_id)
            .cloned()
            .collect()
    }

    /// Summarize usage by meter type for a tenant.
    pub fn summarize(&self, tenant_id: TenantId) -> HashMap<MeterType, f64> {
        let entries = self.entries.read();
        let mut summary: HashMap<MeterType, f64> = HashMap::new();
        for e in entries.iter().filter(|e| e.tenant_id == tenant_id) {
            *summary.entry(e.meter_type.clone()).or_default() += e.quantity;
        }
        summary
    }

    /// Total entry count.
    pub fn count(&self) -> usize {
        self.entries.read().len()
    }
}

impl Default for BillingLedger {
    fn default() -> Self {
        Self::new()
    }
}

/// Helper to create a usage entry.
pub fn usage_entry(
    tenant_id: TenantId,
    session_id: Option<SessionId>,
    meter_type: MeterType,
    quantity: f64,
    provider: Option<String>,
) -> LedgerEntry {
    let unit = meter_type.unit();
    LedgerEntry {
        entry_id: LedgerEntryId::new(),
        idempotency_key: Uuid::new_v4().to_string(),
        tenant_id,
        session_id,
        meter_type,
        quantity,
        unit,
        provider,
        model: None,
        entry_type: EntryType::Usage,
        correction_of: None,
        recorded_at: Utc::now(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn record_and_retrieve() {
        let ledger = BillingLedger::new();
        let tid = TenantId::new();
        let entry = usage_entry(
            tid,
            None,
            MeterType::AsrAudioSeconds,
            30.5,
            Some("deepgram".into()),
        );
        ledger.record(entry).unwrap();
        assert_eq!(ledger.count(), 1);
        assert_eq!(ledger.entries_for_tenant(tid).len(), 1);
    }

    #[test]
    fn idempotency_prevents_duplicates() {
        let ledger = BillingLedger::new();
        let tid = TenantId::new();
        let mut e1 = usage_entry(tid, None, MeterType::AgentInputTokens, 100.0, None);
        e1.idempotency_key = "key-123".into();
        ledger.record(e1).unwrap();

        let mut e2 = usage_entry(tid, None, MeterType::AgentInputTokens, 100.0, None);
        e2.idempotency_key = "key-123".into();
        assert!(matches!(
            ledger.record(e2),
            Err(LedgerError::DuplicateIdempotencyKey(_))
        ));
        assert_eq!(ledger.count(), 1);
    }

    #[test]
    fn summarize_by_meter_type() {
        let ledger = BillingLedger::new();
        let tid = TenantId::new();
        ledger
            .record(usage_entry(
                tid,
                None,
                MeterType::AsrAudioSeconds,
                10.0,
                None,
            ))
            .unwrap();
        ledger
            .record(usage_entry(
                tid,
                None,
                MeterType::AsrAudioSeconds,
                20.0,
                None,
            ))
            .unwrap();
        ledger
            .record(usage_entry(
                tid,
                None,
                MeterType::TtsCharacters,
                500.0,
                None,
            ))
            .unwrap();

        let summary = ledger.summarize(tid);
        assert_eq!(summary[&MeterType::AsrAudioSeconds], 30.0);
        assert_eq!(summary[&MeterType::TtsCharacters], 500.0);
    }

    #[test]
    fn different_tenants_isolated() {
        let ledger = BillingLedger::new();
        let t1 = TenantId::new();
        let t2 = TenantId::new();
        ledger
            .record(usage_entry(
                t1,
                None,
                MeterType::AgentOutputTokens,
                50.0,
                None,
            ))
            .unwrap();
        ledger
            .record(usage_entry(
                t2,
                None,
                MeterType::AgentOutputTokens,
                80.0,
                None,
            ))
            .unwrap();

        assert_eq!(ledger.entries_for_tenant(t1).len(), 1);
        assert_eq!(ledger.entries_for_tenant(t2).len(), 1);
        assert_eq!(ledger.summarize(t1)[&MeterType::AgentOutputTokens], 50.0);
    }
}
