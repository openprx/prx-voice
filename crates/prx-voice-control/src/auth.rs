//! Simple token-based API authentication.
//!
//! Phase 4 uses static bearer tokens per tenant.
//! Production would use JWT/OAuth2 with the prx-voice-security crate.

use axum::body::Body;
use axum::http::{Request, StatusCode};
use axum::middleware::Next;
use axum::response::{IntoResponse, Response};
use parking_lot::RwLock;
use prx_voice_types::ids::TenantId;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;

/// Token metadata.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TokenInfo {
    pub tenant_id: TenantId,
    pub principal_id: String,
    pub scopes: Vec<String>,
}

/// In-memory token store.
#[derive(Clone)]
pub struct TokenStore {
    tokens: Arc<RwLock<HashMap<String, TokenInfo>>>,
}

impl TokenStore {
    pub fn new() -> Self {
        Self {
            tokens: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    /// Register a bearer token.
    pub fn register(&self, token: impl Into<String>, info: TokenInfo) {
        self.tokens.write().insert(token.into(), info);
    }

    /// Validate a bearer token.
    pub fn validate(&self, token: &str) -> Option<TokenInfo> {
        self.tokens.read().get(token).cloned()
    }
}

impl Default for TokenStore {
    fn default() -> Self {
        Self::new()
    }
}

/// Extract bearer token from Authorization header.
pub fn extract_bearer_token(header_value: &str) -> Option<&str> {
    header_value
        .strip_prefix("Bearer ")
        .or_else(|| header_value.strip_prefix("bearer "))
}

/// Axum middleware for token authentication.
/// Skips auth for health endpoints.
pub async fn auth_middleware(request: Request<Body>, next: Next) -> Response {
    let path = request.uri().path();

    // Skip auth for health endpoints
    if path.starts_with("/api/v1/health") {
        return next.run(request).await;
    }

    // Check Authorization header
    let auth_header = request
        .headers()
        .get("authorization")
        .and_then(|v| v.to_str().ok());

    match auth_header {
        Some(header) => {
            if extract_bearer_token(header).is_some() {
                // In a real implementation, validate against TokenStore here.
                // Phase 4: accept any bearer token.
                next.run(request).await
            } else {
                (
                    StatusCode::UNAUTHORIZED,
                    "Invalid Authorization header format",
                )
                    .into_response()
            }
        }
        None => {
            // For Phase 4, allow requests without auth (dev mode)
            // In production, this would return 401
            next.run(request).await
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn extract_bearer_token_valid() {
        assert_eq!(extract_bearer_token("Bearer abc123"), Some("abc123"));
        assert_eq!(extract_bearer_token("bearer xyz"), Some("xyz"));
    }

    #[test]
    fn extract_bearer_token_invalid() {
        assert_eq!(extract_bearer_token("Basic abc123"), None);
        assert_eq!(extract_bearer_token(""), None);
    }

    #[test]
    fn token_store_register_and_validate() {
        let store = TokenStore::new();
        let tid = TenantId::new();
        store.register(
            "test-token-123",
            TokenInfo {
                tenant_id: tid,
                principal_id: "user-1".into(),
                scopes: vec!["sessions:write".into(), "sessions:read".into()],
            },
        );

        let info = store
            .validate("test-token-123")
            .expect("BUG: token just registered");
        assert_eq!(info.tenant_id, tid);
        assert_eq!(info.principal_id, "user-1");
        assert_eq!(info.scopes.len(), 2);
    }

    #[test]
    fn token_store_invalid_token() {
        let store = TokenStore::new();
        assert!(store.validate("nonexistent").is_none());
    }
}
