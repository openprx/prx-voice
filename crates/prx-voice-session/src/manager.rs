//! Multi-session manager with per-tenant limits.

use crate::config::SessionConfig;
use crate::orchestrator::{OrchestratorError, SessionOrchestrator};
use parking_lot::RwLock;
use prx_voice_adapter::agent::AgentAdapter;
use prx_voice_adapter::asr::AsrAdapter;
use prx_voice_adapter::tts::TtsAdapter;
use prx_voice_event::bus::EventBus;
use prx_voice_observe::metrics::MetricsRegistry;
use prx_voice_types::ids::{SessionId, TenantId};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::Mutex;
use tracing::info;

/// Per-tenant session limits.
#[derive(Debug, Clone)]
pub struct TenantLimits {
    pub max_concurrent_sessions: usize,
}

impl Default for TenantLimits {
    fn default() -> Self {
        Self {
            max_concurrent_sessions: 100,
        }
    }
}

/// Session manager error.
#[derive(Debug, thiserror::Error)]
pub enum SessionManagerError {
    #[error("Concurrent session limit exceeded for tenant {tenant_id}: {current}/{limit}")]
    ConcurrentLimitExceeded {
        tenant_id: String,
        current: usize,
        limit: usize,
    },
    #[error("Session not found: {0}")]
    SessionNotFound(String),
    #[error("Orchestrator error: {0}")]
    Orchestrator(#[from] OrchestratorError),
}

/// Entry for a managed session.
pub struct ManagedSession {
    pub orchestrator: Arc<Mutex<SessionOrchestrator>>,
    pub tenant_id: TenantId,
}

/// Manages multiple concurrent sessions with tenant-level limits.
pub struct SessionManager {
    sessions: RwLock<HashMap<SessionId, ManagedSession>>,
    tenant_limits: RwLock<HashMap<TenantId, TenantLimits>>,
    default_limits: TenantLimits,
    event_bus: EventBus,
    metrics: Arc<MetricsRegistry>,
}

impl SessionManager {
    pub fn new(
        event_bus: EventBus,
        default_limits: TenantLimits,
        metrics: Arc<MetricsRegistry>,
    ) -> Self {
        Self {
            sessions: RwLock::new(HashMap::new()),
            tenant_limits: RwLock::new(HashMap::new()),
            default_limits,
            event_bus,
            metrics,
        }
    }

    /// Set limits for a specific tenant.
    pub fn set_tenant_limits(&self, tenant_id: TenantId, limits: TenantLimits) {
        self.tenant_limits.write().insert(tenant_id, limits);
    }

    /// Get active session count for a tenant.
    pub fn tenant_session_count(&self, tenant_id: TenantId) -> usize {
        self.sessions
            .read()
            .values()
            .filter(|s| s.tenant_id == tenant_id)
            .count()
    }

    /// Total active sessions.
    pub fn total_session_count(&self) -> usize {
        self.sessions.read().len()
    }

    /// Create and start a new session.
    pub async fn create_session(
        &self,
        tenant_id: TenantId,
        config: SessionConfig,
        asr: Box<dyn AsrAdapter>,
        agent: Box<dyn AgentAdapter>,
        tts: Box<dyn TtsAdapter>,
    ) -> Result<SessionId, SessionManagerError> {
        // Check tenant limits
        let limits = self
            .tenant_limits
            .read()
            .get(&tenant_id)
            .cloned()
            .unwrap_or_else(|| self.default_limits.clone());

        let current_count = self.tenant_session_count(tenant_id);
        if current_count >= limits.max_concurrent_sessions {
            return Err(SessionManagerError::ConcurrentLimitExceeded {
                tenant_id: tenant_id.to_string(),
                current: current_count,
                limit: limits.max_concurrent_sessions,
            });
        }

        // Create and start orchestrator
        let mut orch = SessionOrchestrator::new(
            tenant_id,
            config,
            asr,
            agent,
            tts,
            self.event_bus.clone(),
            self.metrics.clone(),
        );
        let session_id = orch.session_id();

        orch.start().await?;

        info!(%session_id, %tenant_id, "session created via manager");

        self.sessions.write().insert(
            session_id,
            ManagedSession {
                orchestrator: Arc::new(Mutex::new(orch)),
                tenant_id,
            },
        );

        Ok(session_id)
    }

    /// Get a session's orchestrator.
    pub fn get_session(&self, session_id: SessionId) -> Option<Arc<Mutex<SessionOrchestrator>>> {
        self.sessions
            .read()
            .get(&session_id)
            .map(|s| s.orchestrator.clone())
    }

    /// Remove a session (after close/fail).
    pub fn remove_session(&self, session_id: SessionId) -> bool {
        self.sessions.write().remove(&session_id).is_some()
    }

