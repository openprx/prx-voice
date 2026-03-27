//! Degradation mode management per operations runbook.
//! 4 levels of service degradation with entry/exit criteria.

use serde::{Deserialize, Serialize};

/// Degradation level per runbook spec.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub enum DegradationLevel {
    /// Normal operation.
    Normal,
    /// Level 1: Reduce debug events, trace verbosity, non-critical tenant limits.
    Level1,
    /// Level 2: Use low-cost providers, limit premium features.
    Level2,
    /// Level 3: Disable optional features (replay, analytics, transcript correction).
    Level3,
    /// Level 4: Stop new sessions, drain existing (max 30 min).
    Level4,
}

/// Degradation state with entry/exit criteria.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DegradationState {
    pub level: DegradationLevel,
    pub reason: String,
    pub entered_at: String,
    pub auto_exit_criteria: Option<String>,
}

/// Actions per degradation level.
pub fn actions_for_level(level: DegradationLevel) -> Vec<&'static str> {
    match level {
        DegradationLevel::Normal => vec!["All systems operational"],
        DegradationLevel::Level1 => vec![
            "Reduce debug event emission",
            "Lower trace verbosity to warn",
            "Limit non-critical tenant session rates",
        ],
        DegradationLevel::Level2 => vec![
            "Switch to low-cost ASR/TTS providers",
            "Limit premium Agent models",
            "Reduce Agent context window size",
            "Disable non-essential analytics",
        ],
        DegradationLevel::Level3 => vec![
            "Disable transcript correction",
            "Disable session replay export",
            "Disable analytics export",
            "Disable recording for non-enterprise tenants",
        ],
        DegradationLevel::Level4 => vec![
            "Reject all new session creation",
            "Complete existing sessions (max 30 min)",
            "Drain all adapter connections",
            "Emit critical alert to all channels",
        ],
    }
}

/// Rollback SLAs per the runbook.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RollbackSla {
    pub target: String,
    pub max_duration_minutes: u32,
}

pub fn rollback_slas() -> Vec<RollbackSla> {
    vec![
        RollbackSla {
            target: "Config change".into(),
            max_duration_minutes: 1,
        },
        RollbackSla {
            target: "Routing policy".into(),
            max_duration_minutes: 2,
        },
        RollbackSla {
            target: "Feature flag".into(),
            max_duration_minutes: 1,
        },
        RollbackSla {
            target: "Single service binary".into(),
            max_duration_minutes: 5,
        },
        RollbackSla {
            target: "Multi-service rollback".into(),
            max_duration_minutes: 10,
        },
        RollbackSla {
            target: "Database migration".into(),
            max_duration_minutes: 30,
        },
    ]
}

/// Chaos test scenario.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChaosScenario {
    pub name: String,
    pub description: String,
    pub target: String,
    pub failure_type: String,
    pub duration_sec: u32,
    pub expected_behavior: String,
}

pub fn standard_chaos_scenarios() -> Vec<ChaosScenario> {
    vec![
        ChaosScenario {
            name: "ASR provider outage".into(),
            description: "Kill ASR adapter connections".into(),
            target: "asr-adapter".into(),
            failure_type: "connection_reset".into(),
            duration_sec: 300,
            expected_behavior: "Fallback to backup ASR provider".into(),
        },
        ChaosScenario {
            name: "Agent timeout".into(),
            description: "Inject 60s latency into Agent adapter".into(),
            target: "agent-adapter".into(),
            failure_type: "latency_injection".into(),
            duration_sec: 120,
            expected_behavior: "Agent timeout → fallback response".into(),
        },
        ChaosScenario {
            name: "TTS rate limit".into(),
            description: "Return 429 from TTS provider".into(),
            target: "tts-adapter".into(),
            failure_type: "rate_limit".into(),
            duration_sec: 60,
            expected_behavior: "Fallback to backup TTS or text-only".into(),
        },
        ChaosScenario {
            name: "Database unreachable".into(),
            description: "Block PostgreSQL connections".into(),
            target: "postgresql".into(),
            failure_type: "network_partition".into(),
            duration_sec: 180,
            expected_behavior: "In-memory fallback, no new sessions persisted".into(),
        },
        ChaosScenario {
            name: "Memory pressure".into(),
            description: "Allocate memory to 90% limit".into(),
            target: "orchestrator".into(),
            failure_type: "resource_exhaustion".into(),
            duration_sec: 60,
            expected_behavior: "Session budget enforcement, graceful rejection".into(),
        },
        ChaosScenario {
            name: "Region failover".into(),
            description: "Mark primary region unhealthy".into(),
            target: "load-balancer".into(),
            failure_type: "health_check_failure".into(),
            duration_sec: 300,
            expected_behavior: "Traffic shifts to secondary region".into(),
        },
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn degradation_levels_ordered() {
        assert!(DegradationLevel::Normal < DegradationLevel::Level1);
        assert!(DegradationLevel::Level3 < DegradationLevel::Level4);
    }

    #[test]
    fn level4_stops_new_sessions() {
        let actions = actions_for_level(DegradationLevel::Level4);
        assert!(actions.iter().any(|a| a.contains("Reject all new session")));
    }

    #[test]
    fn rollback_slas_complete() {
        let slas = rollback_slas();
        assert_eq!(slas.len(), 6);
        assert_eq!(slas[0].max_duration_minutes, 1); // config = 1 min
    }

    #[test]
    fn chaos_scenarios_exist() {
        let scenarios = standard_chaos_scenarios();
        assert_eq!(scenarios.len(), 6);
        assert!(scenarios.iter().any(|s| s.name.contains("ASR")));
    }
}
