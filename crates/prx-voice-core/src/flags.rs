//! Feature flag system for canary/gradual rollout.
//! Flags can be global, per-tenant, or percentage-based.

use parking_lot::RwLock;
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};

/// Feature flag definition.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FeatureFlag {
    pub name: String,
    pub description: String,
    pub enabled: bool,
    /// If set, only these tenants see the flag enabled.
    pub tenant_allowlist: HashSet<String>,
    /// Rollout percentage (0-100). 0 = off, 100 = fully on.
    pub rollout_pct: u8,
}

impl FeatureFlag {
    pub fn new(name: impl Into<String>, description: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            description: description.into(),
            enabled: false,
            tenant_allowlist: HashSet::new(),
            rollout_pct: 0,
        }
    }

    pub fn globally_enabled(mut self) -> Self {
        self.enabled = true;
        self.rollout_pct = 100;
        self
    }

    pub fn with_rollout(mut self, pct: u8) -> Self {
        self.enabled = true;
        self.rollout_pct = pct.min(100);
        self
    }

    pub fn with_tenant(mut self, tenant_id: impl Into<String>) -> Self {
        self.enabled = true;
        self.tenant_allowlist.insert(tenant_id.into());
        self
    }

    /// Check if flag is active for a given tenant.
    pub fn is_active_for(&self, tenant_id: &str) -> bool {
        if !self.enabled {
            return false;
        }
        if !self.tenant_allowlist.is_empty() {
            return self.tenant_allowlist.contains(tenant_id);
        }
        if self.rollout_pct >= 100 {
            return true;
        }
        if self.rollout_pct == 0 {
            return false;
        }
        // Deterministic hash-based rollout
        let hash = simple_hash(tenant_id) % 100;
        hash < self.rollout_pct as u64
    }
}

fn simple_hash(s: &str) -> u64 {
    let mut hash: u64 = 5381;
    for byte in s.bytes() {
        hash = hash.wrapping_mul(33).wrapping_add(byte as u64);
    }
    hash
}

/// In-memory feature flag store.
pub struct FlagStore {
    flags: RwLock<HashMap<String, FeatureFlag>>,
}

impl FlagStore {
    pub fn new() -> Self {
        Self {
            flags: RwLock::new(HashMap::new()),
        }
    }

    pub fn register(&self, flag: FeatureFlag) {
        self.flags.write().insert(flag.name.clone(), flag);
    }

    pub fn is_enabled(&self, name: &str, tenant_id: &str) -> bool {
        self.flags
            .read()
            .get(name)
            .map(|f| f.is_active_for(tenant_id))
            .unwrap_or(false)
    }

    pub fn get(&self, name: &str) -> Option<FeatureFlag> {
        self.flags.read().get(name).cloned()
    }

    pub fn list_all(&self) -> Vec<FeatureFlag> {
        self.flags.read().values().cloned().collect()
    }

    pub fn update_rollout(&self, name: &str, pct: u8) -> bool {
        if let Some(flag) = self.flags.write().get_mut(name) {
            flag.rollout_pct = pct.min(100);
            true
        } else {
            false
        }
    }

    pub fn disable(&self, name: &str) -> bool {
        if let Some(flag) = self.flags.write().get_mut(name) {
            flag.enabled = false;
            true
        } else {
            false
        }
    }
}

impl Default for FlagStore {
    fn default() -> Self {
        Self::new()
    }
}

/// Well-known feature flags.
pub mod known_flags {
    pub const REAL_ASR_ENABLED: &str = "real_asr_enabled";
    pub const REAL_AGENT_ENABLED: &str = "real_agent_enabled";
    pub const REAL_TTS_ENABLED: &str = "real_tts_enabled";
    pub const RECORDING_ENABLED: &str = "recording_enabled";
    pub const BILLING_ENABLED: &str = "billing_enabled";
    pub const HANDOFF_ENABLED: &str = "handoff_enabled";
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn globally_enabled_flag() {
        let flag = FeatureFlag::new("test", "test flag").globally_enabled();
        assert!(flag.is_active_for("any-tenant"));
    }

    #[test]
    fn disabled_flag() {
        let flag = FeatureFlag::new("test", "test flag");
        assert!(!flag.is_active_for("any-tenant"));
    }

    #[test]
    fn tenant_allowlist() {
        let flag = FeatureFlag::new("beta", "beta feature")
            .with_tenant("tenant-a")
            .with_tenant("tenant-b");
        assert!(flag.is_active_for("tenant-a"));
        assert!(flag.is_active_for("tenant-b"));
        assert!(!flag.is_active_for("tenant-c"));
    }

    #[test]
    fn rollout_percentage() {
        let flag = FeatureFlag::new("canary", "canary").with_rollout(50);
        // Deterministic: same tenant always gets same result
        let result1 = flag.is_active_for("tenant-x");
        let result2 = flag.is_active_for("tenant-x");
        assert_eq!(result1, result2);
    }

    #[test]
    fn flag_store_operations() {
        let store = FlagStore::new();
        store.register(FeatureFlag::new("feat1", "feature 1").globally_enabled());
        store.register(FeatureFlag::new("feat2", "feature 2"));

        assert!(store.is_enabled("feat1", "t"));
        assert!(!store.is_enabled("feat2", "t"));
        assert!(!store.is_enabled("nonexistent", "t"));

        store.disable("feat1");
        assert!(!store.is_enabled("feat1", "t"));
    }
}
