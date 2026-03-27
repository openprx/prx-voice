//! Shared application state for the control plane.

use parking_lot::RwLock;
use prx_voice_audit::store::AuditStore;
use prx_voice_billing::ledger::BillingLedger;
use prx_voice_event::bus::EventBus;
use prx_voice_observe::metrics::MetricsRegistry;
use prx_voice_session::orchestrator::SessionOrchestrator;
use prx_voice_types::ids::SessionId;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::Mutex;

/// Shared application state.
#[derive(Clone)]
pub struct AppState {
    pub sessions: Arc<RwLock<HashMap<SessionId, Arc<Mutex<SessionOrchestrator>>>>>,
    pub event_bus: EventBus,
    pub metrics: Arc<MetricsRegistry>,
    pub audit: Arc<AuditStore>,
    pub billing: Arc<BillingLedger>,
    /// Idempotency key store: maps idempotency key to the session ID that was created.
    pub idempotency: Arc<RwLock<HashMap<String, String>>>,
}

impl AppState {
    pub fn new(event_bus: EventBus, metrics: Arc<MetricsRegistry>) -> Self {
        Self {
            sessions: Arc::new(RwLock::new(HashMap::new())),
            event_bus,
            metrics,
            audit: Arc::new(AuditStore::new()),
            billing: Arc::new(BillingLedger::new()),
            idempotency: Arc::new(RwLock::new(HashMap::new())),
        }
    }
}
