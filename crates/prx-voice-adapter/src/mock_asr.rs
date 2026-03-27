//! Mock ASR adapter for testing.

use crate::asr::{AsrAdapter, AsrError, AsrResult, AudioChunk};
use crate::health::{AdapterStatus, HealthReport};
use serde::{Deserialize, Serialize};
use tokio::sync::mpsc;

/// Configuration for the mock ASR adapter.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MockAsrConfig {
    /// Simulated latency for final transcript (ms).
    pub latency_ms: u64,
    /// Fixed transcript to return.
    pub transcript: String,
    /// Confidence score.
    pub confidence: f64,
    /// Language.
    pub language: String,
    /// Whether to simulate an error.
    pub inject_error: bool,
}

impl Default for MockAsrConfig {
    fn default() -> Self {
        Self {
            latency_ms: 300,
            transcript: "你好".into(),
            confidence: 0.96,
            language: "zh-CN".into(),
            inject_error: false,
        }
    }
}

/// Mock ASR adapter.
pub struct MockAsr {
    config: MockAsrConfig,
}

impl MockAsr {
    pub fn new(config: MockAsrConfig) -> Self {
        Self { config }
    }
}

#[async_trait::async_trait]
impl AsrAdapter for MockAsr {
    async fn initialize(&mut self) -> Result<(), AsrError> {
        Ok(())
    }

    async fn start_stream(
        &self,
        _language: &str,
    ) -> Result<(mpsc::Sender<AudioChunk>, mpsc::Receiver<AsrResult>), AsrError> {
        if self.config.inject_error {
            return Err(AsrError::ProviderError {
                message: "Mock injected error".into(),
                retryable: false,
            });
        }

        let (audio_tx, _audio_rx) = mpsc::channel(32);
        let (result_tx, result_rx) = mpsc::channel(16);

        let config = self.config.clone();
        tokio::spawn(async move {
            // Simulate latency
            tokio::time::sleep(tokio::time::Duration::from_millis(config.latency_ms)).await;

            // Send partial
            let _ = result_tx
                .send(AsrResult {
                    speech_id: "utt-mock-001".into(),
                    transcript: config.transcript[..config.transcript.len() / 2].into(),
                    confidence: config.confidence * 0.8,
                    stability: 0.6,
                    is_final: false,
                    revision: 1,
                    language: config.language.clone(),
                    asr_latency_ms: None,
                })
                .await;

            // Send final
            let _ = result_tx
                .send(AsrResult {
                    speech_id: "utt-mock-001".into(),
                    transcript: config.transcript,
                    confidence: config.confidence,
                    stability: 1.0,
                    is_final: true,
                    revision: 2,
                    language: config.language,
                    asr_latency_ms: Some(config.latency_ms),
                })
                .await;
        });

        Ok((audio_tx, result_rx))
    }

    async fn cancel(&self) -> Result<(), AsrError> {
        Ok(())
    }

    async fn health(&self) -> HealthReport {
        HealthReport {
            status: if self.config.inject_error {
                AdapterStatus::Down
            } else {
                AdapterStatus::Ready
            },
            latency_ms: Some(self.config.latency_ms),
            error_rate_pct: Some(0.0),
            message: None,
        }
    }

    fn provider(&self) -> &str {
        "mock"
    }

    fn model(&self) -> &str {
        "mock-asr-v1"
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn mock_asr_produces_final_transcript() {
        let mut asr = MockAsr::new(MockAsrConfig {
            latency_ms: 10,
            ..Default::default()
        });
        asr.initialize().await.unwrap();

        let (_audio_tx, mut result_rx) = asr.start_stream("en-US").await.unwrap();

        // Receive partial
        let partial = result_rx.recv().await.unwrap();
        assert!(!partial.is_final);

        // Receive final
        let final_result = result_rx.recv().await.unwrap();
        assert!(final_result.is_final);
        assert!(final_result.confidence > 0.9);
    }
}
