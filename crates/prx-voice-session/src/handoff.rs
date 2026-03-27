//! Human handoff management.
//!
//! When the orchestrator or agent decides a session needs a human,
//! a handoff request is created and tracked until confirmed or timed out.

use chrono::{DateTime, Utc};
use parking_lot::RwLock;
use prx_voice_types::ids::{SessionId, TenantId};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use uuid::Uuid;

/// Unique handoff request ID.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct HandoffId(Uuid);

impl HandoffId {
    pub fn new() -> Self {
        Self(Uuid::new_v4())
    }
}

impl Default for HandoffId {
    fn default() -> Self {
        Self::new()
    }
}

impl std::fmt::Display for HandoffId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "handoff-{}", self.0)
    }
}

/// Handoff target type.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum HandoffTarget {
    HumanAgent,
    ExternalSystem,
    SpecificQueue { queue_id: String },
}

/// Current status of a handoff request.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum HandoffStatus {
    Pending,
    Queued,
    Assigned,
    Confirmed,
    Rejected,
    TimedOut,
    Cancelled,
}

/// A handoff request.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HandoffRequest {
    pub handoff_id: HandoffId,
    pub session_id: SessionId,
    pub tenant_id: TenantId,
    pub target: HandoffTarget,
    pub reason: String,
    pub context_summary: Option<String>,
    pub status: HandoffStatus,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub queue_position: Option<u32>,
    pub estimated_wait_sec: Option<u32>,
    pub assigned_agent_id: Option<String>,
}

/// Handoff management errors.
#[derive(Debug, thiserror::Error)]
pub enum HandoffError {
    #[error("Handoff not found: {0}")]
    NotFound(String),
    #[error("Handoff already in terminal state: {0:?}")]
    AlreadyTerminal(HandoffStatus),
    #[error("Invalid status transition from {from:?} to {to:?}")]
    InvalidTransition {
        from: HandoffStatus,
        to: HandoffStatus,
    },
}

/// In-memory handoff manager.
pub struct HandoffManager {
    requests: RwLock<HashMap<HandoffId, HandoffRequest>>,
    session_index: RwLock<HashMap<SessionId, HandoffId>>,
}

impl HandoffManager {
    pub fn new() -> Self {
        Self {
            requests: RwLock::new(HashMap::new()),
            session_index: RwLock::new(HashMap::new()),
        }
    }

    /// Create a new handoff request.
    pub fn create_request(
        &self,
        session_id: SessionId,
        tenant_id: TenantId,
        target: HandoffTarget,
        reason: impl Into<String>,
        context_summary: Option<String>,
    ) -> HandoffRequest {
        let now = Utc::now();
        let req = HandoffRequest {
            handoff_id: HandoffId::new(),
            session_id,
            tenant_id,
            target,
            reason: reason.into(),
            context_summary,
            status: HandoffStatus::Pending,
            created_at: now,
            updated_at: now,
            queue_position: None,
            estimated_wait_sec: None,
            assigned_agent_id: None,
        };

        self.requests.write().insert(req.handoff_id, req.clone());
        self.session_index
            .write()
            .insert(session_id, req.handoff_id);
        req
    }

    /// Get handoff request by ID.
    pub fn get(&self, handoff_id: HandoffId) -> Option<HandoffRequest> {
        self.requests.read().get(&handoff_id).cloned()
    }

    /// Get handoff request by session ID.
    pub fn get_by_session(&self, session_id: SessionId) -> Option<HandoffRequest> {
        let handoff_id = self.session_index.read().get(&session_id).copied()?;
        self.get(handoff_id)
    }

    /// Update handoff status with validation.
    pub fn update_status(
        &self,
        handoff_id: HandoffId,
        new_status: HandoffStatus,
    ) -> Result<HandoffRequest, HandoffError> {
        let mut requests = self.requests.write();
        let req = requests
            .get_mut(&handoff_id)
            .ok_or_else(|| HandoffError::NotFound(handoff_id.to_string()))?;

        // Check terminal states
        if matches!(
            req.status,
            HandoffStatus::Confirmed
                | HandoffStatus::Rejected
                | HandoffStatus::TimedOut
                | HandoffStatus::Cancelled
        ) {
            return Err(HandoffError::AlreadyTerminal(req.status));
        }

        // Validate transitions
        let valid = matches!(
            (req.status, new_status),
            (HandoffStatus::Pending, HandoffStatus::Queued)
                | (HandoffStatus::Pending, HandoffStatus::Assigned)
                | (HandoffStatus::Pending, HandoffStatus::Cancelled)
                | (HandoffStatus::Pending, HandoffStatus::TimedOut)
                | (HandoffStatus::Queued, HandoffStatus::Assigned)
                | (HandoffStatus::Queued, HandoffStatus::Cancelled)
                | (HandoffStatus::Queued, HandoffStatus::TimedOut)
                | (HandoffStatus::Assigned, HandoffStatus::Confirmed)
                | (HandoffStatus::Assigned, HandoffStatus::Rejected)
                | (HandoffStatus::Assigned, HandoffStatus::TimedOut)
        );

        if !valid {
            return Err(HandoffError::InvalidTransition {
                from: req.status,
                to: new_status,
            });
        }

        req.status = new_status;
        req.updated_at = Utc::now();
        Ok(req.clone())
    }

