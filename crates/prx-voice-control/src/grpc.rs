//! gRPC internal service contracts.
//! Per the API contract spec: package prx.voice.control.v1
//!
//! These trait definitions mirror what a .proto file would generate.
//! Production would use tonic + prost for actual gRPC transport.

use serde::{Deserialize, Serialize};

/// Session service — session lifecycle management.
#[async_trait::async_trait]
pub trait SessionService: Send + Sync {
    async fn create_session(&self, req: CreateSessionReq) -> Result<SessionResp, GrpcError>;
    async fn get_session(&self, req: GetSessionReq) -> Result<SessionResp, GrpcError>;
    async fn close_session(&self, req: CloseSessionReq) -> Result<SessionResp, GrpcError>;
    async fn interrupt_session(&self, req: InterruptSessionReq) -> Result<SessionResp, GrpcError>;
    async fn pause_session(&self, req: PauseSessionReq) -> Result<SessionResp, GrpcError>;
    async fn resume_session(&self, req: ResumeSessionReq) -> Result<SessionResp, GrpcError>;
    async fn list_sessions(&self, req: ListSessionsReq) -> Result<ListSessionsResp, GrpcError>;
}

/// Event service — event publishing and subscription.
#[async_trait::async_trait]
pub trait EventService: Send + Sync {
    async fn publish(&self, req: PublishEventReq) -> Result<PublishEventResp, GrpcError>;
    async fn subscribe(&self, req: SubscribeReq) -> Result<EventStreamResp, GrpcError>;
}

/// Routing service — adapter/provider routing decisions.
#[async_trait::async_trait]
pub trait RoutingService: Send + Sync {
    async fn resolve_route(&self, req: ResolveRouteReq) -> Result<ResolveRouteResp, GrpcError>;
}

/// Tenant policy service — policy enforcement.
#[async_trait::async_trait]
pub trait TenantPolicyService: Send + Sync {
    async fn check_quota(&self, req: CheckQuotaReq) -> Result<CheckQuotaResp, GrpcError>;
    async fn get_policy(&self, req: GetPolicyReq) -> Result<PolicyResp, GrpcError>;
}

/// Audit service — audit log recording.
#[async_trait::async_trait]
pub trait AuditService: Send + Sync {
    async fn record(&self, req: AuditRecordReq) -> Result<AuditRecordResp, GrpcError>;
    async fn query(&self, req: AuditQueryReq) -> Result<AuditQueryResp, GrpcError>;
}

/// Metrics query service — metrics aggregation.
#[async_trait::async_trait]
pub trait MetricsQueryService: Send + Sync {
    async fn query_metrics(&self, req: MetricsQueryReq) -> Result<MetricsQueryResp, GrpcError>;
}

// ── Request / Response types ──

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreateSessionReq {
    pub tenant_id: String,
    pub project_id: Option<String>,
    pub channel: String,
    pub language: String,
    pub asr_providers: Vec<String>,
    pub agent_providers: Vec<String>,
    pub tts_providers: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GetSessionReq {
    pub session_id: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CloseSessionReq {
    pub session_id: String,
    pub reason: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InterruptSessionReq {
    pub session_id: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PauseSessionReq {
    pub session_id: String,
    pub reason: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResumeSessionReq {
    pub session_id: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ListSessionsReq {
    pub tenant_id: Option<String>,
    pub limit: u32,
    pub cursor: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionResp {
    pub session_id: String,
    pub state: String,
    pub channel: String,
    pub language: String,
    pub current_turn_id: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ListSessionsResp {
    pub sessions: Vec<SessionResp>,
    pub total: u64,
    pub cursor: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PublishEventReq {
    pub event_type: String,
    pub session_id: String,
    pub data: serde_json::Value,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PublishEventResp {
    pub seq: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SubscribeReq {
    pub session_id: String,
    pub event_types: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EventStreamResp {
    pub events: Vec<serde_json::Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResolveRouteReq {
    pub tenant_id: String,
    pub adapter_type: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResolveRouteResp {
    pub providers: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CheckQuotaReq {
    pub tenant_id: String,
    pub resource: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CheckQuotaResp {
    pub allowed: bool,
    pub remaining: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GetPolicyReq {
    pub tenant_id: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PolicyResp {
    pub tenant_id: String,
    pub tier: String,
    pub max_concurrent: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuditRecordReq {
    pub action: String,
    pub target_type: String,
    pub target_id: String,
    pub principal: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuditRecordResp {
    pub audit_id: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuditQueryReq {
    pub tenant_id: Option<String>,
    pub limit: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuditQueryResp {
    pub records: Vec<serde_json::Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MetricsQueryReq {
    pub metric_names: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MetricsQueryResp {
    pub metrics: serde_json::Value,
}

/// gRPC error type.
#[derive(Debug, thiserror::Error)]
pub enum GrpcError {
    #[error("Not found: {0}")]
    NotFound(String),
    #[error("Permission denied: {0}")]
    PermissionDenied(String),
    #[error("Invalid argument: {0}")]
    InvalidArgument(String),
    #[error("Resource exhausted: {0}")]
    ResourceExhausted(String),
    #[error("Internal: {0}")]
    Internal(String),
    #[error("Unavailable: {0}")]
    Unavailable(String),
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn request_types_serialize() {
        let req = CreateSessionReq {
            tenant_id: "t-1".into(),
            project_id: None,
            channel: "web_console".into(),
            language: "en-US".into(),
            asr_providers: vec!["deepgram".into()],
            agent_providers: vec!["openai".into()],
            tts_providers: vec!["azure".into()],
        };
        let json = serde_json::to_string(&req).unwrap();
        assert!(json.contains("deepgram"));
    }

    #[test]
    fn grpc_error_display() {
        let err = GrpcError::NotFound("session-123".into());
        assert_eq!(err.to_string(), "Not found: session-123");
    }
}
