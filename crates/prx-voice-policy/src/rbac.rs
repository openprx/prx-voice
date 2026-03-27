//! Role-Based Access Control (RBAC) permission enforcement.
//! Per the tenant-and-rbac spec: deny by default, explicit permission grants.

use serde::{Deserialize, Serialize};
use std::collections::HashSet;

/// Platform-level roles.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Role {
    /// Full platform access.
    PlatformAdmin,
    /// Tenant-level admin.
    TenantAdmin,
    /// Workspace admin.
    WorkspaceAdmin,
    /// Can create/manage sessions.
    WorkspaceOperator,
    /// Can view and test sessions.
    WorkspaceDeveloper,
    /// Can run QA tests, correct transcripts.
    WorkspaceQa,
    /// Read-only access.
    WorkspaceViewer,
    /// Billing access.
    BillingViewer,
}

/// Granular permissions.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Permission {
    SessionCreate,
    SessionRead,
    SessionClose,
    SessionInterrupt,
    SessionPause,
    SessionListAll,
    TurnRead,
    TranscriptCorrect,
    EventSubscribe,
    HandoffRequest,
    AuditRead,
    BillingRead,
    MetricsRead,
    TenantManage,
    PolicyManage,
    QuotaManage,
    RecordingDownload,
    DataExport,
}

/// Get the set of permissions for a role.
pub fn permissions_for_role(role: Role) -> HashSet<Permission> {
    use Permission::*;
    use Role::*;

    match role {
        PlatformAdmin => [
            SessionCreate,
            SessionRead,
            SessionClose,
            SessionInterrupt,
            SessionPause,
            SessionListAll,
            TurnRead,
            TranscriptCorrect,
            EventSubscribe,
            HandoffRequest,
            AuditRead,
            BillingRead,
            MetricsRead,
            TenantManage,
            PolicyManage,
            QuotaManage,
            RecordingDownload,
            DataExport,
        ]
        .into_iter()
        .collect(),

        TenantAdmin => [
            SessionCreate,
            SessionRead,
            SessionClose,
            SessionInterrupt,
            SessionPause,
            SessionListAll,
            TurnRead,
            TranscriptCorrect,
            EventSubscribe,
            HandoffRequest,
            AuditRead,
            BillingRead,
            MetricsRead,
            PolicyManage,
            QuotaManage,
            RecordingDownload,
            DataExport,
        ]
        .into_iter()
        .collect(),

        WorkspaceAdmin => [
            SessionCreate,
            SessionRead,
            SessionClose,
            SessionInterrupt,
            SessionPause,
            SessionListAll,
            TurnRead,
            TranscriptCorrect,
            EventSubscribe,
            HandoffRequest,
            AuditRead,
            MetricsRead,
            RecordingDownload,
        ]
        .into_iter()
        .collect(),

        WorkspaceOperator => [
            SessionCreate,
            SessionRead,
            SessionClose,
            SessionInterrupt,
            SessionPause,
            TurnRead,
            EventSubscribe,
            HandoffRequest,
            MetricsRead,
        ]
        .into_iter()
        .collect(),

        WorkspaceDeveloper => [
            SessionCreate,
            SessionRead,
            SessionClose,
            SessionInterrupt,
            TurnRead,
            EventSubscribe,
            MetricsRead,
        ]
        .into_iter()
        .collect(),

        WorkspaceQa => [
            SessionCreate,
            SessionRead,
            SessionClose,
            TurnRead,
            TranscriptCorrect,
            EventSubscribe,
        ]
        .into_iter()
        .collect(),

        WorkspaceViewer => [SessionRead, SessionListAll, TurnRead, MetricsRead]
            .into_iter()
            .collect(),

        BillingViewer => [BillingRead].into_iter().collect(),
    }
}

/// Check if a role has a specific permission.
pub fn has_permission(role: Role, permission: Permission) -> bool {
    permissions_for_role(role).contains(&permission)
}

/// Check if any of the given roles has the permission.
pub fn any_role_has_permission(roles: &[Role], permission: Permission) -> bool {
    roles.iter().any(|r| has_permission(*r, permission))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn platform_admin_has_all_permissions() {
        let perms = permissions_for_role(Role::PlatformAdmin);
        assert!(perms.contains(&Permission::SessionCreate));
        assert!(perms.contains(&Permission::TenantManage));
        assert!(perms.contains(&Permission::DataExport));
    }

    #[test]
    fn viewer_is_read_only() {
        let perms = permissions_for_role(Role::WorkspaceViewer);
        assert!(perms.contains(&Permission::SessionRead));
        assert!(!perms.contains(&Permission::SessionCreate));
        assert!(!perms.contains(&Permission::SessionClose));
        assert!(!perms.contains(&Permission::TranscriptCorrect));
    }

    #[test]
    fn qa_can_correct_transcripts() {
        assert!(has_permission(
            Role::WorkspaceQa,
            Permission::TranscriptCorrect
        ));
        assert!(!has_permission(
            Role::WorkspaceViewer,
            Permission::TranscriptCorrect
        ));
    }

    #[test]
    fn operator_can_pause() {
        assert!(has_permission(
            Role::WorkspaceOperator,
            Permission::SessionPause
        ));
        assert!(!has_permission(
            Role::WorkspaceViewer,
            Permission::SessionPause
        ));
    }

    #[test]
    fn billing_viewer_limited() {
        let perms = permissions_for_role(Role::BillingViewer);
        assert_eq!(perms.len(), 1);
        assert!(perms.contains(&Permission::BillingRead));
    }

    #[test]
    fn any_role_check() {
        let roles = vec![Role::WorkspaceViewer, Role::WorkspaceQa];
        assert!(any_role_has_permission(
            &roles,
            Permission::TranscriptCorrect
        )); // QA has it
        assert!(!any_role_has_permission(&roles, Permission::TenantManage)); // neither has it
    }
}
