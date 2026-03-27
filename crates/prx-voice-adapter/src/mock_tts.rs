//! Mock TTS adapter for testing.

use crate::health::{AdapterStatus, HealthReport};
use crate::tts::{TtsAdapter, TtsChunk, TtsError, TtsSynthesisRequest};
use serde::{Deserialize, Serialize};
use tokio::sync::mpsc;

/// Configuration for the mock TTS adapter.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MockTtsConfig {
    /// Simulated first-chunk latency (ms).
    pub first_chunk_latency_ms: u64,
    /// Simulated audio duration per char (ms).
    pub ms_per_char: f64,
    /// Chunk size (ms of audio per chunk).
    pub chunk_duration_ms: u64,
    /// Whether to simulate an error.
    pub inject_error: bool,
}

impl Default for MockTtsConfig {
    fn default() -> Self {
        Self {
            first_chunk_latency_ms: 100,
            ms_per_char: 50.0,
            chunk_duration_ms: 200,
            inject_error: false,
        }
    }
}

/// Mock TTS adapter.
pub struct MockTts {
    config: MockTtsConfig,
}

impl MockTts {
    pub fn new(config: MockTtsConfig) -> Self {
        Self { config }
    }
}

#[async_trait::async_trait]
impl TtsAdapter for MockTts {
    async fn initialize(&mut self) -> Result<(), TtsError> {
        Ok(())
    }

    async fn synthesize(
        &self,
        request: TtsSynthesisRequest,
    ) -> Result<mpsc::Receiver<TtsChunk>, TtsError> {
        if self.config.inject_error {
            return Err(TtsError::SynthesisFailed {
                message: "Mock injected error".into(),
                retryable: false,
            });
        }

        let (tx, rx) = mpsc::channel(32);
        let config = self.config.clone();

        tokio::spawn(async move {
            // First chunk latency
            tokio::time::sleep(tokio::time::Duration::from_millis(
                config.first_chunk_latency_ms,
            ))
            .await;

            let total_duration_ms = (request.text.len() as f64 * config.ms_per_char) as u64;
            let num_chunks = (total_duration_ms / config.chunk_duration_ms).max(1);

            for i in 0..num_chunks {
                let is_final = i == num_chunks - 1;
                let chunk_len = if is_final {
                    total_duration_ms - (i * config.chunk_duration_ms)
                } else {
                    config.chunk_duration_ms
                };

                let _ = tx
                    .send(TtsChunk {
                        segment_id: request.segment_id.clone(),
                        chunk_index: i as u32,
                        audio_data: vec![0u8; (chunk_len * 16) as usize], // fake PCM
                        audio_length_ms: chunk_len,
                        encoding: request.encoding.clone(),
                        sample_rate: request.sample_rate,
                        is_final,
                        synthesis_latency_ms: if i == 0 {
                            config.first_chunk_latency_ms
                        } else {
                            5
                        },
                    })
                    .await;

                if !is_final {
                    tokio::time::sleep(tokio::time::Duration::from_millis(10)).await;
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
            status: if self.config.inject_error {
                AdapterStatus::Down
            } else {
                AdapterStatus::Ready
            },
            latency_ms: Some(self.config.first_chunk_latency_ms),
            error_rate_pct: Some(0.0),
            message: None,
        }
    }

    fn provider(&self) -> &str {
        "mock"
    }

    fn voice(&self) -> &str {
        "mock-voice-v1"
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn mock_tts_produces_audio_chunks() {
        let mut tts = MockTts::new(MockTtsConfig {
            first_chunk_latency_ms: 10,
            ms_per_char: 10.0,
            chunk_duration_ms: 50,
            inject_error: false,
        });
        tts.initialize().await.unwrap();

        let req = TtsSynthesisRequest {
            segment_id: "seg-1.0".into(),
            text: "Hello world testing".into(), // 19 chars -> 190ms -> ~4 chunks
            voice: "test-voice".into(),
            language: "en-US".into(),
            speech_rate: None,
            encoding: "pcm".into(),
            sample_rate: 16000,
        };

        let mut rx = tts.synthesize(req).await.unwrap();

        let mut chunks = vec![];
        while let Some(chunk) = rx.recv().await {
            chunks.push(chunk);
        }

        assert!(!chunks.is_empty());
        assert!(chunks.last().unwrap().is_final);
        // All chunks should have the same segment_id
        assert!(chunks.iter().all(|c| c.segment_id == "seg-1.0"));
    }
}
