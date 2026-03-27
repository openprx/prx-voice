//! Ollama Agent adapter — calls local Ollama HTTP API (OpenAI-compatible).
//!
//! Ollama runs locally and exposes the same /v1/chat/completions API as OpenAI.
//! This adapter reuses the SSE streaming logic from openai_agent but points to localhost.

use crate::agent::{AgentAdapter, AgentContext, AgentError, AgentToken};
use crate::health::{AdapterStatus, HealthReport};
use serde::{Deserialize, Serialize};
use tokio::sync::mpsc;
use tracing::{error, info, warn};

/// Ollama configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OllamaConfig {
    /// Ollama API endpoint (default: http://localhost:11434).
    pub endpoint: String,
    /// Model name (e.g., "qwen2.5:1.5b", "llama3.1:8b", "mistral:7b").
    pub model: String,
    /// Max tokens to generate.
    pub max_tokens: u32,
    /// Temperature.
    pub temperature: f64,
    /// System prompt.
    pub system_prompt: Option<String>,
}

impl Default for OllamaConfig {
    fn default() -> Self {
        Self {
            endpoint: "http://localhost:11434".into(),
            model: "qwen2.5:1.5b".into(),
            max_tokens: 1024,
            temperature: 0.7,
            system_prompt: Some(
                "你是一个语音助手。请始终用中文回答，回答要简洁自然，像正常对话一样。".into(),
            ),
        }
    }
}

/// Ollama Agent adapter.
pub struct OllamaAgent {
    config: OllamaConfig,
    client: reqwest::Client,
    initialized: bool,
}

impl OllamaAgent {
    pub fn new(config: OllamaConfig) -> Self {
        Self {
            config,
            client: reqwest::Client::new(),
            initialized: false,
        }
    }
}

#[derive(Debug, Serialize)]
struct OllamaRequest {
    model: String,
    messages: Vec<OllamaMessage>,
    stream: bool,
    options: OllamaOptions,
}

#[derive(Debug, Serialize)]
struct OllamaMessage {
    role: String,
    content: String,
}

#[derive(Debug, Serialize)]
struct OllamaOptions {
    temperature: f64,
    num_predict: u32,
}

#[derive(Debug, Deserialize)]
struct OllamaStreamResponse {
    message: Option<OllamaStreamMessage>,
    done: Option<bool>,
}

#[derive(Debug, Deserialize)]
struct OllamaStreamMessage {
    content: Option<String>,
}

#[async_trait::async_trait]
impl AgentAdapter for OllamaAgent {
    async fn initialize(&mut self) -> Result<(), AgentError> {
        // Check if Ollama is running
        let health_url = format!("{}/api/tags", self.config.endpoint);
        match self.client.get(&health_url).send().await {
            Ok(resp) if resp.status().is_success() => {
                self.initialized = true;
                info!(
                    model = %self.config.model,
                    endpoint = %self.config.endpoint,
                    "Ollama agent initialized"
                );
                Ok(())
            }
            Ok(resp) => {
                let status = resp.status();
                Err(AgentError::Internal(format!("Ollama returned {status}")))
            }
            Err(e) => {
                warn!(
                    error = %e,
                    endpoint = %self.config.endpoint,
                    "Ollama not reachable, adapter will be degraded"
                );
                // Don't fail init — allow graceful degradation
                self.initialized = false;
                Err(AgentError::Internal(format!("Ollama not reachable: {e}")))
            }
        }
    }

    async fn generate(
        &self,
        transcript: &str,
        context: &AgentContext,
    ) -> Result<mpsc::Receiver<AgentToken>, AgentError> {
        let mut messages = Vec::new();

        if let Some(sp) = context
            .system_prompt
            .as_ref()
            .or(self.config.system_prompt.as_ref())
        {
            messages.push(OllamaMessage {
                role: "system".into(),
                content: sp.clone(),
            });
        }
        for turn in &context.history {
            messages.push(OllamaMessage {
                role: turn.role.clone(),
                content: turn.content.clone(),
            });
        }
        messages.push(OllamaMessage {
            role: "user".into(),
            content: transcript.to_string(),
        });

        let body = OllamaRequest {
            model: self.config.model.clone(),
            messages,
            stream: true,
            options: OllamaOptions {
                temperature: self.config.temperature,
                num_predict: self.config.max_tokens,
            },
        };

        let (tx, rx) = mpsc::channel(128);
        let client = self.client.clone();
        let endpoint = format!("{}/api/chat", self.config.endpoint);

        tokio::spawn(async move {
            let response = match client.post(&endpoint).json(&body).send().await {
                Ok(r) => r,
                Err(e) => {
                    error!(error = %e, "Ollama request failed");
                    return;
                }
            };

            if !response.status().is_success() {
                let status = response.status();
                let body_text = response.text().await.unwrap_or_default();
                error!(%status, %body_text, "Ollama error");
                return;
            }

            // Ollama streams newline-delimited JSON
            use futures_util::StreamExt;
            let mut stream = response.bytes_stream();
            let mut buffer = String::new();
            let mut cumulative = String::new();
            let mut token_index: u32 = 0;

            while let Some(chunk_result) = stream.next().await {
                let chunk = match chunk_result {
                    Ok(c) => c,
                    Err(e) => {
                        warn!(error = %e, "Ollama stream read error");
                        break;
                    }
                };

                buffer.push_str(&String::from_utf8_lossy(&chunk));

                while let Some(line_end) = buffer.find('\n') {
                    let line = buffer[..line_end].trim().to_string();
                    buffer = buffer[line_end + 1..].to_string();

                    if line.is_empty() {
                        continue;
                    }

                    if let Ok(resp) = serde_json::from_str::<OllamaStreamResponse>(&line) {
                        let is_done = resp.done.unwrap_or(false);

                        if let Some(msg) = resp.message {
                            if let Some(content) = msg.content {
                                if !content.is_empty() {
                                    cumulative.push_str(&content);
                                    let token = AgentToken {
                                        token_index,
                                        token: content,
                                        cumulative_text: cumulative.clone(),
                                        finish_reason: if is_done {
                                            Some("stop".into())
                                        } else {
                                            None
                                        },
                                    };
                                    token_index += 1;
                                    if tx.send(token).await.is_err() {
                                        return;
                                    }
                                }
                            }
                        }

                        if is_done && !cumulative.is_empty() {
                            // Send final token with finish_reason if we haven't already
                            let _ = tx
                                .send(AgentToken {
                                    token_index,
                                    token: String::new(),
                                    cumulative_text: cumulative.clone(),
                                    finish_reason: Some("stop".into()),
                                })
                                .await;
                            return;
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
            message: Some(format!("ollama/{}", self.config.model)),
        }
    }

    fn provider(&self) -> &str {
        "ollama"
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
        let c = OllamaConfig::default();
        assert_eq!(c.endpoint, "http://localhost:11434");
        assert_eq!(c.model, "qwen2.5:1.5b");
    }

    #[test]
    fn config_serializes() {
        let c = OllamaConfig::default();
        let json = serde_json::to_string(&c).unwrap();
        assert!(json.contains("11434"));
        assert!(json.contains("qwen2.5"));
    }
}
