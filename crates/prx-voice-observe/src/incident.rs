//! Incident detection, classification, and response.
//! Per the incident response spec: SEV-1 through SEV-4.

use chrono::{DateTime, Utc};
use parking_lot::RwLock;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use uuid::Uuid;

/// Incident severity per spec.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub enum Severity {
    /// Broad outage, major data risk, security breach.
    Sev1,
    /// Major degradation (>10% users), dependency outage.
    Sev2,
    /// Partial degradation (<10%), high-value customer affected.
    Sev3,
    /// Low-impact, isolated anomaly.
    Sev4,
}

/// Incident status.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum IncidentStatus {
    Detected,
    Acknowledged,
    Investigating,
    Mitigating,
    Resolved,
    PostMortem,
    Closed,
}

/// Incident category.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum IncidentCategory {
    AvailabilityOutage,
    LatencyDegradation,
    MediaPathFailure,
    ProviderFailure,
    ControlPlaneOutage,
    BillingAuditRisk,
    DataCorruption,
    SecurityBreach,
    TenantIsolationBreach,
    RegionOutage,
    DeploymentRegression,
    CapacityExhaustion,
}

/// Mitigation strategy.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MitigationStrategy {
    Rollback,
    TrafficShift,
    FeatureDegradation,
    ProviderFallback,
    CapacityProtection,
    TenantIsolation,
}

/// An incident record.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Incident {
    pub id: String,
    pub severity: Severity,
    pub status: IncidentStatus,
    pub category: IncidentCategory,
    pub title: String,
    pub description: String,
    pub affected_tenants: Vec<String>,
    pub mitigation: Option<MitigationStrategy>,
    pub commander: Option<String>,
    pub detected_at: DateTime<Utc>,
    pub acknowledged_at: Option<DateTime<Utc>>,
    pub resolved_at: Option<DateTime<Utc>>,
    pub timeline: Vec<TimelineEntry>,
}

/// Entry in the incident timeline.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TimelineEntry {
    pub timestamp: DateTime<Utc>,
    pub actor: String,
    pub action: String,
    pub details: Option<String>,
}

/// Escalation rule based on severity.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EscalationRule {
    pub severity: Severity,
    pub responder: String,
    pub response_time_minutes: u32,
}

/// Default escalation path per spec.
pub fn default_escalation_path() -> Vec<EscalationRule> {
    vec![
        EscalationRule {
            severity: Severity::Sev4,
            responder: "Primary on-call".into(),
            response_time_minutes: 30,
        },
        EscalationRule {
            severity: Severity::Sev3,
            responder: "Primary on-call".into(),
            response_time_minutes: 15,
        },
        EscalationRule {
            severity: Severity::Sev2,
            responder: "SRE Lead".into(),
            response_time_minutes: 10,
        },
        EscalationRule {
            severity: Severity::Sev1,
            responder: "VP Engineering".into(),
            response_time_minutes: 5,
        },
    ]
}

