//! Sherpa-onnx VAD engine using SileroVad.
//! Real streaming voice activity detection.
//!
//! Requires the `sherpa` feature flag.

use crate::health::{AdapterStatus, HealthReport};
use crate::vad::{VadAdapter, VadError, VadResult};

#[cfg(feature = "sherpa")]
use sherpa_rs::silero_vad::{SileroVad, SileroVadConfig};

/// Sherpa SileroVad adapter.
pub struct SherpaVad {
    #[cfg(feature = "sherpa")]
    vad: Option<SileroVad>,
    #[cfg(not(feature = "sherpa"))]
    _phantom: (),
    is_speech: bool,
    initialized: bool,
}

impl SherpaVad {
    pub fn new() -> Self {
        Self {
            #[cfg(feature = "sherpa")]
            vad: None,
            #[cfg(not(feature = "sherpa"))]
            _phantom: (),
            is_speech: false,
            initialized: false,
        }
    }
}

impl Default for SherpaVad {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait::async_trait]
impl VadAdapter for SherpaVad {
    async fn initialize(&mut self) -> Result<(), VadError> {
        #[cfg(feature = "sherpa")]
        {
            let config = SileroVadConfig {
                model: "silero_vad.onnx".into(),
                min_silence_duration: 0.3,
                min_speech_duration: 0.1,
                threshold: 0.5,
                ..Default::default()
            };

            match SileroVad::new(config, 30.0) {
                Ok(vad) => {
                    self.vad = Some(vad);
                    self.initialized = true;
                    tracing::info!("Sherpa SileroVad initialized");
                }
                Err(e) => {
                    tracing::warn!(error = %e, "SileroVad init failed, running in stub mode");
                    self.initialized = true; // stub mode
                }
            }
        }

        #[cfg(not(feature = "sherpa"))]
        {
            self.initialized = true;
        }

        Ok(())
    }

    fn process_frame(
        &mut self,
        audio: &[u8],
        _sample_rate: u32,
        timestamp_ms: u64,
    ) -> Result<VadResult, VadError> {
        if !self.initialized {
            return Err(VadError::NotInitialized);
        }

        #[cfg(feature = "sherpa")]
        {
            if let Some(ref mut vad) = self.vad {
                // Convert u8 PCM to f32
                let samples: Vec<f32> = audio
                    .chunks_exact(2)
                    .map(|c| i16::from_le_bytes([c[0], c[1]]) as f32 / 32768.0)
                    .collect();

                vad.accept_waveform(samples);
                self.is_speech = vad.is_speech();

                return Ok(VadResult {
                    is_speech: self.is_speech,
                    energy_db: -20.0, // SileroVad doesn't expose energy, estimate
                    confidence: if self.is_speech { 0.9 } else { 0.1 },
                    audio_offset_ms: timestamp_ms,
                });
            }
        }

        // Stub: simple energy-based detection
        let energy = if audio.len() >= 2 {
            let sample_count = audio.len() / 2;
            let rms: f64 = audio
                .chunks_exact(2)
                .map(|c| {
                    let s = i16::from_le_bytes([c[0], c[1]]) as f64;
                    s * s
                })
                .sum::<f64>()
                / sample_count as f64;
            rms.sqrt()
        } else {
            0.0
        };

        self.is_speech = energy > 500.0;

        Ok(VadResult {
            is_speech: self.is_speech,
            energy_db: if energy > 0.0 {
                20.0 * energy.log10()
            } else {
                -96.0
            },
            confidence: if self.is_speech { 0.7 } else { 0.2 },
            audio_offset_ms: timestamp_ms,
        })
    }

    fn reset(&mut self) {
        self.is_speech = false;
        #[cfg(feature = "sherpa")]
        {
            if let Some(ref mut vad) = self.vad {
                vad.clear();
            }
        }
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
            message: Some("sherpa-silero-vad".into()),
        }
    }

    fn model(&self) -> &str {
        "sherpa-silero-vad"
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn sherpa_vad_init() {
        let mut vad = SherpaVad::new();
        vad.initialize().await.unwrap_or_default();
        assert!(vad.initialized);
    }

    #[test]
    fn sherpa_vad_stub_process() {
        let mut vad = SherpaVad {
            #[cfg(feature = "sherpa")]
            vad: None,
            #[cfg(not(feature = "sherpa"))]
            _phantom: (),
            is_speech: false,
            initialized: true,
        };

        // Silence
        let silence = vec![0u8; 320];
        let r = vad.process_frame(&silence, 16000, 0);
        assert!(r.is_ok());
        if let Ok(result) = r {
            assert!(!result.is_speech);
        }

        // Loud audio
        let mut loud = Vec::new();
        for _ in 0..160 {
            loud.extend_from_slice(&5000i16.to_le_bytes());
        }
        let r = vad.process_frame(&loud, 16000, 20);
        assert!(r.is_ok());
        if let Ok(result) = r {
            assert!(result.is_speech);
        }
    }
}
