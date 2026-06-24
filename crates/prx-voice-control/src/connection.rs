//! Persistent WebSocket connection management.
//!
//! [`ConnectionManager`] tracks all long-lived WebSocket connections and their
//! associated sessions, enabling dead-connection detection and resource cleanup.

use std::collections::{HashMap, HashSet};
use std::time::{Duration, Instant};

use parking_lot::RwLock;
use prx_voice_types::ids::{ConnId, SessionId, TenantId};

/// Metadata for a persistent WebSocket connection.
///
/// The actual WebSocket sink lives in the handler task — the manager only
/// tracks bookkeeping data (IDs, timestamps, session bindings) so it can
/// be freely shared across threads without dealing with async sink types.
pub struct ConnectionMeta {
    /// Unique connection identifier.
    pub conn_id: ConnId,
    /// Tenant that owns this connection.
    pub tenant_id: TenantId,
    /// Client-supplied deduplication key (from `x-client-id` header).
    pub client_id: String,
    /// Sessions currently active on this connection.
    pub active_sessions: HashSet<SessionId>,
    /// Last time the client proved it is alive (ping, pong, or any message).
    pub last_client_activity: Instant,
    /// When this connection was established.
    pub created_at: Instant,
    /// Cancellation token — when dropped, signals the handler to shut down.
    pub cancel: tokio_util::sync::CancellationToken,
}

/// Manages all persistent WebSocket connections.
pub struct ConnectionManager {
    /// Active connections indexed by ConnId.
    connections: RwLock<HashMap<ConnId, ConnectionMeta>>,
    /// Reverse index: session → connection that owns it.
    session_to_conn: RwLock<HashMap<SessionId, ConnId>>,
    /// Client dedup index: client_id → conn_id.
    client_to_conn: RwLock<HashMap<String, ConnId>>,
}

impl ConnectionManager {
    /// Create an empty connection manager.
    pub fn new() -> Self {
        Self {
            connections: RwLock::new(HashMap::new()),
            session_to_conn: RwLock::new(HashMap::new()),
            client_to_conn: RwLock::new(HashMap::new()),
        }
    }

    /// Register a new connection.
    ///
    /// If a connection with the same `client_id` already exists, the old
    /// connection is evicted and its cancellation token is triggered.
    /// Returns the evicted ConnId if any.
    pub fn register(&self, meta: ConnectionMeta) -> Option<ConnId> {
        let client_id = meta.client_id.clone();
        let conn_id = meta.conn_id;

        // Check for existing connection with same client_id.
        let evicted_id = {
            let client_map = self.client_to_conn.read();
            client_map.get(&client_id).copied()
        };

        // Evict old connection if present.
        if let Some(old_conn_id) = evicted_id {
            // Cancel the old handler so it shuts down gracefully.
            if let Some(old_meta) = self.connections.read().get(&old_conn_id) {
                old_meta.cancel.cancel();
            }
            self.remove_connection_inner(&old_conn_id);
        }

        // Insert new connection.
        self.connections.write().insert(conn_id, meta);
        self.client_to_conn.write().insert(client_id, conn_id);

        evicted_id
    }

    /// Remove a connection and all its index entries.
    /// Returns the session IDs that were bound to this connection.
    pub fn remove_connection(&self, conn_id: &ConnId) -> Vec<SessionId> {
        // Cancel the handler.
        if let Some(meta) = self.connections.read().get(conn_id) {
            meta.cancel.cancel();
        }
        self.remove_connection_inner(conn_id)
    }

    /// Internal removal without cancelling (already cancelled by caller).
    fn remove_connection_inner(&self, conn_id: &ConnId) -> Vec<SessionId> {
        let sessions = if let Some(meta) = self.connections.write().remove(conn_id) {
            self.client_to_conn.write().remove(&meta.client_id);
            meta.active_sessions.into_iter().collect::<Vec<_>>()
        } else {
            Vec::new()
        };

        let mut session_map = self.session_to_conn.write();
        for sid in &sessions {
            session_map.remove(sid);
        }

        sessions
    }

    /// Associate a session with a connection.
    pub fn bind_session(&self, conn_id: &ConnId, session_id: SessionId) {
        if let Some(meta) = self.connections.write().get_mut(conn_id) {
            meta.active_sessions.insert(session_id);
        }
        self.session_to_conn.write().insert(session_id, *conn_id);
    }

    /// Dissociate a session from its connection.
    pub fn unbind_session(&self, session_id: &SessionId) {
        if let Some(conn_id) = self.session_to_conn.write().remove(session_id) {
            if let Some(meta) = self.connections.write().get_mut(&conn_id) {
                meta.active_sessions.remove(session_id);
            }
        }
    }

    /// Record that a client proved it is alive (ping, pong, or data message).
    pub fn record_client_activity(&self, conn_id: &ConnId) {
        if let Some(meta) = self.connections.write().get_mut(conn_id) {
            meta.last_client_activity = Instant::now();
        }
    }

