//! Agent (LLM) adapter trait.

use crate::health::HealthReport;
use serde::{Deserialize, Serialize};
use thiserror::Error;
use tokio::sync::mpsc;

/// A token from the agent's streaming response.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentToken {
    pub token_index: u32,
    pub token: String,
    pub cumulative_text: String,
    pub finish_reason: Option<String>,
}

/// Final agent response summary.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentResponse {
    pub response_text: String,
    pub prompt_tokens: u32,
    pub completion_tokens: u32,
    pub first_token_latency_ms: u64,
    pub total_latency_ms: u64,
    pub finish_reason: String,
}

/// Agent adapter errors.
#[derive(Debug, Error)]
pub enum AgentError {
    #[error("Agent timeout after {timeout_ms}ms")]
    Timeout { timeout_ms: u64 },
    #[error("Agent rate limited, retry after {retry_after_ms}ms")]
    RateLimited { retry_after_ms: u64 },
    #[error("Agent provider error: {message}")]
    ProviderError { message: String, retryable: bool },
    #[error("Agent cancelled")]
    Cancelled,
    #[error("Agent internal error: {0}")]
    Internal(String),
}

/// Agent adapter trait — LLM text generation.
#[async_trait::async_trait]
pub trait AgentAdapter: Send + Sync {
    /// Initialize the adapter.
    async fn initialize(&mut self) -> Result<(), AgentError>;

    /// Send transcript and get streaming token response.
    async fn generate(
        &self,
        transcript: &str,
        context: &AgentContext,
    ) -> Result<mpsc::Receiver<AgentToken>, AgentError>;

    /// Cancel an in-flight generation.
    async fn cancel(&self) -> Result<(), AgentError>;

    /// Health check.
    async fn health(&self) -> HealthReport;

    /// Warm up connections (pre-execution preparation).
    async fn warmup(&self) -> Result<(), AgentError> {
        Ok(())
    }

    /// Stop accepting new requests, finish in-flight.
    async fn drain(&self) -> Result<(), AgentError> {
        Ok(())
    }

    /// Clean up resources.
    async fn shutdown(&self) -> Result<(), AgentError> {
        Ok(())
    }

    /// Provider name.
    fn provider(&self) -> &str;

    /// Model name.
    fn model(&self) -> &str;
}

/// Context passed to the agent for each turn.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentContext {
    pub session_id: String,
    pub turn_id: u32,
    pub language: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub system_prompt: Option<String>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub history: Vec<ConversationTurn>,
}

/// A previous turn in conversation history.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConversationTurn {
    pub role: String,
    pub content: String,
}
