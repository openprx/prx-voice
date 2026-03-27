//! Pluggable engine traits for local ASR/TTS.
//! Implementations wrap specific libraries (sherpa-rs, whisper-rs, piper-rs, etc.)

use serde::{Deserialize, Serialize};

// ── ASR Engine ──

/// Audio chunk to feed into the ASR engine.
#[derive(Debug, Clone)]
pub struct AsrAudioInput {
    pub pcm_data: Vec<i16>,
    pub sample_rate: u32,
}

/// Result from the ASR engine.
#[derive(Debug, Clone)]
pub struct AsrEngineResult {
    pub text: String,
    pub is_final: bool,
    pub confidence: f64,
    pub language: String,
    pub latency_ms: u64,
}

/// Error from a local ASR engine.
#[derive(Debug, thiserror::Error)]
pub enum AsrEngineError {
    #[error("Engine init failed: {0}")]
    InitFailed(String),
    #[error("Processing failed: {0}")]
    ProcessingFailed(String),
    #[error("Model not found: {0}")]
    ModelNotFound(String),
}

/// Local ASR engine trait — implement this for each library.
#[async_trait::async_trait]
pub trait LocalAsrEngine: Send + Sync {
    /// Initialize the engine with a model.
    async fn init(&mut self, config: &AsrEngineConfig) -> Result<(), AsrEngineError>;
    /// Feed audio and get results. Called repeatedly with streaming chunks.
    fn process_audio(
        &mut self,
        input: &AsrAudioInput,
    ) -> Result<Option<AsrEngineResult>, AsrEngineError>;
    /// Signal end of utterance, get final result.
    fn finalize(&mut self) -> Result<Option<AsrEngineResult>, AsrEngineError>;
    /// Reset state for next utterance.
    fn reset(&mut self);
    /// Engine name (for logging/metrics).
    fn name(&self) -> &str;
}

/// ASR engine configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AsrEngineConfig {
    /// Engine type: "sherpa", "whisper", "stub"
    pub engine: String,
    /// Path to model files.
    pub model_path: Option<String>,
    /// Language code.
    pub language: String,
    /// Sample rate expected by the engine.
    pub sample_rate: u32,
    /// Enable partial/streaming results.
    pub streaming: bool,
}

impl Default for AsrEngineConfig {
    fn default() -> Self {
        Self {
            engine: "stub".into(),
            model_path: None,
            language: "en".into(),
            sample_rate: 16000,
            streaming: true,
        }
    }
}

// ── TTS Engine ──

/// Audio output from the TTS engine.
#[derive(Debug, Clone)]
pub struct TtsAudioOutput {
    pub pcm_data: Vec<i16>,
    pub sample_rate: u32,
    pub is_final: bool,
    pub latency_ms: u64,
}

/// Error from a local TTS engine.
#[derive(Debug, thiserror::Error)]
pub enum TtsEngineError {
    #[error("Engine init failed: {0}")]
    InitFailed(String),
    #[error("Synthesis failed: {0}")]
    SynthesisFailed(String),
    #[error("Voice not found: {0}")]
    VoiceNotFound(String),
}

/// Local TTS engine trait — implement for each library.
#[async_trait::async_trait]
pub trait LocalTtsEngine: Send + Sync {
    /// Initialize with model/voice.
    async fn init(&mut self, config: &TtsEngineConfig) -> Result<(), TtsEngineError>;
    /// Synthesize text to audio chunks. Returns chunks incrementally.
    fn synthesize(&mut self, text: &str) -> Result<Vec<TtsAudioOutput>, TtsEngineError>;
    /// Cancel in-flight synthesis.
    fn cancel(&mut self);
    /// Engine name.
    fn name(&self) -> &str;
}

/// TTS engine configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TtsEngineConfig {
    /// Engine type: "sherpa", "piper", "stub"
    pub engine: String,
    /// Path to model files.
    pub model_path: Option<String>,
    /// Voice name.
    pub voice: String,
    /// Language code.
    pub language: String,
    /// Output sample rate.
    pub sample_rate: u32,
    /// Speaking speed (1.0 = normal).
    pub speed: f32,
}

