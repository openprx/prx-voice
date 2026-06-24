//! Background health-check loop for dead connections and zombie sessions.
//!
//! Spawned once at server startup. Periodically scans for:
//! - **Dead connections**: no client activity for > 30 seconds.
//! - **Zombie sessions**: stuck in a non-terminal state beyond its timeout.

use std::sync::Arc;
use std::time::Duration;

use prx_voice_state::transition::SessionState;
use tokio::time::interval;
use tracing::{info, warn};

use crate::connection::ConnectionManager;
use crate::state::AppState;

/// Default interval between health checks.
const CHECK_INTERVAL: Duration = Duration::from_secs(5);

/// A connection with no client activity for this long is considered dead.
const DEAD_CONNECTION_THRESHOLD: Duration = Duration::from_secs(30);

/// Run the health-check loop until the server shuts down.
///
/// This task should be spawned via `tokio::spawn` at startup.
pub async fn run_health_check_loop(conn_mgr: Arc<ConnectionManager>, state: AppState) {
    let mut ticker = interval(CHECK_INTERVAL);

    loop {
        ticker.tick().await;

        cleanup_dead_connections(&conn_mgr, &state).await;
        cleanup_zombie_sessions(&conn_mgr, &state).await;
    }
}

/// Find and remove connections that have gone silent.
async fn cleanup_dead_connections(conn_mgr: &ConnectionManager, state: &AppState) {
    let dead_ids = conn_mgr.find_dead_connections(DEAD_CONNECTION_THRESHOLD);

    for conn_id in dead_ids {
        warn!(%conn_id, "dead connection detected — no client activity for 30s");

        // remove_connection triggers the handler's cancellation token and
        // returns all session IDs bound to this connection.
        let orphaned_sessions = conn_mgr.remove_connection(&conn_id);

        // Close and remove each orphaned session.
        for sid in orphaned_sessions {
            close_and_remove_session(state, &sid, "connection_dead").await;
        }
    }
}

/// Find sessions stuck in a non-terminal state past their timeout.
async fn cleanup_zombie_sessions(conn_mgr: &ConnectionManager, state: &AppState) {
    // Collect candidates while holding only a read lock.
    let candidates: Vec<_> = {
        let sessions = state.sessions.read();
        sessions.keys().copied().collect()
    };

    for sid in candidates {
        let orch_arc = {
            let sessions = state.sessions.read();
            match sessions.get(&sid) {
                Some(arc) => Arc::clone(arc),
                None => continue,
            }
        };

        let should_remove = {
            let orch = orch_arc.lock().await;
            let current = orch.state();

            // Already terminal — just needs cleanup from the map.
            if matches!(current, SessionState::Closed | SessionState::Failed) {
                true
            } else if let Some(timeout) = orch.timeout_for_current_state() {
                if orch.state_age() > timeout {
                    warn!(
                        %sid,
                        state = %current,
                        age_secs = orch.state_age().as_secs(),
                        timeout_secs = timeout.as_secs(),
                        "zombie session — state timeout exceeded"
                    );
                    true
                } else {
                    false
                }
            } else {
                false
            }
        };

        if should_remove {
            close_and_remove_session(state, &sid, "timeout_gc").await;
            conn_mgr.unbind_session(&sid);
        }
    }
}

/// Close a session (if not already terminal) and remove it from AppState.
async fn close_and_remove_session(
    state: &AppState,
    sid: &prx_voice_types::ids::SessionId,
    reason: &str,
) {
    // Close the orchestrator if it's still active.
    let orch_arc = { state.sessions.read().get(sid).cloned() };
    if let Some(orch_arc) = orch_arc {
        let mut orch = orch_arc.lock().await;
        let current = orch.state();
        if !matches!(current, SessionState::Closed | SessionState::Failed) {
            if let Err(e) = orch.close(reason).await {
                warn!(%sid, error = %e, "failed to close zombie session");
            }
        }
    }

    // Remove from session map.
    state.sessions.write().remove(sid);
    info!(%sid, reason, "session removed by health check");
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn constants_are_reasonable() {
        assert!(CHECK_INTERVAL.as_secs() >= 1);
        assert!(CHECK_INTERVAL.as_secs() <= 30);
        assert!(DEAD_CONNECTION_THRESHOLD.as_secs() >= 10);
        assert!(DEAD_CONNECTION_THRESHOLD.as_secs() <= 120);
    }
}
