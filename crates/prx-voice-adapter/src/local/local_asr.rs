//! Local ASR adapter — wraps a LocalAsrEngine and implements AsrAdapter.

use super::engine::{AsrAudioInput, AsrEngineConfig, LocalAsrEngine, StubAsrEngine};
use super::sherpa_asr::SherpaAsrEngine;
use crate::asr::{AsrAdapter, AsrError, AsrResult, AudioChunk};
use crate::health::{AdapterStatus, HealthReport};
use tokio::sync::mpsc;
use tracing::info;

/// Local ASR adapter that delegates to a pluggable engine.
pub struct LocalAsr {
    engine: Box<dyn LocalAsrEngine>,
    config: AsrEngineConfig,
    initialized: bool,
}

impl LocalAsr {
    /// Create with a specific engine implementation.
    pub fn with_engine(engine: Box<dyn LocalAsrEngine>, config: AsrEngineConfig) -> Self {
        Self {
            engine,
            config,
            initialized: false,
        }
    }

    /// Create with the default stub engine.
    pub fn stub() -> Self {
        Self::with_engine(
            Box::new(StubAsrEngine::new(
                "Hello, I would like to check my balance.",
            )),
            AsrEngineConfig::default(),
        )
    }
}

#[async_trait::async_trait]
impl AsrAdapter for LocalAsr {
    async fn initialize(&mut self) -> Result<(), AsrError> {
        self.engine
            .init(&self.config)
            .await
            .map_err(|e| AsrError::Internal(e.to_string()))?;
        self.initialized = true;
        info!(engine = self.engine.name(), "Local ASR engine initialized");
        Ok(())
    }

    async fn start_stream(
        &self,
        _language: &str,
    ) -> Result<(mpsc::Sender<AudioChunk>, mpsc::Receiver<AsrResult>), AsrError> {
        let (audio_tx, mut audio_rx) = mpsc::channel::<AudioChunk>(64);
        let (result_tx, result_rx) = mpsc::channel::<AsrResult>(32);

        // We need the engine in the spawned task. Since the engine is not Clone,
        // we create a new stub engine for the stream. In production, the engine
        // would be shared via Arc<Mutex<>> or per-stream instances.
        let config = self.config.clone();

        tokio::spawn(async move {
            // Create a fresh engine instance per stream based on config
            let mut stream_engine: Box<dyn LocalAsrEngine> = if config.engine == "sherpa" {
                Box::new(SherpaAsrEngine::new())
            } else {
                Box::new(StubAsrEngine::new("你好"))
            };
            if stream_engine.init(&config).await.is_err() {
                return;
            }

            let mut revision = 0u32;

            while let Some(chunk) = audio_rx.recv().await {
                // Convert u8 audio to i16 PCM
                let pcm: Vec<i16> = chunk
                    .data
                    .chunks_exact(2)
                    .map(|c| i16::from_le_bytes([c[0], c[1]]))
                    .collect();

                let input = AsrAudioInput {
                    pcm_data: pcm,
                    sample_rate: chunk.sample_rate,
                };

                if let Ok(Some(result)) = stream_engine.process_audio(&input) {
                    revision += 1;
                    let _ = result_tx
                        .send(AsrResult {
                            speech_id: "utt-local".into(),
                            transcript: result.text,
                            confidence: result.confidence,
                            stability: if result.is_final { 1.0 } else { 0.6 },
                            is_final: false,
                            revision,
                            language: result.language,
                            asr_latency_ms: Some(result.latency_ms),
                        })
                        .await;
                }
            }

            // Finalize
            if let Ok(Some(result)) = stream_engine.finalize() {
                revision += 1;
                let _ = result_tx
                    .send(AsrResult {
                        speech_id: "utt-local".into(),
                        transcript: result.text,
                        confidence: result.confidence,
                        stability: 1.0,
                        is_final: true,
                        revision,
                        language: result.language,
                        asr_latency_ms: Some(result.latency_ms),
                    })
                    .await;
            }
        });

        Ok((audio_tx, result_rx))
    }

    async fn cancel(&self) -> Result<(), AsrError> {
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
    fn model(&self) -> &str {
        self.engine.name()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn local_asr_with_stub_engine() {
        let mut asr = LocalAsr::stub();
        asr.initialize().await.unwrap();

        let (_tx, mut rx) = asr.start_stream("en").await.unwrap();
        drop(_tx); // Close audio channel to trigger finalize

        // Should get at least partial + final
        let mut got_final = false;
        while let Some(r) = rx.recv().await {
            if r.is_final {
                got_final = true;
                assert!(!r.transcript.is_empty());
            }
        }
        assert!(got_final);
    }
}
