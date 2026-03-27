//! Sherpa-onnx TTS engine implementation via sherpa-rs.
//! Uses VitsTts for neural text-to-speech synthesis.
//!
//! Requires the `sherpa` feature flag and model files on disk.
//! Models: <https://k2-fsa.github.io/sherpa/onnx/tts/pretrained_models/vits.html>

use super::engine::{LocalTtsEngine, TtsAudioOutput, TtsEngineConfig, TtsEngineError};

#[cfg(feature = "sherpa")]
use sherpa_rs::tts::{VitsTts, VitsTtsConfig};

/// Sherpa TTS engine wrapping VitsTts.
pub struct SherpaTtsEngine {
    #[cfg(feature = "sherpa")]
    synthesizer: Option<VitsTts>,
    #[cfg(not(feature = "sherpa"))]
    _phantom: (),
    sample_rate: u32,
}

impl SherpaTtsEngine {
    pub fn new() -> Self {
        Self {
            #[cfg(feature = "sherpa")]
            synthesizer: None,
            #[cfg(not(feature = "sherpa"))]
            _phantom: (),
            sample_rate: 22050, // VITS default
        }
    }
}

impl Default for SherpaTtsEngine {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait::async_trait]
impl LocalTtsEngine for SherpaTtsEngine {
    async fn init(&mut self, config: &TtsEngineConfig) -> Result<(), TtsEngineError> {
        self.sample_rate = config.sample_rate;

        #[cfg(feature = "sherpa")]
        {
            let model_path = config.model_path.as_deref().ok_or_else(|| {
                TtsEngineError::InitFailed(
                    "Set model_path in TtsEngineConfig (e.g., \
                     './models/vits-piper-en_US-lessac-medium')"
                        .into(),
                )
            })?;

            let vits_config = VitsTtsConfig {
                model: format!("{model_path}/theresa.onnx"),
                tokens: format!("{model_path}/tokens.txt"),
                lexicon: format!("{model_path}/lexicon.txt"),
                dict_dir: format!("{model_path}/dict"),
                ..Default::default()
            };

            let synthesizer = VitsTts::new(vits_config);

            self.synthesizer = Some(synthesizer);
            tracing::info!(model = model_path, "Sherpa VitsTts engine initialized");
        }

        #[cfg(not(feature = "sherpa"))]
        {
            tracing::warn!("sherpa feature not enabled, using stub mode for TTS");
        }

        Ok(())
    }

    #[allow(clippy::needless_return)]
    fn synthesize(&mut self, text: &str) -> Result<Vec<TtsAudioOutput>, TtsEngineError> {
        #[cfg(feature = "sherpa")]
        {
            if let Some(ref mut synth) = self.synthesizer {
                let audio = synth
                    .create(text, 0, 1.0)
                    .map_err(|e| TtsEngineError::SynthesisFailed(e.to_string()))?;

                // Convert f32 samples to i16
                let pcm: Vec<i16> = audio
                    .samples
                    .iter()
                    .map(|&s| (s * 32767.0).clamp(-32768.0, 32767.0) as i16)
                    .collect();

                // Split into ~200ms chunks
                let chunk_samples = (audio.sample_rate as usize) / 5; // 200ms
                let total_chunks = (pcm.len() + chunk_samples - 1) / chunk_samples.max(1);
                let chunks: Vec<TtsAudioOutput> = pcm
                    .chunks(chunk_samples.max(1))
                    .enumerate()
                    .map(|(i, chunk)| TtsAudioOutput {
                        pcm_data: chunk.to_vec(),
                        sample_rate: audio.sample_rate,
                        is_final: i == total_chunks - 1,
                        latency_ms: if i == 0 { 100 } else { 10 },
                    })
                    .collect();

                return Ok(chunks);
            }
        }

        // Stub mode: generate silence
        #[cfg(not(feature = "sherpa"))]
        {
            let word_count = text.split_whitespace().count().max(1);
            let samples = vec![0i16; word_count * 1600]; // 100ms per word
            return Ok(vec![TtsAudioOutput {
                pcm_data: samples,
                sample_rate: self.sample_rate,
                is_final: true,
                latency_ms: 50,
            }]);
        }

        #[cfg(feature = "sherpa")]
        Ok(vec![])
    }

    fn cancel(&mut self) {
        // VitsTts doesn't support cancel (synchronous API)
    }

    fn name(&self) -> &str {
        "sherpa-vits"
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn sherpa_tts_init_stub() {
        let mut engine = SherpaTtsEngine::new();
        let result = engine.init(&TtsEngineConfig::default()).await;
        assert!(result.is_ok());
    }

    #[test]
    fn sherpa_tts_synthesize_stub() {
        let mut engine = SherpaTtsEngine::new();
        let chunks = engine.synthesize("Hello world");
        assert!(chunks.is_ok());
        let chunks = chunks.unwrap_or_default();
        assert!(!chunks.is_empty());
        if let Some(last) = chunks.last() {
            assert!(last.is_final);
        }
    }
}
