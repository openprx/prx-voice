//! Per-tenant usage quota tracking.

use parking_lot::RwLock;
use prx_voice_types::ids::TenantId;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Quota dimensions tracked per billing period.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct QuotaUsage {
    /// Total session minutes consumed.
    pub session_minutes: f64,
    /// ASR audio seconds consumed.
    pub asr_audio_seconds: f64,
    /// Agent input tokens consumed.
    pub agent_input_tokens: u64,
    /// Agent output tokens consumed.
    pub agent_output_tokens: u64,
    /// TTS characters consumed.
    pub tts_characters: u64,
    /// Current concurrent sessions.
    pub concurrent_sessions: u32,
}

/// Quota limits for a tenant.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QuotaLimits {
    pub max_session_minutes: f64,
    pub max_asr_audio_seconds: f64,
    pub max_agent_tokens: u64,
    pub max_tts_characters: u64,
    pub max_concurrent_sessions: u32,
}

impl Default for QuotaLimits {
    fn default() -> Self {
        // Starter tier defaults
        Self {
            max_session_minutes: 1000.0 * 60.0, // 1000 hours
            max_asr_audio_seconds: 60000.0,     // 1000 minutes
            max_agent_tokens: 2_000_000,
            max_tts_characters: 1_000_000,
            max_concurrent_sessions: 20,
        }
    }
}

/// Quota check result.
#[derive(Debug, Clone, PartialEq)]
pub enum QuotaCheckResult {
    Allowed,
    Exceeded {
        resource: String,
        current: f64,
        limit: f64,
    },
}

/// In-memory quota tracker per tenant.
pub struct QuotaTracker {
    usage: RwLock<HashMap<TenantId, QuotaUsage>>,
    limits: RwLock<HashMap<TenantId, QuotaLimits>>,
}

impl QuotaTracker {
    pub fn new() -> Self {
        Self {
            usage: RwLock::new(HashMap::new()),
            limits: RwLock::new(HashMap::new()),
        }
    }

    /// Set quota limits for a tenant.
    pub fn set_limits(&self, tenant_id: TenantId, limits: QuotaLimits) {
        self.limits.write().insert(tenant_id, limits);
    }

    /// Check if creating a new session is allowed.
    pub fn check_session_create(&self, tenant_id: TenantId) -> QuotaCheckResult {
        let usage = self.usage.read();
        let limits = self.limits.read();

        let current = usage
            .get(&tenant_id)
            .map(|u| u.concurrent_sessions)
            .unwrap_or(0);
        let limit = limits
            .get(&tenant_id)
            .map(|l| l.max_concurrent_sessions)
            .unwrap_or(20);

        if current >= limit {
            QuotaCheckResult::Exceeded {
                resource: "concurrent_sessions".into(),
                current: current as f64,
                limit: limit as f64,
            }
        } else {
            QuotaCheckResult::Allowed
        }
    }

    /// Record a session starting.
    pub fn record_session_start(&self, tenant_id: TenantId) {
        self.usage
            .write()
            .entry(tenant_id)
            .or_default()
            .concurrent_sessions += 1;
    }

    /// Record a session ending.
    pub fn record_session_end(&self, tenant_id: TenantId, duration_seconds: f64) {
        let mut usage = self.usage.write();
        let entry = usage.entry(tenant_id).or_default();
        entry.concurrent_sessions = entry.concurrent_sessions.saturating_sub(1);
        entry.session_minutes += duration_seconds / 60.0;
    }

    /// Record ASR usage.
    pub fn record_asr_usage(&self, tenant_id: TenantId, audio_seconds: f64) {
        self.usage
            .write()
            .entry(tenant_id)
            .or_default()
            .asr_audio_seconds += audio_seconds;
    }

    /// Record agent token usage.
    pub fn record_agent_tokens(&self, tenant_id: TenantId, input_tokens: u64, output_tokens: u64) {
        let mut usage = self.usage.write();
        let entry = usage.entry(tenant_id).or_default();
        entry.agent_input_tokens += input_tokens;
        entry.agent_output_tokens += output_tokens;
    }

    /// Record TTS character usage.
    pub fn record_tts_characters(&self, tenant_id: TenantId, chars: u64) {
        self.usage
            .write()
            .entry(tenant_id)
            .or_default()
            .tts_characters += chars;
    }

    /// Get current usage for a tenant.
    pub fn get_usage(&self, tenant_id: TenantId) -> QuotaUsage {
        self.usage
            .read()
            .get(&tenant_id)
            .cloned()
            .unwrap_or_default()
    }

    /// Get limits for a tenant.
    pub fn get_limits(&self, tenant_id: TenantId) -> QuotaLimits {
        self.limits
            .read()
            .get(&tenant_id)
            .cloned()
            .unwrap_or_default()
    }
}

impl Default for QuotaTracker {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn session_quota_enforced() {
        let tracker = QuotaTracker::new();
        let tid = TenantId::new();
        tracker.set_limits(
            tid,
            QuotaLimits {
                max_concurrent_sessions: 2,
                ..Default::default()
            },
        );

        assert_eq!(tracker.check_session_create(tid), QuotaCheckResult::Allowed);
        tracker.record_session_start(tid);
        tracker.record_session_start(tid);
        assert!(matches!(
            tracker.check_session_create(tid),
            QuotaCheckResult::Exceeded { .. }
        ));
    }

    #[test]
    fn session_end_frees_quota() {
        let tracker = QuotaTracker::new();
        let tid = TenantId::new();
        tracker.set_limits(
            tid,
            QuotaLimits {
                max_concurrent_sessions: 1,
                ..Default::default()
            },
        );

        tracker.record_session_start(tid);
        assert!(matches!(
            tracker.check_session_create(tid),
            QuotaCheckResult::Exceeded { .. }
        ));

        tracker.record_session_end(tid, 120.0);
        assert_eq!(tracker.check_session_create(tid), QuotaCheckResult::Allowed);
    }

    #[test]
    fn usage_accumulates() {
        let tracker = QuotaTracker::new();
        let tid = TenantId::new();

        tracker.record_asr_usage(tid, 30.5);
        tracker.record_asr_usage(tid, 20.0);
        tracker.record_agent_tokens(tid, 100, 50);
        tracker.record_tts_characters(tid, 500);

        let usage = tracker.get_usage(tid);
        assert_eq!(usage.asr_audio_seconds, 50.5);
        assert_eq!(usage.agent_input_tokens, 100);
        assert_eq!(usage.agent_output_tokens, 50);
        assert_eq!(usage.tts_characters, 500);
    }

    #[test]
    fn session_minutes_tracked() {
        let tracker = QuotaTracker::new();
        let tid = TenantId::new();

        tracker.record_session_start(tid);
        tracker.record_session_end(tid, 180.0); // 3 minutes

        let usage = tracker.get_usage(tid);
        assert_eq!(usage.session_minutes, 3.0);
        assert_eq!(usage.concurrent_sessions, 0);
    }
}
