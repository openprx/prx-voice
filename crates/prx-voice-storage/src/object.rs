//! Object storage abstraction for recordings, exports, artifacts.
//! Per the data model spec: Plane 3 (Object Plane).
//! Path format: /{region}/{tenant_id}/{workspace_id}/{project_id}/{artifact_type}/{yyyy}/{mm}/{dd}/{session_id}/{artifact_id}

use chrono::Utc;
use parking_lot::RwLock;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use uuid::Uuid;

/// Object storage errors.
#[derive(Debug, thiserror::Error)]
pub enum ObjectError {
    #[error("Object not found: {0}")]
    NotFound(String),
    #[error("Storage error: {0}")]
    StorageError(String),
    #[error("Object too large: {size} bytes exceeds {limit} bytes")]
    TooLarge { size: u64, limit: u64 },
}

/// Object metadata.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ObjectMeta {
    pub key: String,
    pub tenant_id: Uuid,
    pub artifact_type: ArtifactType,
    pub content_type: String,
    pub size_bytes: u64,
    pub checksum: Option<String>,
    pub retention_class: String,
    pub created_at: String,
}

/// Types of artifacts stored.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ArtifactType {
    Recording,
    TranscriptExport,
    DiagnosticBundle,
    ReplayPackage,
    AuditExport,
}

/// Object storage trait.
#[async_trait::async_trait]
pub trait ObjectStore: Send + Sync {
    /// Store an object.
    async fn put(&self, key: &str, data: Vec<u8>, meta: ObjectMeta) -> Result<(), ObjectError>;
    /// Retrieve an object.
    async fn get(&self, key: &str) -> Result<(Vec<u8>, ObjectMeta), ObjectError>;
    /// Delete an object.
    async fn delete(&self, key: &str) -> Result<(), ObjectError>;
    /// Check if an object exists.
    async fn exists(&self, key: &str) -> Result<bool, ObjectError>;
    /// List objects by prefix.
    async fn list(&self, prefix: &str, limit: usize) -> Result<Vec<ObjectMeta>, ObjectError>;
}

/// Generate a deterministic object key per the spec.
pub fn object_key(
    region: &str,
    tenant_id: Uuid,
    artifact_type: &ArtifactType,
    session_id: Uuid,
    artifact_id: Uuid,
) -> String {
    let now = Utc::now();
    let type_str = serde_json::to_value(artifact_type)
        .ok()
        .and_then(|v| v.as_str().map(String::from))
        .unwrap_or_else(|| "unknown".into());
    format!(
        "{region}/{tenant_id}/{type_str}/{}/{:02}/{:02}/{session_id}/{artifact_id}",
        now.format("%Y"),
        now.format("%m"),
        now.format("%d"),
    )
}

/// In-memory object store for dev/test.
pub struct MemoryObjectStore {
    objects: RwLock<HashMap<String, (Vec<u8>, ObjectMeta)>>,
    max_size: u64,
}

impl MemoryObjectStore {
    pub fn new(max_size_bytes: u64) -> Self {
        Self {
            objects: RwLock::new(HashMap::new()),
            max_size: max_size_bytes,
        }
    }
}

impl Default for MemoryObjectStore {
    fn default() -> Self {
        Self::new(100 * 1024 * 1024) // 100MB default
    }
}

#[async_trait::async_trait]
impl ObjectStore for MemoryObjectStore {
    async fn put(&self, key: &str, data: Vec<u8>, meta: ObjectMeta) -> Result<(), ObjectError> {
        if data.len() as u64 > self.max_size {
            return Err(ObjectError::TooLarge {
                size: data.len() as u64,
                limit: self.max_size,
            });
        }
        self.objects.write().insert(key.to_string(), (data, meta));
        Ok(())
    }

    async fn get(&self, key: &str) -> Result<(Vec<u8>, ObjectMeta), ObjectError> {
        self.objects
            .read()
            .get(key)
            .cloned()
            .ok_or_else(|| ObjectError::NotFound(key.into()))
    }

    async fn delete(&self, key: &str) -> Result<(), ObjectError> {
        self.objects
            .write()
            .remove(key)
            .map(|_| ())
            .ok_or_else(|| ObjectError::NotFound(key.into()))
    }

    async fn exists(&self, key: &str) -> Result<bool, ObjectError> {
        Ok(self.objects.read().contains_key(key))
    }

    async fn list(&self, prefix: &str, limit: usize) -> Result<Vec<ObjectMeta>, ObjectError> {
        let objects = self.objects.read();
        let items: Vec<ObjectMeta> = objects
            .iter()
            .filter(|(k, _)| k.starts_with(prefix))
            .take(limit)
            .map(|(_, (_, meta))| meta.clone())
            .collect();
        Ok(items)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_meta(key: &str, tenant_id: Uuid) -> ObjectMeta {
        ObjectMeta {
            key: key.into(),
            tenant_id,
            artifact_type: ArtifactType::Recording,
            content_type: "audio/wav".into(),
            size_bytes: 1024,
            checksum: Some("sha256:abc123".into()),
            retention_class: "standard_operational".into(),
            created_at: Utc::now().to_rfc3339(),
        }
    }

    #[tokio::test]
    async fn put_get_delete() {
        let store = MemoryObjectStore::default();
        let tid = Uuid::new_v4();
        let key = "test/recording.wav";

        store
            .put(key, vec![1, 2, 3], test_meta(key, tid))
            .await
            .unwrap();
        assert!(store.exists(key).await.unwrap());

        let (data, meta) = store.get(key).await.unwrap();
        assert_eq!(data, vec![1, 2, 3]);
        assert_eq!(meta.tenant_id, tid);

        store.delete(key).await.unwrap();
        assert!(!store.exists(key).await.unwrap());
    }

    #[tokio::test]
    async fn list_by_prefix() {
        let store = MemoryObjectStore::default();
        let tid = Uuid::new_v4();

        store
            .put("tenant-a/rec1.wav", vec![1], test_meta("rec1", tid))
            .await
            .unwrap();
        store
            .put("tenant-a/rec2.wav", vec![2], test_meta("rec2", tid))
            .await
            .unwrap();
        store
            .put("tenant-b/rec3.wav", vec![3], test_meta("rec3", tid))
            .await
            .unwrap();

        let items = store.list("tenant-a/", 10).await.unwrap();
        assert_eq!(items.len(), 2);
    }

    #[tokio::test]
    async fn size_limit_enforced() {
        let store = MemoryObjectStore::new(10);
        let result = store
            .put("big", vec![0u8; 100], test_meta("big", Uuid::new_v4()))
            .await;
        assert!(matches!(result, Err(ObjectError::TooLarge { .. })));
    }

    #[test]
    fn object_key_format() {
        let key = object_key(
            "us-east-1",
            Uuid::nil(),
            &ArtifactType::Recording,
            Uuid::nil(),
            Uuid::nil(),
        );
        assert!(key.starts_with("us-east-1/"));
        assert!(key.contains("recording"));
    }
}
