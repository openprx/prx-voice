//! In-memory storage implementations for dev/test.

use crate::models::*;
use crate::traits::*;
use chrono::{DateTime, Utc};
use parking_lot::RwLock;
use std::collections::HashMap;
use uuid::Uuid;

/// In-memory session repository.
pub struct MemorySessionRepo {
    data: RwLock<HashMap<Uuid, SessionRecord>>,
}

impl MemorySessionRepo {
    pub fn new() -> Self {
        Self {
            data: RwLock::new(HashMap::new()),
        }
    }
}

impl Default for MemorySessionRepo {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait::async_trait]
impl SessionRepository for MemorySessionRepo {
    async fn create(&self, record: SessionRecord) -> Result<(), StorageError> {
        self.data.write().insert(record.session_id, record);
        Ok(())
    }

    async fn get(&self, session_id: Uuid) -> Result<Option<SessionRecord>, StorageError> {
        Ok(self.data.read().get(&session_id).cloned())
    }

    async fn update_state(
        &self,
        session_id: Uuid,
        state: &str,
        updated_at: DateTime<Utc>,
    ) -> Result<(), StorageError> {
        let mut data = self.data.write();
        let rec = data
            .get_mut(&session_id)
            .ok_or_else(|| StorageError::NotFound(session_id.to_string()))?;
        rec.state = state.to_string();
        rec.updated_at = updated_at;
        Ok(())
    }

    async fn close(
        &self,
        session_id: Uuid,
        reason: &str,
        closed_at: DateTime<Utc>,
    ) -> Result<(), StorageError> {
        let mut data = self.data.write();
        let rec = data
            .get_mut(&session_id)
            .ok_or_else(|| StorageError::NotFound(session_id.to_string()))?;
        rec.state = "Closed".to_string();
        rec.close_reason = Some(reason.to_string());
        rec.closed_at = Some(closed_at);
        rec.updated_at = closed_at;
        Ok(())
    }

    async fn list_by_tenant(
        &self,
        tenant_id: Uuid,
        cursor: PageCursor,
    ) -> Result<PageResult<SessionRecord>, StorageError> {
        let data = self.data.read();
        let all: Vec<_> = data
            .values()
            .filter(|s| s.tenant_id == tenant_id)
            .cloned()
            .collect();
        let total = all.len() as i64;
        let items: Vec<_> = all
            .into_iter()
            .skip(cursor.offset as usize)
            .take(cursor.limit as usize)
            .collect();
        let has_more = cursor.offset + cursor.limit < total;
        Ok(PageResult {
            items,
            total,
            has_more,
        })
    }
}

/// In-memory event repository.
pub struct MemoryEventRepo {
    data: RwLock<Vec<EventRecord>>,
}

impl MemoryEventRepo {
    pub fn new() -> Self {
        Self {
            data: RwLock::new(Vec::new()),
        }
    }
}

impl Default for MemoryEventRepo {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait::async_trait]
impl EventRepository for MemoryEventRepo {
    async fn append(&self, record: EventRecord) -> Result<(), StorageError> {
        self.data.write().push(record);
        Ok(())
    }

    async fn list_by_session(
        &self,
        session_id: Uuid,
        cursor: PageCursor,
    ) -> Result<PageResult<EventRecord>, StorageError> {
        let data = self.data.read();
        let all: Vec<_> = data
            .iter()
            .filter(|e| e.session_id == session_id)
            .cloned()
            .collect();
        let total = all.len() as i64;
        let items: Vec<_> = all
            .into_iter()
            .skip(cursor.offset as usize)
            .take(cursor.limit as usize)
            .collect();
        let has_more = cursor.offset + cursor.limit < total;
        Ok(PageResult {
            items,
            total,
            has_more,
        })
    }

    async fn get_latest_seq(&self, session_id: Uuid) -> Result<i64, StorageError> {
        let data = self.data.read();
        Ok(data
            .iter()
            .filter(|e| e.session_id == session_id)
            .map(|e| e.seq)
            .max()
            .unwrap_or(0))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Utc;

    fn test_session(tenant_id: Uuid) -> SessionRecord {
        SessionRecord {
            session_id: Uuid::new_v4(),
            tenant_id,
            state: "Listening".into(),
            channel: "web_console".into(),
            direction: "inbound".into(),
            language: "en-US".into(),
            total_turns: 0,
            created_at: Utc::now(),
            updated_at: Utc::now(),
            closed_at: None,
            close_reason: None,
            metadata: None,
        }
    }

    #[tokio::test]
    async fn session_crud() {
        let repo = MemorySessionRepo::new();
        let tid = Uuid::new_v4();
        let session = test_session(tid);
        let sid = session.session_id;

        repo.create(session).await.unwrap();
        let got = repo.get(sid).await.unwrap().unwrap();
        assert_eq!(got.state, "Listening");

        repo.update_state(sid, "Speaking", Utc::now())
            .await
            .unwrap();
        let got = repo.get(sid).await.unwrap().unwrap();
        assert_eq!(got.state, "Speaking");

        repo.close(sid, "normal", Utc::now()).await.unwrap();
        let got = repo.get(sid).await.unwrap().unwrap();
        assert_eq!(got.state, "Closed");
        assert_eq!(got.close_reason.as_deref(), Some("normal"));
    }

    #[tokio::test]
    async fn session_list_by_tenant() {
        let repo = MemorySessionRepo::new();
        let t1 = Uuid::new_v4();
        let t2 = Uuid::new_v4();

        repo.create(test_session(t1)).await.unwrap();
        repo.create(test_session(t1)).await.unwrap();
        repo.create(test_session(t2)).await.unwrap();

        let result = repo
            .list_by_tenant(t1, PageCursor::default())
            .await
            .unwrap();
        assert_eq!(result.total, 2);
        assert_eq!(result.items.len(), 2);
    }

    #[tokio::test]
    async fn event_append_and_query() {
        let repo = MemoryEventRepo::new();
        let sid = Uuid::new_v4();

        for i in 1..=5 {
            repo.append(EventRecord {
                event_id: Uuid::new_v4(),
                session_id: sid,
                tenant_id: Uuid::new_v4(),
                turn_id: Some(1),
                seq: i,
                event_type: "prx.voice.test".into(),
                severity: "info".into(),
                payload: serde_json::json!({}),
                created_at: Utc::now(),
            })
            .await
            .unwrap();
        }

        let result = repo
            .list_by_session(
                sid,
                PageCursor {
                    limit: 3,
                    offset: 0,
                },
            )
            .await
            .unwrap();
        assert_eq!(result.items.len(), 3);
        assert_eq!(result.total, 5);
        assert!(result.has_more);

        let latest = repo.get_latest_seq(sid).await.unwrap();
        assert_eq!(latest, 5);
    }
}
