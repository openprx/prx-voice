//! GDPR/Compliance data export and deletion support.
//! Per the security spec: data subject access, right to erasure, audit export.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// Data Subject Access Request (DSAR).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DataSubjectRequest {
    pub request_id: String,
    pub request_type: DsarType,
    pub subject_identifier: SubjectIdentifier,
    pub tenant_id: String,
    pub requested_by: String,
    pub requested_at: DateTime<Utc>,
    pub status: DsarStatus,
    pub completed_at: Option<DateTime<Utc>>,
    pub notes: Option<String>,
}

/// Type of DSAR.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DsarType {
    /// Right of access (Art. 15 GDPR) — export all data.
    Access,
    /// Right to erasure (Art. 17 GDPR) — delete all data.
    Erasure,
    /// Right to rectification (Art. 16 GDPR) — correct data.
    Rectification,
    /// Right to data portability (Art. 20 GDPR) — export in machine-readable format.
    Portability,
    /// Right to restrict processing (Art. 18 GDPR).
    Restriction,
}

/// How to identify the data subject.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SubjectIdentifier {
    /// By phone number (SIP URI or E.164).
    Phone(String),
    /// By email address.
    Email(String),
    /// By session ID (for anonymous users).
    SessionId(String),
    /// By external user ID.
    UserId(String),
}

/// DSAR processing status.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DsarStatus {
    Received,
    Validating,
    Processing,
    Completed,
    Rejected,
    Failed,
}

impl DataSubjectRequest {
    pub fn new(
        request_type: DsarType,
        subject: SubjectIdentifier,
        tenant_id: impl Into<String>,
        requested_by: impl Into<String>,
    ) -> Self {
        Self {
            request_id: format!("DSAR-{}", &Uuid::new_v4().to_string()[..8]),
            request_type,
            subject_identifier: subject,
            tenant_id: tenant_id.into(),
            requested_by: requested_by.into(),
            requested_at: Utc::now(),
            status: DsarStatus::Received,
            completed_at: None,
            notes: None,
        }
    }
}

/// Data export package (for Access/Portability requests).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DataExportPackage {
    pub request_id: String,
    pub subject: SubjectIdentifier,
    pub export_format: ExportFormat,
    pub generated_at: DateTime<Utc>,
    pub sections: Vec<ExportSection>,
    pub total_records: usize,
}

/// Export format.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ExportFormat {
    Json,
    Csv,
}

/// A section in the export package.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExportSection {
    pub name: String,
    pub description: String,
    pub record_count: usize,
    pub data: serde_json::Value,
}

/// Deletion manifest (for Erasure requests).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeletionManifest {
    pub request_id: String,
    pub subject: SubjectIdentifier,
    pub items: Vec<DeletionItem>,
    pub total_items: usize,
    pub completed_at: Option<DateTime<Utc>>,
}

/// An item to be deleted.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeletionItem {
    pub resource_type: String,
    pub resource_id: String,
    pub status: DeletionItemStatus,
    pub reason: Option<String>,
}

/// Deletion status per item.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DeletionItemStatus {
    Pending,
    Deleted,
    Retained, // Legal hold or compliance requirement
    Failed,
}

/// Retention policy for compliance.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RetentionPolicy {
    pub name: String,
    /// Retention period in days. 0 = indefinite.
    pub retention_days: u32,
    /// Whether data can be deleted before retention expires.
    pub allow_early_deletion: bool,
    /// Legal hold overrides deletion.
    pub supports_legal_hold: bool,
    /// Data categories covered.
    pub categories: Vec<String>,
}

/// Default retention policies per the spec.
pub fn default_retention_policies() -> Vec<RetentionPolicy> {
    vec![
        RetentionPolicy {
            name: "session_metadata".into(),
            retention_days: 90,
            allow_early_deletion: true,
            supports_legal_hold: true,
            categories: vec!["sessions".into(), "turns".into()],
        },
        RetentionPolicy {
            name: "recordings".into(),
            retention_days: 30,
            allow_early_deletion: true,
            supports_legal_hold: true,
            categories: vec!["recordings".into()],
        },
        RetentionPolicy {
            name: "audit_logs".into(),
            retention_days: 2 * 365, // 2 years
            allow_early_deletion: false,
            supports_legal_hold: true,
            categories: vec!["audit".into()],
        },
        RetentionPolicy {
            name: "billing_records".into(),
            retention_days: 3 * 365, // 3 years
            allow_early_deletion: false,
            supports_legal_hold: true,
            categories: vec!["billing".into()],
        },
        RetentionPolicy {
            name: "debug_traces".into(),
            retention_days: 7,
            allow_early_deletion: true,
            supports_legal_hold: false,
            categories: vec!["debug".into(), "traces".into()],
        },
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn create_access_request() {
        let req = DataSubjectRequest::new(
            DsarType::Access,
            SubjectIdentifier::Email("user@example.com".into()),
            "tenant-1",
            "admin@company.com",
        );
        assert!(req.request_id.starts_with("DSAR-"));
        assert_eq!(req.status, DsarStatus::Received);
        assert_eq!(req.request_type, DsarType::Access);
    }

    #[test]
    fn create_erasure_request() {
        let req = DataSubjectRequest::new(
            DsarType::Erasure,
            SubjectIdentifier::Phone("+14155551234".into()),
            "tenant-1",
            "privacy-officer",
        );
        assert_eq!(req.request_type, DsarType::Erasure);
    }

    #[test]
    fn deletion_manifest() {
        let manifest = DeletionManifest {
            request_id: "DSAR-abc".into(),
            subject: SubjectIdentifier::UserId("user-123".into()),
            items: vec![
                DeletionItem {
                    resource_type: "session".into(),
                    resource_id: "sess-1".into(),
                    status: DeletionItemStatus::Deleted,
                    reason: None,
                },
                DeletionItem {
                    resource_type: "recording".into(),
                    resource_id: "rec-1".into(),
                    status: DeletionItemStatus::Retained,
                    reason: Some("Legal hold".into()),
                },
            ],
            total_items: 2,
            completed_at: None,
        };
        assert_eq!(manifest.items.len(), 2);
        assert_eq!(manifest.items[1].status, DeletionItemStatus::Retained);
    }

    #[test]
    fn default_retention_policies_complete() {
        let policies = default_retention_policies();
        assert_eq!(policies.len(), 5);
        // Audit logs: 2 years, no early deletion
        let audit = policies.iter().find(|p| p.name == "audit_logs").unwrap();
        assert_eq!(audit.retention_days, 730);
        assert!(!audit.allow_early_deletion);
    }

    #[test]
    fn dsar_type_serializes() {
        let json = serde_json::to_string(&DsarType::Portability).unwrap();
        assert_eq!(json, "\"portability\"");
    }
}