/// Communication SLA per severity.
pub fn communication_sla(severity: Severity) -> CommunicationSla {
    match severity {
        Severity::Sev1 => CommunicationSla {
            initial_minutes: 15,
            update_minutes: 30,
            resolution_report_days: 5,
        },
        Severity::Sev2 => CommunicationSla {
            initial_minutes: 30,
            update_minutes: 60,
            resolution_report_days: 10,
        },
        Severity::Sev3 => CommunicationSla {
            initial_minutes: 120,
            update_minutes: 1440,
            resolution_report_days: 0,
        },
        Severity::Sev4 => CommunicationSla {
            initial_minutes: 1440,
            update_minutes: 10080,
            resolution_report_days: 0,
        },
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CommunicationSla {
    pub initial_minutes: u32,
    pub update_minutes: u32,
    pub resolution_report_days: u32,
}

/// In-memory incident tracker.
pub struct IncidentTracker {
    incidents: RwLock<HashMap<String, Incident>>,
}

impl IncidentTracker {
    pub fn new() -> Self {
        Self {
            incidents: RwLock::new(HashMap::new()),
        }
    }

    /// Create a new incident.
    pub fn create(
        &self,
        severity: Severity,
        category: IncidentCategory,
        title: impl Into<String>,
        description: impl Into<String>,
    ) -> Incident {
        let now = Utc::now();
        let incident = Incident {
            id: format!("INC-{}", &Uuid::new_v4().to_string()[..8]),
            severity,
            status: IncidentStatus::Detected,
            category,
            title: title.into(),
            description: description.into(),
            affected_tenants: Vec::new(),
            mitigation: None,
            commander: None,
            detected_at: now,
            acknowledged_at: None,
            resolved_at: None,
            timeline: vec![TimelineEntry {
                timestamp: now,
                actor: "system".into(),
                action: "Incident detected".into(),
                details: None,
            }],
        };
        self.incidents
            .write()
            .insert(incident.id.clone(), incident.clone());
        incident
    }

    /// Acknowledge an incident.
    pub fn acknowledge(&self, id: &str, commander: impl Into<String>) -> Option<Incident> {
        let mut incidents = self.incidents.write();
        let inc = incidents.get_mut(id)?;
        let now = Utc::now();
        inc.status = IncidentStatus::Acknowledged;
        inc.acknowledged_at = Some(now);
        inc.commander = Some(commander.into());
        inc.timeline.push(TimelineEntry {
            timestamp: now,
            actor: inc.commander.clone().unwrap_or_default(),
            action: "Incident acknowledged".into(),
            details: None,
        });
        Some(inc.clone())
    }

    /// Resolve an incident.
    pub fn resolve(&self, id: &str, details: impl Into<String>) -> Option<Incident> {
        let mut incidents = self.incidents.write();
        let inc = incidents.get_mut(id)?;
        let now = Utc::now();
        inc.status = IncidentStatus::Resolved;
        inc.resolved_at = Some(now);
        inc.timeline.push(TimelineEntry {
            timestamp: now,
            actor: inc.commander.clone().unwrap_or_else(|| "system".into()),
            action: "Incident resolved".into(),
            details: Some(details.into()),
        });
        Some(inc.clone())
    }

    /// Get an incident by ID.
    pub fn get(&self, id: &str) -> Option<Incident> {
        self.incidents.read().get(id).cloned()
    }

    /// List active (non-resolved/closed) incidents.
    pub fn list_active(&self) -> Vec<Incident> {
        self.incidents
            .read()
            .values()
            .filter(|i| !matches!(i.status, IncidentStatus::Resolved | IncidentStatus::Closed))
            .cloned()
            .collect()
    }

    /// Count by severity.
    pub fn count_by_severity(&self) -> HashMap<Severity, usize> {
        let mut counts = HashMap::new();
        for inc in self.incidents.read().values() {
            *counts.entry(inc.severity).or_insert(0) += 1;
        }
        counts
    }
}

impl Default for IncidentTracker {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn incident_lifecycle() {
        let tracker = IncidentTracker::new();
        let inc = tracker.create(
            Severity::Sev2,
            IncidentCategory::ProviderFailure,
            "ASR Provider Down",
            "Deepgram returning 503",
        );
        assert_eq!(inc.status, IncidentStatus::Detected);
        assert!(inc.id.starts_with("INC-"));

        tracker.acknowledge(&inc.id, "ops-lead");
        let acked = tracker.get(&inc.id).unwrap();
        assert_eq!(acked.status, IncidentStatus::Acknowledged);
        assert_eq!(acked.commander.as_deref(), Some("ops-lead"));

        tracker.resolve(&inc.id, "Deepgram recovered, sessions resumed");
        let resolved = tracker.get(&inc.id).unwrap();
        assert_eq!(resolved.status, IncidentStatus::Resolved);
        assert!(resolved.resolved_at.is_some());
        assert_eq!(resolved.timeline.len(), 3);
    }

    #[test]
    fn list_active_excludes_resolved() {
        let tracker = IncidentTracker::new();
        let i1 = tracker.create(
            Severity::Sev3,
            IncidentCategory::LatencyDegradation,
            "Slow ASR",
            "p95 > 2s",
        );
        let i2 = tracker.create(
            Severity::Sev4,
            IncidentCategory::CapacityExhaustion,
            "High load",
            "80% capacity",
        );
        tracker.resolve(&i1.id, "fixed");

        let active = tracker.list_active();
        assert_eq!(active.len(), 1);
        assert_eq!(active[0].id, i2.id);
    }

    #[test]
    fn escalation_rules() {
        let rules = default_escalation_path();
        assert_eq!(rules.len(), 4);
        assert_eq!(rules[3].severity, Severity::Sev1);
        assert_eq!(rules[3].response_time_minutes, 5);
    }

    #[test]
    fn communication_sla_sev1() {
        let sla = communication_sla(Severity::Sev1);
        assert_eq!(sla.initial_minutes, 15);
        assert_eq!(sla.update_minutes, 30);
        assert_eq!(sla.resolution_report_days, 5);
    }
}
