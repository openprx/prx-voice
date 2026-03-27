//! Per-tenant policy configuration.

use parking_lot::RwLock;
use prx_voice_types::ids::TenantId;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Tier determining default quotas and features.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum TenantTier {
    Trial,
    Starter,
    Growth,
    Enterprise,
}

/// Per-tenant policy configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TenantPolicy {
    pub tenant_id: TenantId,
    pub tier: TenantTier,
    pub max_concurrent_sessions: u32,
    pub max_session_duration_sec: u64,
    pub max_turns_per_session: u32,
    pub interrupt_enabled: bool,
    pub recording_enabled: bool,
    /// ASR provider preference (primary, fallback).
    pub asr_providers: Vec<String>,
    /// Agent provider preference.
    pub agent_providers: Vec<String>,
    /// TTS provider preference.
    pub tts_providers: Vec<String>,
    /// Default language.
    pub default_language: String,
}

impl TenantPolicy {
    /// Create default policy for a tier.
    pub fn for_tier(tenant_id: TenantId, tier: TenantTier) -> Self {
        match tier {
            TenantTier::Trial => Self {
                tenant_id,
                tier,
                max_concurrent_sessions: 2,
                max_session_duration_sec: 300,
                max_turns_per_session: 20,
                interrupt_enabled: true,
                recording_enabled: false,
                asr_providers: vec!["mock".into()],
                agent_providers: vec!["mock".into()],
                tts_providers: vec!["mock".into()],
                default_language: "en-US".into(),
            },
            TenantTier::Starter => Self {
                tenant_id,
                tier,
                max_concurrent_sessions: 20,
                max_session_duration_sec: 1800,
                max_turns_per_session: 100,
                interrupt_enabled: true,
                recording_enabled: true,
                asr_providers: vec!["deepgram".into(), "mock".into()],
                agent_providers: vec!["openai".into(), "mock".into()],
                tts_providers: vec!["azure".into(), "mock".into()],
                default_language: "en-US".into(),
            },
            TenantTier::Growth => Self {
                tenant_id,
                tier,
                max_concurrent_sessions: 100,
                max_session_duration_sec: 3600,
                max_turns_per_session: 200,
                interrupt_enabled: true,
                recording_enabled: true,
                asr_providers: vec!["deepgram".into(), "google".into(), "mock".into()],
                agent_providers: vec!["openai".into(), "anthropic".into(), "mock".into()],
                tts_providers: vec!["azure".into(), "elevenlabs".into(), "mock".into()],
                default_language: "en-US".into(),
            },
            TenantTier::Enterprise => Self {
                tenant_id,
                tier,
                max_concurrent_sessions: 500,
                max_session_duration_sec: 7200,
                max_turns_per_session: 500,
                interrupt_enabled: true,
                recording_enabled: true,
                asr_providers: vec!["deepgram".into(), "google".into(), "mock".into()],
                agent_providers: vec!["openai".into(), "anthropic".into(), "mock".into()],
                tts_providers: vec!["azure".into(), "elevenlabs".into(), "mock".into()],
                default_language: "en-US".into(),
            },
        }
    }
}

/// In-memory tenant policy store.
pub struct TenantPolicyStore {
    policies: RwLock<HashMap<TenantId, TenantPolicy>>,
}

impl TenantPolicyStore {
    pub fn new() -> Self {
        Self {
            policies: RwLock::new(HashMap::new()),
        }
    }

    pub fn set(&self, policy: TenantPolicy) {
        self.policies.write().insert(policy.tenant_id, policy);
    }

    pub fn get(&self, tenant_id: TenantId) -> Option<TenantPolicy> {
        self.policies.read().get(&tenant_id).cloned()
    }

    pub fn remove(&self, tenant_id: TenantId) {
        self.policies.write().remove(&tenant_id);
    }
}

impl Default for TenantPolicyStore {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn trial_tier_has_low_limits() {
        let tid = TenantId::new();
        let p = TenantPolicy::for_tier(tid, TenantTier::Trial);
        assert_eq!(p.max_concurrent_sessions, 2);
        assert_eq!(p.max_session_duration_sec, 300);
    }

    #[test]
    fn enterprise_tier_has_high_limits() {
        let tid = TenantId::new();
        let p = TenantPolicy::for_tier(tid, TenantTier::Enterprise);
        assert_eq!(p.max_concurrent_sessions, 500);
    }

    #[test]
    fn policy_store_crud() {
        let store = TenantPolicyStore::new();
        let tid = TenantId::new();
        let policy = TenantPolicy::for_tier(tid, TenantTier::Starter);

        assert!(store.get(tid).is_none());
        store.set(policy);
        assert!(store.get(tid).is_some());
        assert_eq!(
            store.get(tid).expect("just inserted").tier,
            TenantTier::Starter
        );

        store.remove(tid);
        assert!(store.get(tid).is_none());
    }
}
