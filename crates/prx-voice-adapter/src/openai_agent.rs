//! OpenAI Agent adapter — real LLM chat completion via streaming API.
//!
//! Calls POST https://api.openai.com/v1/chat/completions with stream=true.
//! Requires OPENAI_API_KEY environment variable.

use crate::agent::{AgentAdapter, AgentContext, AgentError, AgentToken};
use crate::health::{AdapterStatus, HealthReport};
use serde::{Deserialize, Serialize};
use tokio::sync::mpsc;
use tracing::{error, info, warn};

/// OpenAI adapter configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OpenAiConfig {
    /// OpenAI API key (or read from OPENAI_API_KEY env).
    pub api_key: Option<String>,
    /// Model to use (default: gpt-4o).
    pub model: String,
    /// Max tokens to generate.
    pub max_tokens: u32,
    /// Temperature (0.0-2.0).
    pub temperature: f64,
    /// API endpoint.
    pub endpoint: String,
    /// Default system prompt.
    pub system_prompt: Option<String>,
}

impl Default for OpenAiConfig {
    fn default() -> Self {
        Self {
            api_key: None,
            model: "gpt-4o".into(),
            max_tokens: 1024,
            temperature: 0.7,
            endpoint: "https://api.openai.com/v1/chat/completions".into(),
            system_prompt: Some(
                "You are a helpful voice assistant. Keep responses concise and conversational."
                    .into(),
            ),
        }
    }
}

impl OpenAiConfig {
    pub fn resolve_api_key(&self) -> Result<String, AgentError> {
        self.api_key
            .clone()
            .or_else(|| std::env::var("OPENAI_API_KEY").ok())
            .ok_or_else(|| {
                AgentError::Internal(
                    "OpenAI API key not configured. Set OPENAI_API_KEY env var or config.".into(),
                )
            })
    }
}

/// OpenAI streaming Agent adapter.
pub struct OpenAiAgent {
    config: OpenAiConfig,
    client: reqwest::Client,
    initialized: bool,
}

impl OpenAiAgent {
    pub fn new(config: OpenAiConfig) -> Self {
        Self {
            config,
            client: reqwest::Client::new(),
            initialized: false,
        }
    }
}

/// OpenAI request types.
#[derive(Debug, Serialize)]
struct ChatRequest {
    model: String,
    messages: Vec<ChatMessage>,
    max_tokens: u32,
    temperature: f64,
    stream: bool,
}

#[derive(Debug, Serialize)]
struct ChatMessage {
    role: String,
    content: String,
}

/// OpenAI SSE streaming response chunk.
#[derive(Debug, Deserialize)]
struct StreamChunk {
    choices: Vec<StreamChoice>,
}

#[derive(Debug, Deserialize)]
struct StreamChoice {
    delta: StreamDelta,
    finish_reason: Option<String>,
}

#[derive(Debug, Deserialize)]
struct StreamDelta {
    content: Option<String>,
}

#[async_trait::async_trait]
impl AgentAdapter for OpenAiAgent {
    async fn initialize(&mut self) -> Result<(), AgentError> {
        let _ = self.config.resolve_api_key()?;
        self.initialized = true;
        info!(provider = "openai", model = %self.config.model, "Agent adapter initialized");
        Ok(())
    }

