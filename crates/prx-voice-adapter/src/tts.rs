//! TTS adapter trait.

use crate::health::HealthReport;
use serde::{Deserialize, Serialize};
use thiserror::Error;
use tokio::sync::mpsc;

/// A chunk of synthesized audio.
#[derive(Debug, Clone)]
pub struct TtsChunk {
    pub segment_id: String,
    pub chunk_index: u32,
    pub audio_data: Vec<u8>,
    pub audio_length_ms: u64,
    pub encoding: String,
    pub sample_rate: u32,
    pub is_final: bool,
    pub synthesis_latency_ms: u64,
}

/// TTS adapter errors.
#[derive(Debug, Error)]
pub enum TtsError {
    #[error("TTS synthesis failed: {message}")]
    SynthesisFailed { message: String, retryable: bool },
    #[error("TTS voice unavailable: {voice}")]
    VoiceUnavailable { voice: String },
    #[error("TTS rate limited")]
    RateLimited,
    #[error("TTS cancelled")]
    Cancelled,
    #[error("TTS internal error: {0}")]
    Internal(String),
}

/// TTS synthesis request.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TtsSynthesisRequest {
    pub segment_id: String,
    pub text: String,
    pub voice: String,
    pub language: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub speech_rate: Option<f64>,
    pub encoding: String,
    pub sample_rate: u32,
}

/// TTS adapter trait — text-to-speech synthesis.
#[async_trait::async_trait]
pub trait TtsAdapter: Send + Sync {
    /// Initialize the adapter.
    async fn initialize(&mut self) -> Result<(), TtsError>;

    /// Synthesize text to audio chunks.
    async fn synthesize(
        &self,
        request: TtsSynthesisRequest,
    ) -> Result<mpsc::Receiver<TtsChunk>, TtsError>;

    /// Cancel/stop in-flight synthesis.
    async fn cancel(&self) -> Result<(), TtsError>;

    /// Health check.
    async fn health(&self) -> HealthReport;

    /// Warm up connections (pre-execution preparation).
    async fn warmup(&self) -> Result<(), TtsError> {
        Ok(())
    }

    /// Stop accepting new requests, finish in-flight.
    async fn drain(&self) -> Result<(), TtsError> {
        Ok(())
    }

    /// Clean up resources.
    async fn shutdown(&self) -> Result<(), TtsError> {
        Ok(())
    }

    /// Provider name.
    fn provider(&self) -> &str;

    /// Voice name.
    fn voice(&self) -> &str;
}