    /// Assign a human agent to a handoff.
    pub fn assign_agent(
        &self,
        handoff_id: HandoffId,
        agent_id: impl Into<String>,
    ) -> Result<HandoffRequest, HandoffError> {
        {
            let mut requests = self.requests.write();
            let req = requests
                .get_mut(&handoff_id)
                .ok_or_else(|| HandoffError::NotFound(handoff_id.to_string()))?;
            req.assigned_agent_id = Some(agent_id.into());
        }
        self.update_status(handoff_id, HandoffStatus::Assigned)
    }

    /// Set queue position for a pending handoff.
    pub fn set_queue_position(
        &self,
        handoff_id: HandoffId,
        position: u32,
        estimated_wait_sec: u32,
    ) -> Result<(), HandoffError> {
        let mut requests = self.requests.write();
        let req = requests
            .get_mut(&handoff_id)
            .ok_or_else(|| HandoffError::NotFound(handoff_id.to_string()))?;
        req.queue_position = Some(position);
        req.estimated_wait_sec = Some(estimated_wait_sec);
        Ok(())
    }

    /// Count pending handoffs for a tenant.
    pub fn pending_count(&self, tenant_id: TenantId) -> usize {
        self.requests
            .read()
            .values()
            .filter(|r| {
                r.tenant_id == tenant_id
                    && matches!(r.status, HandoffStatus::Pending | HandoffStatus::Queued)
            })
            .count()
    }
}

impl Default for HandoffManager {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn create_and_retrieve_handoff() {
        let mgr = HandoffManager::new();
        let sid = SessionId::new();
        let tid = TenantId::new();

        let req = mgr.create_request(sid, tid, HandoffTarget::HumanAgent, "user requested", None);
        assert_eq!(req.status, HandoffStatus::Pending);

        let retrieved = mgr.get(req.handoff_id).expect("should find by handoff_id");
        assert_eq!(retrieved.session_id, sid);

        let by_session = mgr.get_by_session(sid).expect("should find by session_id");
        assert_eq!(by_session.handoff_id, req.handoff_id);
    }

    #[test]
    fn handoff_lifecycle_pending_to_confirmed() {
        let mgr = HandoffManager::new();
        let sid = SessionId::new();
        let tid = TenantId::new();

        let req = mgr.create_request(sid, tid, HandoffTarget::HumanAgent, "test", None);

        // Pending -> Queued
        mgr.update_status(req.handoff_id, HandoffStatus::Queued)
            .expect("pending to queued");
        mgr.set_queue_position(req.handoff_id, 3, 120)
            .expect("set queue position");

        // Queued -> Assigned
        mgr.assign_agent(req.handoff_id, "agent-42")
            .expect("assign agent");
        let updated = mgr.get(req.handoff_id).expect("should exist");
        assert_eq!(updated.status, HandoffStatus::Assigned);
        assert_eq!(updated.assigned_agent_id.as_deref(), Some("agent-42"));

        // Assigned -> Confirmed
        mgr.update_status(req.handoff_id, HandoffStatus::Confirmed)
            .expect("assigned to confirmed");
        let final_state = mgr.get(req.handoff_id).expect("should exist");
        assert_eq!(final_state.status, HandoffStatus::Confirmed);
    }

    #[test]
    fn terminal_state_rejects_updates() {
        let mgr = HandoffManager::new();
        let req = mgr.create_request(
            SessionId::new(),
            TenantId::new(),
            HandoffTarget::HumanAgent,
            "test",
            None,
        );
        mgr.update_status(req.handoff_id, HandoffStatus::Cancelled)
            .expect("pending to cancelled");

        let result = mgr.update_status(req.handoff_id, HandoffStatus::Queued);
        assert!(matches!(result, Err(HandoffError::AlreadyTerminal(_))));
    }

    #[test]
    fn invalid_transition_rejected() {
        let mgr = HandoffManager::new();
        let req = mgr.create_request(
            SessionId::new(),
            TenantId::new(),
            HandoffTarget::HumanAgent,
            "test",
            None,
        );

        // Pending -> Confirmed is not valid (must go through Assigned)
        let result = mgr.update_status(req.handoff_id, HandoffStatus::Confirmed);
        assert!(matches!(
            result,
            Err(HandoffError::InvalidTransition { .. })
        ));
    }

    #[test]
    fn pending_count_per_tenant() {
        let mgr = HandoffManager::new();
        let t1 = TenantId::new();
        let t2 = TenantId::new();

        mgr.create_request(SessionId::new(), t1, HandoffTarget::HumanAgent, "1", None);
        mgr.create_request(SessionId::new(), t1, HandoffTarget::HumanAgent, "2", None);
        mgr.create_request(SessionId::new(), t2, HandoffTarget::HumanAgent, "3", None);

        assert_eq!(mgr.pending_count(t1), 2);
        assert_eq!(mgr.pending_count(t2), 1);
    }
}
