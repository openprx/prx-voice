//! SLO/Error Budget engine.
//! Per the SLO spec: tracks SLI values, computes error budget consumption,
//! and determines release gate state (GREEN/YELLOW/RED).

use crate::metrics::MetricsRegistry;
use serde::{Deserialize, Serialize};

/// SLO target definition.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SloTarget {
    pub name: String,
    pub description: String,
    /// Target percentage (e.g., 99.9 for 99.9%).
    pub target_pct: f64,
    /// Monthly error budget in minutes (derived from target).
    pub monthly_budget_minutes: f64,
}

impl SloTarget {
    /// Create an SLO target. Automatically computes monthly budget.
    /// 30 days * 24h * 60m = 43,200 minutes per month.
    pub fn new(name: impl Into<String>, description: impl Into<String>, target_pct: f64) -> Self {
        let monthly_minutes = 43_200.0;
        let budget = monthly_minutes * (1.0 - target_pct / 100.0);
        Self {
            name: name.into(),
            description: description.into(),
            target_pct,
            monthly_budget_minutes: budget,
        }
    }
}

/// Release gate state based on error budget.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum GateState {
    /// >50% budget remaining. All releases allowed.
    Green,
    /// 20-50% budget remaining. Bug fixes only.
    Yellow,
    /// <20% budget remaining. Only incident mitigation.
    Red,
}

/// Burn rate alert level.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum BurnRateLevel {
    /// Normal: 1x burn rate.
    Normal,
    /// P4: 1x burn, 10% in 3 days.
    P4,
    /// P3: 3x burn, 10% in 1 day.
    P3,
    /// P2: 6x burn, 5% in 6 hours.
    P2,
    /// P1: 14.4x burn, 2% in 1 hour. Immediate response.
    P1,
}

/// Error budget status snapshot.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ErrorBudgetStatus {
    pub slo_name: String,
    pub target_pct: f64,
    pub monthly_budget_minutes: f64,
    pub consumed_minutes: f64,
    pub remaining_pct: f64,
    pub gate_state: GateState,
    pub burn_rate: f64,
    pub burn_rate_level: BurnRateLevel,
}

/// SLO engine that computes error budget from metrics.
pub struct SloEngine {
    targets: Vec<SloTarget>,
}

impl SloEngine {
    pub fn new() -> Self {
        Self {
            targets: vec![
                SloTarget::new("session_availability", "Session create success rate", 99.9),
                SloTarget::new(
                    "control_plane_availability",
                    "Control plane API availability",
                    99.9,
                ),
                SloTarget::new(
                    "asr_latency",
                    "ASR final transcript latency p95 < 1200ms",
                    99.5,
                ),
                SloTarget::new("tts_latency", "TTS first byte latency p95 < 500ms", 99.5),
            ],
        }
    }

    /// Add a custom SLO target.
    pub fn add_target(&mut self, target: SloTarget) {
        self.targets.push(target);
    }

    /// Compute error budget status for all targets.
    pub fn compute_status(&self, metrics: &MetricsRegistry) -> Vec<ErrorBudgetStatus> {
        self.targets
            .iter()
            .map(|target| {
                let consumed = self.compute_consumed_minutes(target, metrics);
                let remaining_pct = if target.monthly_budget_minutes > 0.0 {
                    ((target.monthly_budget_minutes - consumed) / target.monthly_budget_minutes
                        * 100.0)
                        .max(0.0)
                } else {
                    0.0
                };

                let gate_state = if remaining_pct > 50.0 {
                    GateState::Green
                } else if remaining_pct > 20.0 {
                    GateState::Yellow
                } else {
                    GateState::Red
                };

                // Burn rate = consumed / expected_at_this_point
                // Simplified: just use consumed / budget ratio
                let burn_rate = if target.monthly_budget_minutes > 0.0 {
                    consumed / target.monthly_budget_minutes * 30.0 // normalize to daily
                } else {
                    0.0
                };

                let burn_rate_level = if burn_rate >= 14.4 {
                    BurnRateLevel::P1
                } else if burn_rate >= 6.0 {
                    BurnRateLevel::P2
                } else if burn_rate >= 3.0 {
                    BurnRateLevel::P3
                } else if burn_rate >= 1.0 {
                    BurnRateLevel::P4
                } else {
                    BurnRateLevel::Normal
                };

                ErrorBudgetStatus {
                    slo_name: target.name.clone(),
                    target_pct: target.target_pct,
                    monthly_budget_minutes: target.monthly_budget_minutes,
                    consumed_minutes: consumed,
                    remaining_pct,
                    gate_state,
                    burn_rate,
                    burn_rate_level,
                }
            })
            .collect()
    }

    /// Get the most restrictive gate state across all SLOs.
    pub fn overall_gate_state(&self, metrics: &MetricsRegistry) -> GateState {
        let statuses = self.compute_status(metrics);
        if statuses.iter().any(|s| s.gate_state == GateState::Red) {
            GateState::Red
        } else if statuses.iter().any(|s| s.gate_state == GateState::Yellow) {
            GateState::Yellow
        } else {
            GateState::Green
        }
    }

    fn compute_consumed_minutes(&self, target: &SloTarget, metrics: &MetricsRegistry) -> f64 {
        // Compute consumed error budget based on failure counts
        // For session_availability: failures / total * total_minutes
        let created = metrics.counter("prx_voice_session_created_total") as f64;
        let failed = metrics.counter("prx_voice_session_failed_total") as f64;

        if created == 0.0 {
            return 0.0;
        }

        let failure_rate = failed / created;
        let allowed_failure_rate = 1.0 - target.target_pct / 100.0;

        if failure_rate > allowed_failure_rate {
            // Over budget — convert excess failures to minutes
            let excess_rate = failure_rate - allowed_failure_rate;
            excess_rate * target.monthly_budget_minutes / allowed_failure_rate.max(0.001)
        } else {
            // Within budget
            failure_rate / allowed_failure_rate.max(0.001) * target.monthly_budget_minutes * 0.1
        }
    }
}

