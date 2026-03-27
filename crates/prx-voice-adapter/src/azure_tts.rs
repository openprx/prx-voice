//! Azure Cognitive Services TTS adapter — real text-to-speech.
//!
//! Calls Azure Speech Services REST API for synthesis.
//! Requires AZURE_SPEECH_KEY and AZURE_SPEECH_REGION env vars.

use crate::health::{AdapterStatus, HealthReport};
use crate::tts::{TtsAdapter, TtsChunk, TtsError, TtsSynthesisRequest};
use serde::{Deserialize, Serialize};
use tokio::sync::mpsc;
use tracing::{error, info};

/// Azure TTS configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AzureTtsConfig {
    /// Azure Speech subscription key.
    pub subscription_key: Option<String>,
    /// Azure region (e.g., eastus).
    pub region: String,
    /// Default voice.
    pub voice: String,
    /// Output format.
    pub output_format: String,
}

impl Default for AzureTtsConfig {
    fn default() -> Self {
        Self {
            subscription_key: None,
            region: "eastus".into(),
            voice: "en-US-JennyNeural".into(),
            output_format: "audio-16khz-16bit-mono-pcm".into(),
        }
    }
}

impl AzureTtsConfig {
    /// Resolve the subscription key from config or environment.
    pub fn resolve_key(&self) -> Result<String, TtsError> {
        self.subscription_key
            .clone()
            .or_else(|| std::env::var("AZURE_SPEECH_KEY").ok())
            .ok_or_else(|| {
                TtsError::Internal(
                    "Azure Speech key not configured. Set AZURE_SPEECH_KEY env var.".into(),
                )
            })
    }

    /// Resolve the region from environment or config.
    pub fn resolve_region(&self) -> String {
        std::env::var("AZURE_SPEECH_REGION").unwrap_or_else(|_| self.region.clone())
    }

    /// Build the REST endpoint URL.
    pub fn endpoint(&self) -> String {
        let region = self.resolve_region();
        format!("https://{region}.tts.speech.microsoft.com/cognitiveservices/v1")
    }
}

/// Azure TTS adapter.
pub struct AzureTts {
    config: AzureTtsConfig,
    client: reqwest::Client,
    initialized: bool,
}

impl AzureTts {
    /// Create a new Azure TTS adapter with the given configuration.
    pub fn new(config: AzureTtsConfig) -> Self {
        Self {
            config,
            client: reqwest::Client::new(),
            initialized: false,
        }
    }
}

#[async_trait::async_trait]
impl TtsAdapter for AzureTts {
    async fn initialize(&mut self) -> Result<(), TtsError> {
        let _ = self.config.resolve_key()?;
        self.initialized = true;
        info!(provider = "azure", voice = %self.config.voice, "TTS adapter initialized");
        Ok(())
    }

    async fn synthesize(
        &self,
        request: TtsSynthesisRequest,
    ) -> Result<mpsc::Receiver<TtsChunk>, TtsError> {
        let key = self.config.resolve_key()?;
        let endpoint = self.config.endpoint();
        let voice = if request.voice == "default" {
            self.config.voice.clone()
        } else {
            request.voice.clone()
        };

        // Build SSML
        let ssml = format!(
            r#"<speak version="1.0" xmlns="http://www.w3.org/2001/10/synthesis" xml:lang="{lang}">
                <voice name="{voice}">{text}</voice>
            </speak>"#,
            lang = request.language,
            voice = voice,
            text = xml_escape(&request.text),
        );

        let (tx, rx) = mpsc::channel(32);
        let client = self.client.clone();
        let output_format = self.config.output_format.clone();
        let segment_id = request.segment_id.clone();
        let sample_rate = request.sample_rate;

        tokio::spawn(async move {
            let start = std::time::Instant::now();

            let response = match client
                .post(&endpoint)
                .header("Ocp-Apim-Subscription-Key", &key)
                .header("Content-Type", "application/ssml+xml")
                .header("X-Microsoft-OutputFormat", &output_format)
                .body(ssml)
                .send()
                .await
            {
                Ok(r) => r,
                Err(e) => {
                    error!(error = %e, "Azure TTS request failed");
                    return;
                }
            };

            if !response.status().is_success() {
                let status = response.status();
                let body = response.text().await.unwrap_or_default();
                error!(%status, %body, "Azure TTS error");
                return;
            }

            // Read response body as audio bytes
            let audio_bytes = match response.bytes().await {
                Ok(b) => b.to_vec(),
                Err(e) => {
                    error!(error = %e, "Failed to read Azure TTS response");
                    return;
                }
            };

            let synthesis_latency_ms = start.elapsed().as_millis() as u64;

            // Split into chunks (e.g., 200ms per chunk at 16kHz 16-bit mono = 6400 bytes)
            let bytes_per_chunk = (sample_rate as usize) * 2 * 200 / 1000; // 200ms chunks
            let total_chunks = audio_bytes.len().div_ceil(bytes_per_chunk);

            for (i, chunk_data) in audio_bytes.chunks(bytes_per_chunk.max(1)).enumerate() {
                let is_final = i == total_chunks - 1;
                let audio_length_ms = (chunk_data.len() as u64 * 1000) / (sample_rate as u64 * 2);

                let chunk = TtsChunk {
                    segment_id: segment_id.clone(),
                    chunk_index: i as u32,
                    audio_data: chunk_data.to_vec(),
                    audio_length_ms,
                    encoding: "pcm16".into(),
                    sample_rate,
                    is_final,
                    synthesis_latency_ms: if i == 0 { synthesis_latency_ms } else { 0 },
                };

                if tx.send(chunk).await.is_err() {
                    break;
                }
            }
        });

        Ok(rx)
    }

    async fn cancel(&self) -> Result<(), TtsError> {
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
        "azure"
    }

    fn voice(&self) -> &str {
        &self.config.voice
    }
}

fn xml_escape(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&apos;")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn config_builds_endpoint() {
        let config = AzureTtsConfig {
            region: "westus2".into(),
            ..Default::default()
        };
        assert_eq!(
            config.endpoint(),
            "https://westus2.tts.speech.microsoft.com/cognitiveservices/v1"
        );
    }

    #[test]
    fn xml_escape_works() {
        assert_eq!(
            xml_escape("Hello <world> & \"friends\""),
            "Hello &lt;world&gt; &amp; &quot;friends&quot;"
        );
    }
}
