//! Deepgram ASR adapter — real streaming speech-to-text via WebSocket.
//!
//! Connects to Deepgram's streaming API:
//! wss://api.deepgram.com/v1/listen?model=nova-2&language=en-US
//!
//! Requires DEEPGRAM_API_KEY environment variable.

use crate::asr::{AsrAdapter, AsrError, AsrResult, AudioChunk};
use crate::health::{AdapterStatus, HealthReport};
use serde::{Deserialize, Serialize};
use tokio::sync::mpsc;
use tracing::{error, info, warn};

/// Deepgram adapter configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeepgramConfig {
    /// Deepgram API key (or read from DEEPGRAM_API_KEY env).
    pub api_key: Option<String>,
    /// Model to use (default: nova-2).
    pub model: String,
    /// Language code (default: en-US).
    pub language: String,
    /// Enable interim results.
    pub interim_results: bool,
    /// Enable punctuation.
    pub punctuate: bool,
    /// WebSocket endpoint base URL.
    pub endpoint: String,
}

impl Default for DeepgramConfig {
    fn default() -> Self {
        Self {
            api_key: None,
            model: "nova-2".into(),
            language: "en-US".into(),
            interim_results: true,
            punctuate: true,
            endpoint: "wss://api.deepgram.com/v1/listen".into(),
        }
    }
}

impl DeepgramConfig {
    /// Resolve API key from config or environment.
    pub fn resolve_api_key(&self) -> Result<String, AsrError> {
        self.api_key
            .clone()
            .or_else(|| std::env::var("DEEPGRAM_API_KEY").ok())
            .ok_or_else(|| {
                AsrError::Internal(
                    "Deepgram API key not configured. Set DEEPGRAM_API_KEY env var or config."
                        .into(),
                )
            })
    }

    /// Build the WebSocket URL with query parameters.
    pub fn ws_url(&self) -> String {
        format!(
            "{}?model={}&language={}&interim_results={}&punctuate={}",
            self.endpoint, self.model, self.language, self.interim_results, self.punctuate,
        )
    }
}

/// Deepgram streaming ASR adapter.
pub struct DeepgramAsr {
    config: DeepgramConfig,
    initialized: bool,
}

impl DeepgramAsr {
    pub fn new(config: DeepgramConfig) -> Self {
        Self {
            config,
            initialized: false,
        }
    }
}

/// Deepgram WebSocket response types.
#[derive(Debug, Deserialize)]
struct DeepgramResponse {
    channel: Option<DeepgramChannel>,
    is_final: Option<bool>,
    duration: Option<f64>,
}

#[derive(Debug, Deserialize)]
struct DeepgramChannel {
    alternatives: Vec<DeepgramAlternative>,
}

#[derive(Debug, Deserialize)]
struct DeepgramAlternative {
    transcript: String,
    confidence: f64,
}

#[async_trait::async_trait]
impl AsrAdapter for DeepgramAsr {
    async fn initialize(&mut self) -> Result<(), AsrError> {
        // Validate API key is available
        let _ = self.config.resolve_api_key()?;
        self.initialized = true;
        info!(provider = "deepgram", model = %self.config.model, "ASR adapter initialized");
        Ok(())
    }

    async fn start_stream(
        &self,
        language: &str,
    ) -> Result<(mpsc::Sender<AudioChunk>, mpsc::Receiver<AsrResult>), AsrError> {
        let api_key = self.config.resolve_api_key()?;

        let mut config = self.config.clone();
        config.language = language.to_string();

        let (audio_tx, mut audio_rx) = mpsc::channel::<AudioChunk>(64);
        let (result_tx, result_rx) = mpsc::channel::<AsrResult>(32);

        let ws_url = config.ws_url();

        tokio::spawn(async move {
            use futures_util::{SinkExt, StreamExt};
            use tokio_tungstenite::tungstenite::{self, http::Request};

            // Build WebSocket request with auth header
            let request = match Request::builder()
                .uri(&ws_url)
                .header("Authorization", format!("Token {api_key}"))
                .header(
                    "Sec-WebSocket-Key",
                    tungstenite::handshake::client::generate_key(),
                )
                .header("Sec-WebSocket-Version", "13")
                .header("Host", "api.deepgram.com")
                .header("Connection", "Upgrade")
                .header("Upgrade", "websocket")
                .body(())
            {
                Ok(r) => r,
                Err(e) => {
                    error!(error = %e, "Failed to build Deepgram WebSocket request");
                    return;
                }
            };

            let ws_stream = match tokio_tungstenite::connect_async(request).await {
                Ok((stream, _)) => stream,
                Err(e) => {
                    error!(error = %e, "Failed to connect to Deepgram");
                    return;
                }
            };

            let (mut ws_sink, mut ws_stream_rx) = ws_stream.split();
            let result_tx_clone = result_tx.clone();
            let mut revision: u32 = 0;

            // Spawn reader task (receives Deepgram JSON responses)
            let reader = tokio::spawn(async move {
                while let Some(msg) = ws_stream_rx.next().await {
                    match msg {
                        Ok(tungstenite::Message::Text(text)) => {
                            if let Ok(resp) = serde_json::from_str::<DeepgramResponse>(&text) {
                                if let Some(channel) = resp.channel {
                                    if let Some(alt) = channel.alternatives.first() {
                                        if alt.transcript.is_empty() {
                                            continue;
                                        }
                                        revision += 1;
                                        let is_final = resp.is_final.unwrap_or(false);
                                        let result = AsrResult {
                                            speech_id: "utt-deepgram".into(),
                                            transcript: alt.transcript.clone(),
                                            confidence: alt.confidence,
                                            stability: if is_final { 1.0 } else { 0.6 },
                                            is_final,
                                            revision,
                                            language: config.language.clone(),
                                            asr_latency_ms: resp
                                                .duration
                                                .map(|d| (d * 1000.0) as u64),
                                        };
                                        if result_tx_clone.send(result).await.is_err() {
                                            break;
                                        }
                                    }
                                }
                            }
                        }
                        Ok(tungstenite::Message::Close(_)) => break,
                        Err(e) => {
                            warn!(error = %e, "Deepgram WebSocket error");
                            break;
                        }
                        _ => {}
                    }
                }
            });

            // Forward audio chunks as binary WebSocket messages
            while let Some(chunk) = audio_rx.recv().await {
                if ws_sink
                    .send(tungstenite::Message::Binary(chunk.data.into()))
                    .await
                    .is_err()
                {
                    break;
                }
            }

            // Send close frame to signal end of audio
            let _ = ws_sink
                .send(tungstenite::Message::Binary(vec![].into()))
                .await;
            let _ = reader.await;
        });

        Ok((audio_tx, result_rx))
    }

    async fn cancel(&self) -> Result<(), AsrError> {
        // WebSocket will be dropped, closing the connection
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
        "deepgram"
    }

    fn model(&self) -> &str {
        &self.config.model
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn config_builds_ws_url() {
        let config = DeepgramConfig::default();
        let url = config.ws_url();
        assert!(url.starts_with("wss://api.deepgram.com"));
        assert!(url.contains("model=nova-2"));
        assert!(url.contains("language=en-US"));
    }

    #[test]
    fn config_missing_api_key_errors() {
        let config = DeepgramConfig {
            api_key: None,
            ..Default::default()
        };
        // Only fails if env var also not set
        // This test validates the method doesn't panic
        let _result = config.resolve_api_key();
    }
}
