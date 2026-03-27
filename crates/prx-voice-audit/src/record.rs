//! Audit record types per the security spec.
//! Records are append-only and immutable.

use chrono::{DateTime, Utc};
use prx_voice_types::ids::TenantId;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// Unique audit record ID.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct AuditId(Uuid);

impl AuditId {
    pub fn new() -> Self {
        Self(Uuid::new_v4())
    }
}

impl Default for AuditId {
    fn default() -> Self {
        Self::new()
    }
}

/// Who performed the action.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PrincipalType {
    User,
    ServiceAccount,
    System,
    Operator,
}

/// What happened.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AuditAction {
    SessionCreated,
    SessionClosed,
    SessionFailed,
    SessionPaused,
    SessionResumed,
    TranscriptCorrected,
    HandoffRequested,
    PolicyModified,
    QuotaModified,
    DataExported,
    RetentionPolicyChanged,
    TenantCreated,
    TenantModified,
    RoleAssigned,
    RoleRevoked,
    ConfigChanged,
    BillingModified,
}

/// Outcome of the action.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum AuditResult {
    Success,
    Failure,
    Denied,
}

/// The immutable audit record.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuditRecord {
    pub audit_id: AuditId,
    pub timestamp: DateTime<Utc>,
    pub tenant_id: Option<TenantId>,
    pub principal_id: String,
    pub principal_type: PrincipalType,
    pub action: AuditAction,
    pub target_type: String,
    pub target_id: String,
    pub result: AuditResult,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reason: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub correlation_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub details: Option<serde_json::Value>,
}

impl AuditRecord {
    /// Build a new audit record.
    pub fn new(
        principal_id: impl Into<String>,
        principal_type: PrincipalType,
        action: AuditAction,
        target_type: impl Into<String>,
        target_id: impl Into<String>,
        result: AuditResult,
    ) -> Self {
        Self {
            audit_id: AuditId::new(),
            timestamp: Utc::now(),
            tenant_id: None,
            principal_id: principal_id.into(),
            principal_type,
            action,
            target_type: target_type.into(),
            target_id: target_id.into(),
            result,
            reason: None,
            correlation_id: None,
            details: None,
        }
    }

    pub fn with_tenant(mut self, tenant_id: TenantId) -> Self {
        self.tenant_id = Some(tenant_id);
        self
    }

    pub fn with_reason(mut self, reason: impl Into<String>) -> Self {
        self.reason = Some(reason.into());
        self
    }

    pub fn with_correlation(mut self, id: impl Into<String>) -> Self {
        self.correlation_id = Some(id.into());
        self
    }

    pub fn with_details(mut self, details: serde_json::Value) -> Self {
        self.details = Some(details);
        self
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn audit_record_serializes() {
        let r = AuditRecord::new(
            "user-123",
            PrincipalType::User,
            AuditAction::SessionCreated,
            "session",
            "sess-abc",
            AuditResult::Success,
        )
        .with_reason("normal creation");

        let json = serde_json::to_string(&r).unwrap();
        assert!(json.contains("session_created"));
        assert!(json.contains("normal creation"));
    }

    #[test]
    fn optional_fields_skipped_when_none() {
        let r = AuditRecord::new(
            "system",
            PrincipalType::System,
            AuditAction::SessionClosed,
            "session",
            "sess-xyz",
            AuditResult::Success,
        );
        let json = serde_json::to_string(&r).unwrap();
        assert!(!json.contains("reason"));
        assert!(!json.contains("correlation_id"));
        assert!(!json.contains("details"));
    }
}