    /// Find connections whose last client activity is older than `threshold`.
    pub fn find_dead_connections(&self, threshold: Duration) -> Vec<ConnId> {
        let now = Instant::now();
        self.connections
            .read()
            .iter()
            .filter(|(_, meta)| now.duration_since(meta.last_client_activity) > threshold)
            .map(|(id, _)| *id)
            .collect()
    }

    /// Check whether a session belongs to a given connection.
    pub fn session_belongs_to(&self, session_id: &SessionId, conn_id: &ConnId) -> bool {
        self.session_to_conn
            .read()
            .get(session_id)
            .is_some_and(|cid| cid == conn_id)
    }

    /// Get the set of active session IDs for a connection.
    pub fn active_sessions(&self, conn_id: &ConnId) -> HashSet<SessionId> {
        self.connections
            .read()
            .get(conn_id)
            .map(|m| m.active_sessions.clone())
            .unwrap_or_default()
    }

    /// Total number of active connections.
    pub fn connection_count(&self) -> usize {
        self.connections.read().len()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn dummy_meta(client_id: &str) -> ConnectionMeta {
        ConnectionMeta {
            conn_id: ConnId::new(),
            tenant_id: TenantId::new(),
            client_id: client_id.to_string(),
            active_sessions: HashSet::new(),
            last_client_activity: Instant::now(),
            created_at: Instant::now(),
            cancel: tokio_util::sync::CancellationToken::new(),
        }
    }

    #[test]
    fn register_and_remove() {
        let mgr = ConnectionManager::new();
        let meta = dummy_meta("client-1");
        let conn_id = meta.conn_id;

        assert!(mgr.register(meta).is_none());
        assert_eq!(mgr.connection_count(), 1);

        let removed = mgr.remove_connection(&conn_id);
        assert!(removed.is_empty());
        assert_eq!(mgr.connection_count(), 0);
    }

    #[test]
    fn client_id_dedup_evicts_old_connection() {
        let mgr = ConnectionManager::new();

        let m1 = dummy_meta("same-client");
        let id1 = m1.conn_id;
        let cancel1 = m1.cancel.clone();
        mgr.register(m1);

        let m2 = dummy_meta("same-client");
        let evicted = mgr.register(m2);

        assert_eq!(evicted, Some(id1));
        assert_eq!(mgr.connection_count(), 1);
        // Old connection's cancel token should be triggered.
        assert!(cancel1.is_cancelled());
    }

    #[test]
    fn bind_and_unbind_session() {
        let mgr = ConnectionManager::new();
        let meta = dummy_meta("client-1");
        let conn_id = meta.conn_id;
        mgr.register(meta);

        let sid = SessionId::new();
        mgr.bind_session(&conn_id, sid);

        assert!(mgr.session_belongs_to(&sid, &conn_id));
        assert_eq!(mgr.active_sessions(&conn_id).len(), 1);

        mgr.unbind_session(&sid);
        assert!(!mgr.session_belongs_to(&sid, &conn_id));
        assert!(mgr.active_sessions(&conn_id).is_empty());
    }

    #[test]
    fn remove_connection_cleans_up_sessions() {
        let mgr = ConnectionManager::new();
        let meta = dummy_meta("client-1");
        let conn_id = meta.conn_id;
        mgr.register(meta);

        let sid1 = SessionId::new();
        let sid2 = SessionId::new();
        mgr.bind_session(&conn_id, sid1);
        mgr.bind_session(&conn_id, sid2);

        let removed = mgr.remove_connection(&conn_id);
        assert_eq!(removed.len(), 2);
        assert!(!mgr.session_belongs_to(&sid1, &conn_id));
    }

    #[test]
    fn find_dead_connections_threshold() {
        let mgr = ConnectionManager::new();
        let mut meta = dummy_meta("client-1");
        meta.last_client_activity = Instant::now() - Duration::from_secs(60);
        let conn_id = meta.conn_id;
        mgr.register(meta);

        let dead = mgr.find_dead_connections(Duration::from_secs(30));
        assert_eq!(dead.len(), 1);
        assert_eq!(dead[0], conn_id);

        let dead = mgr.find_dead_connections(Duration::from_secs(120));
        assert!(dead.is_empty());
    }

    #[test]
    fn record_client_activity_refreshes_timestamp() {
        let mgr = ConnectionManager::new();
        let mut meta = dummy_meta("client-1");
        meta.last_client_activity = Instant::now() - Duration::from_secs(60);
        let conn_id = meta.conn_id;
        mgr.register(meta);

        assert_eq!(
            mgr.find_dead_connections(Duration::from_secs(30)).len(),
            1
        );

        mgr.record_client_activity(&conn_id);

        assert!(mgr.find_dead_connections(Duration::from_secs(30)).is_empty());
    }
}
