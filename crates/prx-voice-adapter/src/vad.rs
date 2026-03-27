//! Voice Activity Detection (VAD) adapter trait.
//! Detects speech onset/offset in audio streams.

use crate::health::HealthReport;
use serde::{Deserialize, Serialize};
use thiserror::Error;

/// VAD detection result.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VadResult {
    /// Whether speech is currently detected.
    pub is_speech: bool,
    /// Energy level in dB.
    pub energy_db: f64,
    /// Confidence of detection (0.0-1.0).
    pub confidence: f64,
    /// Audio offset in milliseconds.
    pub audio_offset_ms: u64,
}

/// VAD errors.
#[derive(Debug, Error)]
pub enum VadError {
    #[error("VAD processing failed: {0}")]
    ProcessingFailed(String),
    #[error("VAD not initialized")]
    NotInitialized,
}

/// VAD configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VadConfig {
    /// Speech onset threshold (energy dB).
    pub onset_threshold_db: f64,
    /// Speech offset threshold (energy dB).
    pub offset_threshold_db: f64,
    /// Minimum speech duration to trigger (ms).
    pub min_speech_ms: u64,
    /// Silence duration to confirm offset (ms).
    pub silence_duration_ms: u64,
    /// VAD model name.
    pub model: String,
}

impl Default for VadConfig {
    fn default() -> Self {
        Self {
            onset_threshold_db: -35.0,
            offset_threshold_db: -45.0,
            min_speech_ms: 100,
            silence_duration_ms: 300,
            model: "energy".into(),
        }
    }
}

/// VAD adapter trait.
#[async_trait::async_trait]
pub trait VadAdapter: Send + Sync {
    /// Initialize the VAD.
    async fn initialize(&mut self) -> Result<(), VadError>;

    /// Process an audio frame and return detection result.
    fn process_frame(
        &mut self,
        audio: &[u8],
        sample_rate: u32,
        timestamp_ms: u64,
    ) -> Result<VadResult, VadError>;

    /// Reset state (e.g., between turns).
    fn reset(&mut self);

    /// Health check.
    async fn health(&self) -> HealthReport;

    /// Model name.
    fn model(&self) -> &str;
}

/// Simple energy-based VAD implementation.
pub struct EnergyVad {
    config: VadConfig,
    is_speech: bool,
    speech_start_ms: Option<u64>,
    silence_start_ms: Option<u64>,
    initialized: bool,
}

impl EnergyVad {
    pub fn new(config: VadConfig) -> Self {
        Self {
            config,
            is_speech: false,
            speech_start_ms: None,
            silence_start_ms: None,
            initialized: false,
        }
    }

    fn compute_energy_db(audio: &[u8]) -> f64 {
        if audio.len() < 2 {
            return -96.0;
        }
        let samples: Vec<i16> = audio
            .chunks_exact(2)
            .map(|chunk| i16::from_le_bytes([chunk[0], chunk[1]]))
            .collect();
        if samples.is_empty() {
            return -96.0;
        }
        let rms = (samples.iter().map(|&s| (s as f64).powi(2)).sum::<f64>() / samples.len() as f64)
            .sqrt();
        if rms < 1.0 { -96.0 } else { 20.0 * rms.log10() }
    }
}

#[async_trait::async_trait]
impl VadAdapter for EnergyVad {
    async fn initialize(&mut self) -> Result<(), VadError> {
        self.initialized = true;
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

        let energy = Self::compute_energy_db(audio);

        if !self.is_speech {
            if energy >= self.config.onset_threshold_db {
                match self.speech_start_ms {
                    Some(start) if timestamp_ms - start >= self.config.min_speech_ms => {
                        self.is_speech = true;
                        self.silence_start_ms = None;
                    }
                    None => {
                        self.speech_start_ms = Some(timestamp_ms);
                    }
                    _ => {}
                }
            } else {
                self.speech_start_ms = None;
            }
        } else if energy < self.config.offset_threshold_db {
            match self.silence_start_ms {
                Some(start) if timestamp_ms - start >= self.config.silence_duration_ms => {
                    self.is_speech = false;
                    self.speech_start_ms = None;
                    self.silence_start_ms = None;
                }
                None => {
                    self.silence_start_ms = Some(timestamp_ms);
                }
                _ => {}
            }
        } else {
            self.silence_start_ms = None;
        }

        Ok(VadResult {
            is_speech: self.is_speech,
            energy_db: energy,
            confidence: if self.is_speech { 0.9 } else { 0.1 },
            audio_offset_ms: timestamp_ms,
        })
    }

    fn reset(&mut self) {
        self.is_speech = false;
        self.speech_start_ms = None;
        self.silence_start_ms = None;
    }

    async fn health(&self) -> HealthReport {
        use crate::health::AdapterStatus;
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

    fn model(&self) -> &str {
        &self.config.model
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_audio(amplitude: i16, samples: usize) -> Vec<u8> {
        let mut data = Vec::with_capacity(samples * 2);
        for _ in 0..samples {
            data.extend_from_slice(&amplitude.to_le_bytes());
        }
        data
    }

    #[tokio::test]
    async fn energy_vad_detects_speech() {
        let mut vad = EnergyVad::new(VadConfig {
            min_speech_ms: 0, // immediate detection for test
            ..Default::default()
        });
        vad.initialize().await.unwrap();

        // Silence
        let silence = make_audio(10, 160);
        let r = vad.process_frame(&silence, 16000, 0).unwrap();
        assert!(!r.is_speech);

        // Loud audio
        let speech = make_audio(5000, 160);
        let r = vad.process_frame(&speech, 16000, 20).unwrap();
        assert!(r.is_speech);
    }

    #[tokio::test]
    async fn energy_vad_reset() {
        let mut vad = EnergyVad::new(VadConfig::default());
        vad.initialize().await.unwrap();
        vad.is_speech = true;
        vad.reset();
        assert!(!vad.is_speech);
    }

    #[test]
    fn energy_computation() {
        let silent = vec![0u8; 320]; // 160 samples of silence
        assert!(EnergyVad::compute_energy_db(&silent) < -90.0);

        let loud = make_audio(10000, 160);
        assert!(EnergyVad::compute_energy_db(&loud) > 50.0);
    }
}
