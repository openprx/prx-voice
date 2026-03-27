//! Pricing tiers, overage policies, and invoice generation.

use serde::{Deserialize, Serialize};

/// Pricing tier per the billing spec.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PricingTier {
    pub name: String,
    pub monthly_fee_cny: f64,
    pub included: IncludedQuotas,
    pub overage: OverageRates,
    pub sla_target_pct: f64,
    pub data_retention_days: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IncludedQuotas {
    pub concurrent_sessions: u32,
    pub asr_minutes: u64,
    pub agent_tokens: u64,
    pub tts_characters: u64,
    pub recording_storage_gb: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OverageRates {
    pub session_per_min_cny: f64,
    pub asr_per_min_cny: f64,
    pub agent_input_per_1k_tokens_cny: f64,
    pub agent_output_per_1k_tokens_cny: f64,
    pub tts_per_1k_chars_cny: f64,
    pub recording_per_gb_month_cny: f64,
    pub handoff_per_event_cny: f64,
}

/// Default pricing tiers per the billing spec.
pub fn default_tiers() -> Vec<PricingTier> {
    vec![
        PricingTier {
            name: "Trial".into(),
            monthly_fee_cny: 0.0,
            included: IncludedQuotas {
                concurrent_sessions: 2,
                asr_minutes: 30,
                agent_tokens: 100_000,
                tts_characters: 50_000,
                recording_storage_gb: 0,
            },
            overage: OverageRates {
                session_per_min_cny: 0.0,
                asr_per_min_cny: 0.0,
                agent_input_per_1k_tokens_cny: 0.0,
                agent_output_per_1k_tokens_cny: 0.0,
                tts_per_1k_chars_cny: 0.0,
                recording_per_gb_month_cny: 0.0,
                handoff_per_event_cny: 0.0,
            },
            sla_target_pct: 0.0,
            data_retention_days: 1,
        },
        PricingTier {
            name: "Starter".into(),
            monthly_fee_cny: 999.0,
            included: IncludedQuotas {
                concurrent_sessions: 20,
                asr_minutes: 1_000,
                agent_tokens: 2_000_000,
                tts_characters: 1_000_000,
                recording_storage_gb: 10,
            },
            overage: OverageRates {
                session_per_min_cny: 0.12,
                asr_per_min_cny: 0.10,
                agent_input_per_1k_tokens_cny: 0.02,
                agent_output_per_1k_tokens_cny: 0.08,
                tts_per_1k_chars_cny: 0.12,
                recording_per_gb_month_cny: 2.0,
                handoff_per_event_cny: 1.5,
            },
            sla_target_pct: 99.5,
            data_retention_days: 7,
        },
        PricingTier {
            name: "Growth".into(),
            monthly_fee_cny: 4999.0,
            included: IncludedQuotas {
                concurrent_sessions: 100,
                asr_minutes: 8_000,
                agent_tokens: 15_000_000,
                tts_characters: 8_000_000,
                recording_storage_gb: 100,
            },
            overage: OverageRates {
                session_per_min_cny: 0.10,
                asr_per_min_cny: 0.08,
                agent_input_per_1k_tokens_cny: 0.015,
                agent_output_per_1k_tokens_cny: 0.06,
                tts_per_1k_chars_cny: 0.10,
                recording_per_gb_month_cny: 1.5,
                handoff_per_event_cny: 1.2,
            },
            sla_target_pct: 99.9,
            data_retention_days: 30,
        },
        PricingTier {
            name: "Enterprise".into(),
            monthly_fee_cny: -1.0, // custom
            included: IncludedQuotas {
                concurrent_sessions: 500,
                asr_minutes: 0,
                agent_tokens: 0,
                tts_characters: 0,
                recording_storage_gb: 0,
            },
            overage: OverageRates {
                session_per_min_cny: 0.0,
                asr_per_min_cny: 0.0,
                agent_input_per_1k_tokens_cny: 0.0,
                agent_output_per_1k_tokens_cny: 0.0,
                tts_per_1k_chars_cny: 0.0,
                recording_per_gb_month_cny: 0.0,
                handoff_per_event_cny: 0.0,
            },
            sla_target_pct: 99.95,
            data_retention_days: 90,
        },
    ]
}

/// Overage policy action.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum OverageAction {
    AlertAndContinue,
    ThrottleNewSessions,
    Block,
}

/// SLA credit schedule.
pub fn sla_credit(uptime_pct: f64) -> f64 {
    if uptime_pct >= 99.9 {
        0.0
    } else if uptime_pct >= 99.0 {
        10.0
    } else if uptime_pct >= 95.0 {
        25.0
    } else {
        50.0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_tiers_correct() {
        let tiers = default_tiers();
        assert_eq!(tiers.len(), 4);
        assert_eq!(tiers[1].name, "Starter");
        assert_eq!(tiers[1].monthly_fee_cny, 999.0);
        assert_eq!(tiers[1].included.asr_minutes, 1000);
    }

    #[test]
    fn sla_credits() {
        assert_eq!(sla_credit(99.95), 0.0);
        assert_eq!(sla_credit(99.5), 10.0);
        assert_eq!(sla_credit(97.0), 25.0);
        assert_eq!(sla_credit(90.0), 50.0);
    }
}
