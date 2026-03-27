//! Deployment strategies and database migration safety.

use serde::{Deserialize, Serialize};

/// Deployment strategy.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DeployStrategy {
    RollingUpdate,
    BlueGreen,
    Canary,
}

/// Canary rollout step.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CanaryStep {
    pub name: String,
    pub traffic_pct: u8,
    pub min_duration_minutes: u32,
    pub success_criteria: Vec<String>,
}

/// Default canary rollout per deployment spec.
pub fn default_canary_steps() -> Vec<CanaryStep> {
    vec![
        CanaryStep {
            name: "Internal test tenant".into(),
            traffic_pct: 0,
            min_duration_minutes: 30,
            success_criteria: vec!["No errors in test sessions".into()],
        },
        CanaryStep {
            name: "Low-risk tenant".into(),
            traffic_pct: 1,
            min_duration_minutes: 30,
            success_criteria: vec!["Session success rate > 99.9%".into()],
        },
        CanaryStep {
            name: "Canary group".into(),
            traffic_pct: 5,
            min_duration_minutes: 60,
            success_criteria: vec![
                "No latency regression".into(),
                "No fallback increase".into(),
            ],
        },
        CanaryStep {
            name: "Partial production".into(),
            traffic_pct: 25,
            min_duration_minutes: 60,
            success_criteria: vec!["SLI within budget".into()],
        },
        CanaryStep {
            name: "Majority production".into(),
            traffic_pct: 50,
            min_duration_minutes: 30,
            success_criteria: vec!["No memory growth".into()],
        },
        CanaryStep {
            name: "Full rollout".into(),
            traffic_pct: 100,
            min_duration_minutes: 0,
            success_criteria: vec!["Stable for 24h".into()],
        },
    ]
}

/// Database migration safety pattern: expand-migrate-contract.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MigrationPhase {
    /// Add new column/index (backward compatible).
    Expand,
    /// Migrate data in transactions.
    Migrate,
    /// Remove old column/rename new (forward-only).
    Contract,
}

/// Rollback conditions per deployment spec.
pub fn immediate_rollback_conditions() -> Vec<&'static str> {
    vec![
        "Core chain availability < SLO",
        "Multi-tenant blast radius detected",
        "Key latency > threshold for 5 minutes",
        "Billing/audit chain broken",
        "Cancel/interrupt broken",
        "Session state corruption detected",
    ]
}

/// Release gating checklist.
pub fn release_gate_checklist() -> Vec<&'static str> {
    vec![
        "Code freeze and CI build pass",
        "Security and dependency scan pass",
        "Staging deployment successful",
        "Schema compatibility verified",
        "No performance regression",
        "Runbook updated",
        "Monitoring and alerts synced",
        "Error budget state is GREEN",
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn canary_steps_progress() {
        let steps = default_canary_steps();
        assert_eq!(steps.len(), 6);
        assert_eq!(steps.last().map(|s| s.traffic_pct), Some(100));
        // Traffic should be monotonically increasing
        for i in 1..steps.len() {
            assert!(steps[i].traffic_pct >= steps[i - 1].traffic_pct);
        }
    }

    #[test]
    fn rollback_conditions_exist() {
        let conditions = immediate_rollback_conditions();
        assert!(conditions.len() >= 5);
    }

    #[test]
    fn release_checklist_complete() {
        let checklist = release_gate_checklist();
        assert!(checklist.len() >= 7);
        assert!(checklist.iter().any(|c| c.contains("Error budget")));
    }
}