    async fn generate(
        &self,
        transcript: &str,
        context: &AgentContext,
    ) -> Result<mpsc::Receiver<AgentToken>, AgentError> {
        let api_key = self.config.resolve_api_key()?;

        // Build messages array
        let mut messages = Vec::new();

        // System prompt
        let system_prompt = context
            .system_prompt
            .clone()
            .or_else(|| self.config.system_prompt.clone());
        if let Some(sp) = system_prompt {
            messages.push(ChatMessage {
                role: "system".into(),
                content: sp,
            });
        }

        // Conversation history
        for turn in &context.history {
            messages.push(ChatMessage {
                role: turn.role.clone(),
                content: turn.content.clone(),
            });
        }

        // Current user message
        messages.push(ChatMessage {
            role: "user".into(),
            content: transcript.to_string(),
        });

        let request_body = ChatRequest {
            model: self.config.model.clone(),
            messages,
            max_tokens: self.config.max_tokens,
            temperature: self.config.temperature,
            stream: true,
        };

        let (tx, rx) = mpsc::channel(128);
        let client = self.client.clone();
        let endpoint = self.config.endpoint.clone();

        tokio::spawn(async move {
            let response = match client
                .post(&endpoint)
                .header("Authorization", format!("Bearer {api_key}"))
                .header("Content-Type", "application/json")
                .json(&request_body)
                .send()
                .await
            {
                Ok(r) => r,
                Err(e) => {
                    error!(error = %e, "OpenAI API request failed");
                    return;
                }
            };

            if !response.status().is_success() {
                let status = response.status();
                let body = response.text().await.unwrap_or_default();
                error!(%status, %body, "OpenAI API error");
                return;
            }

            // Process SSE stream
            let mut cumulative = String::new();
            let mut token_index: u32 = 0;
            let bytes_stream = response.bytes_stream();

            use futures_util::StreamExt;
            let mut stream = bytes_stream;
            let mut buffer = String::new();

            while let Some(chunk_result) = stream.next().await {
                let chunk = match chunk_result {
                    Ok(c) => c,
                    Err(e) => {
                        warn!(error = %e, "Stream read error");
                        break;
                    }
                };

                buffer.push_str(&String::from_utf8_lossy(&chunk));

                // Process complete SSE lines
                while let Some(line_end) = buffer.find('\n') {
                    let line = buffer[..line_end].trim().to_string();
                    buffer = buffer[line_end + 1..].to_string();

                    if line.is_empty() || line == "data: [DONE]" {
                        continue;
                    }

                    if let Some(json_str) = line.strip_prefix("data: ") {
                        if let Ok(chunk) = serde_json::from_str::<StreamChunk>(json_str) {
                            for choice in &chunk.choices {
                                if let Some(content) = &choice.delta.content {
                                    cumulative.push_str(content);
                                    let token = AgentToken {
                                        token_index,
                                        token: content.clone(),
                                        cumulative_text: cumulative.clone(),
                                        finish_reason: choice.finish_reason.clone(),
                                    };
                                    token_index += 1;
                                    if tx.send(token).await.is_err() {
                                        return;
                                    }
                                }
                                // Emit final token with finish_reason if no content
                                if let Some(reason) = &choice.finish_reason {
                                    if choice.delta.content.is_none() && !cumulative.is_empty() {
                                        let token = AgentToken {
                                            token_index,
                                            token: String::new(),
                                            cumulative_text: cumulative.clone(),
                                            finish_reason: Some(reason.clone()),
                                        };
                                        let _ = tx.send(token).await;
                                    }
                                }
                            }
                        }
                    }
                }
            }
        });

        Ok(rx)
    }

    async fn cancel(&self) -> Result<(), AgentError> {
        Ok(())
    }

    async fn health(&self) -> HealthReport {
        HealthReport {
            status: if self.initialized {
                AdapterStatus::Ready
            } else {
                AdapterStatus::Down
            },
            latency_ms: None,
            error_rate_pct: None,
            message: None,
        }
    }

    fn provider(&self) -> &str {
        "openai"
    }

    fn model(&self) -> &str {
        &self.config.model
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_config() {
        let config = OpenAiConfig::default();
        assert_eq!(config.model, "gpt-4o");
        assert_eq!(config.max_tokens, 1024);
        assert!(config.system_prompt.is_some());
    }

    #[test]
    fn config_missing_api_key() {
        let config = OpenAiConfig {
            api_key: None,
            ..Default::default()
        };
        let _result = config.resolve_api_key();
    }
}
