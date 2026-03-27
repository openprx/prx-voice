//! Mock Agent adapter for testing.

use crate::agent::{AgentAdapter, AgentContext, AgentError, AgentToken};
use crate::health::{AdapterStatus, HealthReport};
use serde::{Deserialize, Serialize};
use tokio::sync::mpsc;

/// Configuration for the mock agent adapter.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MockAgentConfig {
    /// Simulated first-token latency (ms).
    pub first_token_latency_ms: u64,
    /// Fixed response text.
    pub response_text: String,
    /// Whether to simulate an error.
    pub inject_error: bool,
}

impl Default for MockAgentConfig {
    fn default() -> Self {
        Self {
            first_token_latency_ms: 200,
            response_text: "你好！有什么可以帮你的吗？".into(),
            inject_error: false,
        }
    }
}

/// Mock Agent adapter.
pub struct MockAgent {
    config: MockAgentConfig,
}

impl MockAgent {
    pub fn new(config: MockAgentConfig) -> Self {
        Self { config }
    }
}

#[async_trait::async_trait]
impl AgentAdapter for MockAgent {
    async fn initialize(&mut self) -> Result<(), AgentError> {
        Ok(())
    }

    async fn generate(
        &self,
        _transcript: &str,
        _context: &AgentContext,
    ) -> Result<mpsc::Receiver<AgentToken>, AgentError> {
        if self.config.inject_error {
            return Err(AgentError::ProviderError {
                message: "Mock injected error".into(),
                retryable: false,
            });
        }

        let (tx, rx) = mpsc::channel(64);
        let config = self.config.clone();

        tokio::spawn(async move {
            tokio::time::sleep(tokio::time::Duration::from_millis(
                config.first_token_latency_ms,
            ))
            .await;

            let words: Vec<&str> = config.response_text.split_whitespace().collect();
            let mut cumulative = String::new();

            for (i, word) in words.iter().enumerate() {
                if !cumulative.is_empty() {
                    cumulative.push(' ');
                }
                cumulative.push_str(word);

                let is_last = i == words.len() - 1;
                let _ = tx
                    .send(AgentToken {
                        token_index: i as u32,
                        token: word.to_string(),
                        cumulative_text: cumulative.clone(),
                        finish_reason: if is_last { Some("stop".into()) } else { None },
                    })
                    .await;

                // Small delay between tokens
                tokio::time::sleep(tokio::time::Duration::from_millis(5)).await;
            }
        });

        Ok(rx)
    }

    async fn cancel(&self) -> Result<(), AgentError> {
        Ok(())
    }

    async fn health(&self) -> HealthReport {
        HealthReport {
            status: if self.config.inject_error {
                AdapterStatus::Down
            } else {
                AdapterStatus::Ready
            },
            latency_ms: Some(self.config.first_token_latency_ms),
            error_rate_pct: Some(0.0),
            message: None,
        }
    }

    fn provider(&self) -> &str {
        "mock"
    }

    fn model(&self) -> &str {
        "mock-agent-v1"
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn mock_agent_streams_tokens() {
        let mut agent = MockAgent::new(MockAgentConfig {
            first_token_latency_ms: 10,
            response_text: "Hello world".into(),
            inject_error: false,
        });
        agent.initialize().await.unwrap();

        let ctx = AgentContext {
            session_id: "sess-test".into(),
            turn_id: 1,
            language: "en-US".into(),
            system_prompt: None,
            history: vec![],
        };

        let mut rx = agent.generate("test input", &ctx).await.unwrap();

        let t1 = rx.recv().await.unwrap();
        assert_eq!(t1.token, "Hello");
        assert!(t1.finish_reason.is_none());

        let t2 = rx.recv().await.unwrap();
        assert_eq!(t2.token, "world");
        assert_eq!(t2.finish_reason.as_deref(), Some("stop"));
        assert_eq!(t2.cumulative_text, "Hello world");
    }
}
