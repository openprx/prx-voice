//! ASR adapter trait.

use crate::health::HealthReport;
use serde::{Deserialize, Serialize};
use thiserror::Error;
use tokio::sync::mpsc;

/// ASR transcript result (partial or final).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AsrResult {
    pub speech_id: String,
    pub transcript: String,
    pub confidence: f64,
    pub stability: f64,
    pub is_final: bool,
    pub revision: u32,
    pub language: String,
    /// ASR latency in ms (only set on final).
    pub asr_latency_ms: Option<u64>,
}

/// ASR adapter errors.
#[derive(Debug, Error)]
pub enum AsrError {
    #[error("ASR timeout after {timeout_ms}ms")]
    Timeout { timeout_ms: u64 },
    #[error("ASR provider error: {message}")]
    ProviderError { message: String, retryable: bool },
    #[error("ASR cancelled")]
    Cancelled,
    #[error("ASR internal error: {0}")]
    Internal(String),
}

/// Audio chunk to feed to ASR.
#[derive(Debug, Clone)]
pub struct AudioChunk {
    pub data: Vec<u8>,
    pub sample_rate: u32,
    pub channels: u16,
    pub timestamp_ms: u64,
}

/// ASR adapter trait — streaming speech-to-text.
#[async_trait::async_trait]
pub trait AsrAdapter: Send + Sync {
    /// Initialize the adapter.
    async fn initialize(&mut self) -> Result<(), AsrError>;

    /// Start a streaming recognition session.
    /// Returns a sender for audio chunks and a receiver for results.
    async fn start_stream(
        &self,
        language: &str,
    ) -> Result<(mpsc::Sender<AudioChunk>, mpsc::Receiver<AsrResult>), AsrError>;

    /// Cancel an in-flight recognition.
    async fn cancel(&self) -> Result<(), AsrError>;

    /// Health check.
    async fn health(&self) -> HealthReport;

    /// Warm up connections (pre-execution preparation).
    async fn warmup(&self) -> Result<(), AsrError> {
        Ok(())
    }

    /// Stop accepting new requests, finish in-flight.
    async fn drain(&self) -> Result<(), AsrError> {
        Ok(())
    }

    /// Clean up resources.
    async fn shutdown(&self) -> Result<(), AsrError> {
        Ok(())
    }

    /// Provider name.
    fn provider(&self) -> &str;

    /// Model name.
    fn model(&self) -> &str;
}