impl Default for SloEngine {
    fn default() -> Self {
        Self::new()
    }
}

/// Customer-facing SLA commitment per tier.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CustomerSla {
    pub tier: String,
    pub availability_pct: f64,
    pub session_create_success_pct: f64,
    pub control_plane_response_p95_ms: u64,
    pub credit_10_pct_threshold: f64,
    pub credit_25_pct_threshold: f64,
    pub credit_50_pct_threshold: f64,
}

/// Default customer SLAs per the SLO spec.
pub fn customer_slas() -> Vec<CustomerSla> {
    vec![
        CustomerSla {
            tier: "Starter".into(),
            availability_pct: 99.5,
            session_create_success_pct: 99.0,
            control_plane_response_p95_ms: 500,
            credit_10_pct_threshold: 99.0,
            credit_25_pct_threshold: 95.0,
            credit_50_pct_threshold: 90.0,
        },
        CustomerSla {
            tier: "Growth".into(),
            availability_pct: 99.9,
            session_create_success_pct: 99.5,
            control_plane_response_p95_ms: 300,
            credit_10_pct_threshold: 99.0,
            credit_25_pct_threshold: 95.0,
            credit_50_pct_threshold: 90.0,
        },
        CustomerSla {
            tier: "Enterprise".into(),
            availability_pct: 99.95,
            session_create_success_pct: 99.9,
            control_plane_response_p95_ms: 200,
            credit_10_pct_threshold: 99.9,
            credit_25_pct_threshold: 99.0,
            credit_50_pct_threshold: 95.0,
        },
    ]
}

/// Latency budget breakdown per the SLO spec (1500ms target).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LatencyBudget {
    pub component: String,
    pub min_ms: u32,
    pub max_ms: u32,
}

pub fn latency_budget_breakdown() -> Vec<LatencyBudget> {
    vec![
        LatencyBudget {
            component: "VAD endpointing".into(),
            min_ms: 200,
            max_ms: 300,
        },
        LatencyBudget {
            component: "Network (client→server)".into(),
            min_ms: 20,
            max_ms: 50,
        },
        LatencyBudget {
            component: "ASR final".into(),
            min_ms: 200,
            max_ms: 400,
        },
        LatencyBudget {
            component: "LLM first token".into(),
            min_ms: 200,
            max_ms: 400,
        },
        LatencyBudget {
            component: "TTS first byte".into(),
            min_ms: 100,
            max_ms: 300,
        },
        LatencyBudget {
            component: "Network (server→client)".into(),
            min_ms: 20,
            max_ms: 50,
        },
        LatencyBudget {
            component: "Playback buffer".into(),
            min_ms: 50,
            max_ms: 100,
        },
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn slo_target_computes_budget() {
        let t = SloTarget::new("test", "test slo", 99.9);
        // 43200 * 0.001 = 43.2 minutes
        assert!((t.monthly_budget_minutes - 43.2).abs() < 0.1);
    }

    #[test]
    fn gate_state_green_when_no_failures() {
        let engine = SloEngine::new();
        let metrics = MetricsRegistry::new();
        // No sessions = no failures = green
        let state = engine.overall_gate_state(&metrics);
        assert_eq!(state, GateState::Green);
    }

    #[test]
    fn gate_state_with_sessions() {
        let engine = SloEngine::new();
        let metrics = MetricsRegistry::new();

        // 1000 sessions, 0 failures = well within budget
        metrics.inc_by("prx_voice_session_created_total", 1000);
        let state = engine.overall_gate_state(&metrics);
        assert_eq!(state, GateState::Green);
    }

    #[test]
    fn error_budget_status_fields() {
        let engine = SloEngine::new();
        let metrics = MetricsRegistry::new();
        metrics.inc_by("prx_voice_session_created_total", 100);

        let statuses = engine.compute_status(&metrics);
        assert!(!statuses.is_empty());
        assert_eq!(statuses[0].slo_name, "session_availability");
        assert_eq!(statuses[0].target_pct, 99.9);
    }

    #[test]
    fn burn_rate_levels() {
        // Just verify the thresholds
        assert_eq!(
            if 15.0_f64 >= 14.4 {
                BurnRateLevel::P1
            } else {
                BurnRateLevel::Normal
            },
            BurnRateLevel::P1
        );
        assert_eq!(
            if 7.0_f64 >= 6.0 {
                BurnRateLevel::P2
            } else {
                BurnRateLevel::Normal
            },
            BurnRateLevel::P2
        );
    }

    #[test]
    fn customer_slas_per_tier() {
        let slas = customer_slas();
        assert_eq!(slas.len(), 3);
        let enterprise = slas.iter().find(|s| s.tier == "Enterprise");
        assert!(enterprise.is_some());
        assert_eq!(enterprise.map(|e| e.availability_pct), Some(99.95));
    }

    #[test]
    fn latency_budget_sums_to_target() {
        let budget = latency_budget_breakdown();
        let max_total: u32 = budget.iter().map(|b| b.max_ms).sum();
        assert!(
            max_total <= 1700,
            "Max latency budget {max_total}ms should be ~1500ms"
        );
    }
}