    /// List all active session IDs for a tenant.
    pub fn list_sessions(&self, tenant_id: Option<TenantId>) -> Vec<SessionId> {
        self.sessions
            .read()
            .iter()
            .filter(|(_, s)| tenant_id.is_none() || Some(s.tenant_id) == tenant_id)
            .map(|(id, _)| *id)
            .collect()
    }

    /// Get event bus reference.
    pub fn event_bus(&self) -> &EventBus {
        &self.event_bus
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use prx_voice_adapter::mock_agent::{MockAgent, MockAgentConfig};
    use prx_voice_adapter::mock_asr::{MockAsr, MockAsrConfig};
    use prx_voice_adapter::mock_tts::{MockTts, MockTtsConfig};
    use prx_voice_event::bus::EventBusConfig;
    use prx_voice_observe::metrics::MetricsRegistry;

    fn mock_adapters() -> (
        Box<dyn AsrAdapter>,
        Box<dyn AgentAdapter>,
        Box<dyn TtsAdapter>,
    ) {
        (
            Box::new(MockAsr::new(MockAsrConfig {
                latency_ms: 5,
                ..Default::default()
            })),
            Box::new(MockAgent::new(MockAgentConfig {
                first_token_latency_ms: 5,
                ..Default::default()
            })),
            Box::new(MockTts::new(MockTtsConfig {
                first_chunk_latency_ms: 5,
                ..Default::default()
            })),
        )
    }

    #[tokio::test]
    async fn create_and_retrieve_session() {
        let bus = EventBus::new(EventBusConfig::default());
        let mgr = SessionManager::new(
            bus,
            TenantLimits::default(),
            Arc::new(MetricsRegistry::new()),
        );
        let tenant = TenantId::new();
        let (asr, agent, tts) = mock_adapters();

        let sid = mgr
            .create_session(tenant, SessionConfig::default(), asr, agent, tts)
            .await
            .unwrap();
        assert!(mgr.get_session(sid).is_some());
        assert_eq!(mgr.total_session_count(), 1);
        assert_eq!(mgr.tenant_session_count(tenant), 1);
    }

    #[tokio::test]
    async fn concurrent_limit_enforced() {
        let bus = EventBus::new(EventBusConfig::default());
        let mgr = SessionManager::new(
            bus,
            TenantLimits {
                max_concurrent_sessions: 2,
            },
            Arc::new(MetricsRegistry::new()),
        );
        let tenant = TenantId::new();

        // Create 2 sessions (at limit)
        for _ in 0..2 {
            let (asr, agent, tts) = mock_adapters();
            mgr.create_session(tenant, SessionConfig::default(), asr, agent, tts)
                .await
                .unwrap();
        }
        assert_eq!(mgr.tenant_session_count(tenant), 2);

        // Third should fail
        let (asr, agent, tts) = mock_adapters();
        let result = mgr
            .create_session(tenant, SessionConfig::default(), asr, agent, tts)
            .await;
        assert!(matches!(
            result,
            Err(SessionManagerError::ConcurrentLimitExceeded { .. })
        ));
    }

    #[tokio::test]
    async fn different_tenants_independent_limits() {
        let bus = EventBus::new(EventBusConfig::default());
        let mgr = SessionManager::new(
            bus,
            TenantLimits {
                max_concurrent_sessions: 1,
            },
            Arc::new(MetricsRegistry::new()),
        );
        let t1 = TenantId::new();
        let t2 = TenantId::new();

        let (asr, agent, tts) = mock_adapters();
        mgr.create_session(t1, SessionConfig::default(), asr, agent, tts)
            .await
            .unwrap();

        // Different tenant should still work
        let (asr, agent, tts) = mock_adapters();
        mgr.create_session(t2, SessionConfig::default(), asr, agent, tts)
            .await
            .unwrap();

        assert_eq!(mgr.total_session_count(), 2);
        assert_eq!(mgr.tenant_session_count(t1), 1);
        assert_eq!(mgr.tenant_session_count(t2), 1);
    }

    #[tokio::test]
    async fn remove_session_frees_slot() {
        let bus = EventBus::new(EventBusConfig::default());
        let mgr = SessionManager::new(
            bus,
            TenantLimits {
                max_concurrent_sessions: 1,
            },
            Arc::new(MetricsRegistry::new()),
        );
        let tenant = TenantId::new();

        let (asr, agent, tts) = mock_adapters();
        let sid = mgr
            .create_session(tenant, SessionConfig::default(), asr, agent, tts)
            .await
            .unwrap();

        mgr.remove_session(sid);
        assert_eq!(mgr.tenant_session_count(tenant), 0);

        // Can create again
        let (asr, agent, tts) = mock_adapters();
        mgr.create_session(tenant, SessionConfig::default(), asr, agent, tts)
            .await
            .unwrap();
        assert_eq!(mgr.tenant_session_count(tenant), 1);
    }
}