impl Default for TtsEngineConfig {
    fn default() -> Self {
        Self {
            engine: "stub".into(),
            model_path: None,
            voice: "default".into(),
            language: "en".into(),
            sample_rate: 16000,
            speed: 1.0,
        }
    }
}

// ── Stub Engines (for testing without real models) ──

/// Stub ASR engine — returns a fixed transcript.
pub struct StubAsrEngine {
    transcript: String,
    initialized: bool,
}

impl StubAsrEngine {
    pub fn new(transcript: impl Into<String>) -> Self {
        Self {
            transcript: transcript.into(),
            initialized: false,
        }
    }
}

#[async_trait::async_trait]
impl LocalAsrEngine for StubAsrEngine {
    async fn init(&mut self, _config: &AsrEngineConfig) -> Result<(), AsrEngineError> {
        self.initialized = true;
        Ok(())
    }

    fn process_audio(
        &mut self,
        _input: &AsrAudioInput,
    ) -> Result<Option<AsrEngineResult>, AsrEngineError> {
        // Return partial on each chunk
        Ok(Some(AsrEngineResult {
            text: self.transcript[..self.transcript.len().min(10)].to_string(),
            is_final: false,
            confidence: 0.7,
            language: "en".into(),
            latency_ms: 50,
        }))
    }

    fn finalize(&mut self) -> Result<Option<AsrEngineResult>, AsrEngineError> {
        Ok(Some(AsrEngineResult {
            text: self.transcript.clone(),
            is_final: true,
            confidence: 0.95,
            language: "en".into(),
            latency_ms: 100,
        }))
    }

    fn reset(&mut self) {}
    fn name(&self) -> &str {
        "stub"
    }
}

// ── HTTP ASR Engine (calls local Python ASR server) ──

/// HTTP-based ASR engine that sends audio to a local ASR server (e.g., models/asr_server.py).
/// Used when the sherpa native feature cannot be compiled.
pub struct HttpAsrEngine {
    endpoint: String,
    buffer: Vec<i16>,
    initialized: bool,
}

impl HttpAsrEngine {
    pub fn new(endpoint: impl Into<String>) -> Self {
        Self {
            endpoint: endpoint.into(),
            buffer: Vec::new(),
            initialized: false,
        }
    }
}

impl Default for HttpAsrEngine {
    fn default() -> Self {
        Self::new("http://localhost:8765")
    }
}

#[async_trait::async_trait]
impl LocalAsrEngine for HttpAsrEngine {
    async fn init(&mut self, _config: &AsrEngineConfig) -> Result<(), AsrEngineError> {
        // Check if server is reachable
        let health_url = format!("{}/health", self.endpoint);
        match reqwest::Client::new()
            .get(&health_url)
            .timeout(std::time::Duration::from_secs(2))
            .send()
            .await
        {
            Ok(r) if r.status().is_success() => {
                self.initialized = true;
                tracing::info!(endpoint = %self.endpoint, "HTTP ASR engine connected");
                Ok(())
            }
            Ok(r) => Err(AsrEngineError::InitFailed(format!(
                "ASR server returned {}",
                r.status()
            ))),
            Err(e) => Err(AsrEngineError::InitFailed(format!(
                "ASR server at {} not reachable: {e}. Start it with: python3 models/asr_server.py",
                self.endpoint
            ))),
        }
    }

    fn process_audio(
        &mut self,
        input: &AsrAudioInput,
    ) -> Result<Option<AsrEngineResult>, AsrEngineError> {
        // Accumulate audio for batch send on finalize
        self.buffer.extend_from_slice(&input.pcm_data);
        Ok(None)
    }

    fn finalize(&mut self) -> Result<Option<AsrEngineResult>, AsrEngineError> {
        // We need to send HTTP request which is async, but finalize is sync.
        // Store the buffer and let the caller handle it via finalize_async.
        // For compatibility, return None here — the real work is in finalize_async.
        Ok(None)
    }

