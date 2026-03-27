//! Local TTS adapter — wraps a LocalTtsEngine and implements TtsAdapter.

use super::engine::{LocalTtsEngine, StubTtsEngine, TtsEngineConfig};
use super::sherpa_tts::SherpaTtsEngine;
use crate::health::{AdapterStatus, HealthReport};
use crate::tts::{TtsAdapter, TtsChunk, TtsError, TtsSynthesisRequest};
use tokio::sync::mpsc;
use tracing::info;

/// Local TTS adapter that delegates to a pluggable engine.
pub struct LocalTts {
    engine: Box<dyn LocalTtsEngine>,
    config: TtsEngineConfig,
    initialized: bool,
}

impl LocalTts {
    /// Create with a specific engine implementation.
    pub fn with_engine(engine: Box<dyn LocalTtsEngine>, config: TtsEngineConfig) -> Self {
        Self {
            engine,
            config,
            initialized: false,
        }
    }

    /// Create with the default stub engine.
    pub fn stub() -> Self {
        Self::with_engine(Box::new(StubTtsEngine::new()), TtsEngineConfig::default())
    }
}

#[async_trait::async_trait]
impl TtsAdapter for LocalTts {
    async fn initialize(&mut self) -> Result<(), TtsError> {
        self.engine
            .init(&self.config)
            .await
            .map_err(|e| TtsError::Internal(e.to_string()))?;
        self.initialized = true;
        info!(engine = self.engine.name(), "Local TTS engine initialized");
        Ok(())
    }

    async fn synthesize(
        &self,
        request: TtsSynthesisRequest,
    ) -> Result<mpsc::Receiver<TtsChunk>, TtsError> {
        let (tx, rx) = mpsc::channel(32);
        let config = self.config.clone();

        // Create engine instance per synthesis based on config
        let mut synth_engine: Box<dyn LocalTtsEngine> = if config.engine == "sherpa" {
            Box::new(SherpaTtsEngine::new())
        } else {
            Box::new(StubTtsEngine::new())
        };
        if synth_engine.init(&config).await.is_err() {
            return Err(TtsError::Internal("Engine init failed".into()));
        }

        tokio::spawn(async move {
            match synth_engine.synthesize(&request.text) {
                Ok(outputs) => {
                    for (i, output) in outputs.into_iter().enumerate() {
                        // Convert i16 PCM to u8 bytes
                        let bytes: Vec<u8> = output
                            .pcm_data
                            .iter()
                            .flat_map(|&s| s.to_le_bytes())
                            .collect();

                        let _ = tx
                            .send(TtsChunk {
                                segment_id: request.segment_id.clone(),
                                chunk_index: i as u32,
                                audio_data: bytes,
                                audio_length_ms: (output.pcm_data.len() as u64 * 1000)
                                    / output.sample_rate as u64,
                                encoding: "pcm16".into(),
                                sample_rate: output.sample_rate,
                                is_final: output.is_final,
                                synthesis_latency_ms: output.latency_ms,
                            })
                            .await;
                    }
                }
                Err(e) => {
                    tracing::error!(error = %e, "Local TTS synthesis failed");
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
            message: Some(format!("engine={}", self.engine.name())),
        }
    }

    fn provider(&self) -> &str {
        "local"
    }
    fn voice(&self) -> &str {
        &self.config.voice
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn local_tts_with_stub_engine() {
        let mut tts = LocalTts::stub();
        tts.initialize().await.unwrap();

        let req = TtsSynthesisRequest {
            segment_id: "seg-1.0".into(),
            text: "Hello world testing".into(),
            voice: "default".into(),
            language: "en".into(),
            speech_rate: None,
            encoding: "pcm16".into(),
            sample_rate: 16000,
        };

        let mut rx = tts.synthesize(req).await.unwrap();
        let mut chunks = vec![];
        while let Some(c) = rx.recv().await {
            chunks.push(c);
        }
        assert!(!chunks.is_empty());
        assert!(chunks.last().unwrap().is_final);
    }
}
