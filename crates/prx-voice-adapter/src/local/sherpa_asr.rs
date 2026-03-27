//! Sherpa-onnx ASR engine implementation via sherpa-rs.
//! Uses ZipFormer for streaming-style recognition.
//!
//! Requires the `sherpa` feature flag and model files on disk.
//! Models: <https://k2-fsa.github.io/sherpa/onnx/pretrained_models/index.html>

use super::engine::{
    AsrAudioInput, AsrEngineConfig, AsrEngineError, AsrEngineResult, LocalAsrEngine,
};

#[cfg(feature = "sherpa")]
use sherpa_rs::zipformer::{ZipFormer, ZipFormerConfig};

/// Sherpa ASR engine wrapping ZipFormer.
pub struct SherpaAsrEngine {
    #[cfg(feature = "sherpa")]
    recognizer: Option<ZipFormer>,
    #[cfg(not(feature = "sherpa"))]
    _phantom: (),
    buffer: Vec<f32>,
    sample_rate: u32,
    /// Accumulated text from partial decodes.
    accumulated: String,
}

impl SherpaAsrEngine {
    pub fn new() -> Self {
        Self {
            #[cfg(feature = "sherpa")]
            recognizer: None,
            #[cfg(not(feature = "sherpa"))]
            _phantom: (),
            buffer: Vec::new(),
            sample_rate: 16000,
            accumulated: String::new(),
        }
    }
}

impl Default for SherpaAsrEngine {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait::async_trait]
impl LocalAsrEngine for SherpaAsrEngine {
    async fn init(&mut self, config: &AsrEngineConfig) -> Result<(), AsrEngineError> {
        self.sample_rate = config.sample_rate;

        #[cfg(feature = "sherpa")]
        {
            let model_path = config.model_path.as_deref().ok_or_else(|| {
                AsrEngineError::ModelNotFound(
                    "Set model_path in AsrEngineConfig (e.g., \
                     './models/sherpa-onnx-streaming-zipformer-en-2023-06-21')"
                        .into(),
                )
            })?;

            let sherpa_config = ZipFormerConfig {
                encoder: format!("{model_path}/encoder-epoch-99-avg-1.onnx"),
                decoder: format!("{model_path}/decoder-epoch-99-avg-1.onnx"),
                joiner: format!("{model_path}/joiner-epoch-99-avg-1.onnx"),
                tokens: format!("{model_path}/tokens.txt"),
                ..Default::default()
            };

            let recognizer = ZipFormer::new(sherpa_config)
                .map_err(|e| AsrEngineError::InitFailed(format!("ZipFormer init: {e}")))?;

            self.recognizer = Some(recognizer);
            tracing::info!(
                model = model_path,
                "Sherpa ZipFormer ASR engine initialized"
            );
        }

        #[cfg(not(feature = "sherpa"))]
        {
            tracing::warn!("sherpa feature not enabled, using stub mode");
        }

        Ok(())
    }

    #[allow(clippy::needless_return)]
    fn process_audio(
        &mut self,
        input: &AsrAudioInput,
    ) -> Result<Option<AsrEngineResult>, AsrEngineError> {
        // Accumulate audio
        self.buffer
            .extend(input.pcm_data.iter().map(|&s| s as f32 / 32768.0));

        // Process in chunks of ~500ms (8000 samples at 16kHz)
        let chunk_size = (self.sample_rate as usize) / 2;
        if self.buffer.len() < chunk_size {
            return Ok(None);
        }

        #[cfg(feature = "sherpa")]
        {
            if let Some(ref mut recognizer) = self.recognizer {
                let chunk: Vec<f32> = self.buffer.drain(..chunk_size).collect();
                let text = recognizer.decode(self.sample_rate, chunk);
                if !text.is_empty() && text != self.accumulated {
                    self.accumulated.clone_from(&text);
                    return Ok(Some(AsrEngineResult {
                        text,
                        is_final: false,
                        confidence: 0.8,
                        language: "zh-CN".into(),
                        latency_ms: 50,
                    }));
                }
            }
        }

        #[cfg(not(feature = "sherpa"))]
        {
            // Stub: return partial after accumulating enough audio
            self.buffer.clear();
            return Ok(Some(AsrEngineResult {
                text: "(语音识别未启用)".into(),
                is_final: false,
                confidence: 0.5,
                language: "zh-CN".into(),
                latency_ms: 0,
            }));
        }

        #[cfg(feature = "sherpa")]
        Ok(None)
    }

    #[allow(clippy::needless_return)]
    fn finalize(&mut self) -> Result<Option<AsrEngineResult>, AsrEngineError> {
        #[cfg(feature = "sherpa")]
        {
            if let Some(ref mut recognizer) = self.recognizer {
                if !self.buffer.is_empty() {
                    let remaining = std::mem::take(&mut self.buffer);
                    let text = recognizer.decode(self.sample_rate, remaining);
                    if !text.is_empty() {
                        self.accumulated = text;
                    }
                }
                if !self.accumulated.is_empty() {
                    return Ok(Some(AsrEngineResult {
                        text: std::mem::take(&mut self.accumulated),
                        is_final: true,
                        confidence: 0.95,
                        language: "zh-CN".into(),
                        latency_ms: 100,
                    }));
                }
            }
        }

        #[cfg(not(feature = "sherpa"))]
        {
            self.buffer.clear();
            return Ok(Some(AsrEngineResult {
                text: "(未识别到语音)".into(),
                is_final: true,
                confidence: 0.0,
                language: "zh-CN".into(),
                latency_ms: 0,
            }));
        }

        #[cfg(feature = "sherpa")]
        Ok(None)
    }

    fn reset(&mut self) {
        self.buffer.clear();
        self.accumulated.clear();
    }

    fn name(&self) -> &str {
        "sherpa-zipformer"
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn sherpa_engine_init_without_feature() {
        let mut engine = SherpaAsrEngine::new();
        // Without sherpa feature, should still init (stub mode)
        let result = engine.init(&AsrEngineConfig::default()).await;
        // Without model_path it may warn but shouldn't crash
        assert!(result.is_ok());
    }

    #[test]
    fn sherpa_engine_finalize_stub() {
        let mut engine = SherpaAsrEngine::new();
        let result = engine.finalize();
        assert!(result.is_ok());
        let inner = result.unwrap_or(None);
        assert!(inner.is_some());
        if let Some(r) = inner {
            assert!(r.is_final);
        }
    }
}