    fn reset(&mut self) {
        self.buffer.clear();
    }

    fn name(&self) -> &str {
        "http-asr"
    }
}

impl HttpAsrEngine {
    /// Async finalize — sends accumulated audio to the HTTP ASR server and returns the result.
    pub async fn finalize_async(&mut self) -> Result<Option<AsrEngineResult>, AsrEngineError> {
        if self.buffer.is_empty() {
            return Ok(None);
        }

        // Convert i16 PCM to bytes (little-endian)
        let audio_bytes: Vec<u8> = self
            .buffer
            .iter()
            .flat_map(|&s| s.to_le_bytes())
            .collect();
        self.buffer.clear();

        let url = format!("{}/asr", self.endpoint);
        let client = reqwest::Client::new();
        let resp = client
            .post(&url)
            .header("Content-Type", "application/octet-stream")
            .body(audio_bytes)
            .timeout(std::time::Duration::from_secs(10))
            .send()
            .await
            .map_err(|e| AsrEngineError::ProcessingFailed(format!("HTTP ASR request failed: {e}")))?;

        if !resp.status().is_success() {
            return Err(AsrEngineError::ProcessingFailed(format!(
                "ASR server returned {}",
                resp.status()
            )));
        }

        #[derive(serde::Deserialize)]
        struct AsrResponse {
            text: String,
        }

        let body: AsrResponse = resp
            .json()
            .await
            .map_err(|e| AsrEngineError::ProcessingFailed(format!("Invalid ASR response: {e}")))?;

        if body.text.is_empty() {
            return Ok(None);
        }

        Ok(Some(AsrEngineResult {
            text: body.text,
            is_final: true,
            confidence: 0.9,
            language: "zh-CN".into(),
            latency_ms: 200,
        }))
    }
}

/// Stub TTS engine — returns silence with correct duration.
pub struct StubTtsEngine {
    initialized: bool,
}

impl StubTtsEngine {
    pub fn new() -> Self {
        Self { initialized: false }
    }
}

impl Default for StubTtsEngine {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait::async_trait]
impl LocalTtsEngine for StubTtsEngine {
    async fn init(&mut self, _config: &TtsEngineConfig) -> Result<(), TtsEngineError> {
        self.initialized = true;
        Ok(())
    }

    fn synthesize(&mut self, text: &str) -> Result<Vec<TtsAudioOutput>, TtsEngineError> {
        // Generate ~100ms of silence per word
        let word_count = text.split_whitespace().count().max(1);
        let samples_per_word = 1600; // 100ms at 16kHz
        let total_samples = word_count * samples_per_word;

        Ok(vec![TtsAudioOutput {
            pcm_data: vec![0i16; total_samples],
            sample_rate: 16000,
            is_final: true,
            latency_ms: 50,
        }])
    }

    fn cancel(&mut self) {}
    fn name(&self) -> &str {
        "stub"
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn stub_asr_engine_works() {
        let mut engine = StubAsrEngine::new("Hello world");
        engine.init(&AsrEngineConfig::default()).await.unwrap();

        let input = AsrAudioInput {
            pcm_data: vec![0i16; 160],
            sample_rate: 16000,
        };
        let partial = engine.process_audio(&input).unwrap().unwrap();
        assert!(!partial.is_final);

        let final_result = engine.finalize().unwrap().unwrap();
        assert!(final_result.is_final);
        assert_eq!(final_result.text, "Hello world");
    }

    #[tokio::test]
    async fn stub_tts_engine_works() {
        let mut engine = StubTtsEngine::new();
        engine.init(&TtsEngineConfig::default()).await.unwrap();

        let chunks = engine.synthesize("Hello world test").unwrap();
        assert_eq!(chunks.len(), 1);
        assert!(chunks[0].is_final);
        assert!(!chunks[0].pcm_data.is_empty());
    }
}
