//! Adapter fallback chain.
//!
//! Wraps multiple adapters of the same type. On primary failure,
//! tries the next adapter in the chain. Maximum hop count enforced.

use crate::agent::{AgentAdapter, AgentContext, AgentError, AgentToken};
use crate::asr::{AsrAdapter, AsrError, AsrResult, AudioChunk};
use crate::health::{AdapterStatus, HealthReport};
use crate::tts::{TtsAdapter, TtsChunk, TtsError, TtsSynthesisRequest};
use tokio::sync::mpsc;
use tracing::{info, warn};

/// Fallback ASR adapter — tries adapters in order until one succeeds.
pub struct FallbackAsr {
    adapters: Vec<Box<dyn AsrAdapter>>,
    max_hops: usize,
}

impl FallbackAsr {
    /// Create with a list of adapters (first = primary).
    pub fn new(adapters: Vec<Box<dyn AsrAdapter>>) -> Self {
        let max_hops = adapters.len();
        Self { adapters, max_hops }
    }
}

#[async_trait::async_trait]
impl AsrAdapter for FallbackAsr {
    async fn initialize(&mut self) -> Result<(), AsrError> {
        for adapter in &mut self.adapters {
            // Initialize all adapters upfront (warm them)
            if let Err(e) = adapter.initialize().await {
                warn!(provider = adapter.provider(), error = %e, "fallback ASR adapter init failed, skipping");
            }
        }
        Ok(())
    }

    async fn start_stream(
        &self,
        language: &str,
    ) -> Result<(mpsc::Sender<AudioChunk>, mpsc::Receiver<AsrResult>), AsrError> {
        let mut last_error = None;
        for (i, adapter) in self.adapters.iter().enumerate() {
            if i >= self.max_hops {
                break;
            }
            match adapter.start_stream(language).await {
                Ok(result) => {
                    if i > 0 {
                        info!(
                            provider = adapter.provider(),
                            hop = i,
                            "ASR fallback succeeded on hop {}",
                            i
                        );
                    }
                    return Ok(result);
                }
                Err(e) => {
                    warn!(
                        provider = adapter.provider(),
                        hop = i,
                        error = %e,
                        "ASR adapter failed, trying next"
                    );
                    last_error = Some(e);
                }
            }
        }
        Err(last_error.unwrap_or(AsrError::Internal("No ASR adapters available".into())))
    }

    async fn cancel(&self) -> Result<(), AsrError> {
        // Cancel on all adapters (best effort)
        for adapter in &self.adapters {
            let _ = adapter.cancel().await;
        }
        Ok(())
    }

    async fn health(&self) -> HealthReport {
        // Report health of first healthy adapter
        for adapter in &self.adapters {
            let h = adapter.health().await;
            if h.status == AdapterStatus::Ready {
                return h;
            }
        }
        // All degraded/down
        HealthReport {
            status: AdapterStatus::Down,
            latency_ms: None,
            error_rate_pct: None,
            message: Some("All ASR fallback adapters are down".into()),
        }
    }

    fn provider(&self) -> &str {
        self.adapters
            .first()
            .map(|a| a.provider())
            .unwrap_or("none")
    }

    fn model(&self) -> &str {
        self.adapters.first().map(|a| a.model()).unwrap_or("none")
    }
}

/// Fallback Agent adapter.
pub struct FallbackAgent {
    adapters: Vec<Box<dyn AgentAdapter>>,
    max_hops: usize,
}

impl FallbackAgent {
    /// Create with a list of adapters (first = primary).
    pub fn new(adapters: Vec<Box<dyn AgentAdapter>>) -> Self {
        let max_hops = adapters.len();
        Self { adapters, max_hops }
    }
}

#[async_trait::async_trait]
impl AgentAdapter for FallbackAgent {
    async fn initialize(&mut self) -> Result<(), AgentError> {
        for adapter in &mut self.adapters {
            if let Err(e) = adapter.initialize().await {
                warn!(provider = adapter.provider(), error = %e, "fallback Agent adapter init failed, skipping");
            }
        }
        Ok(())
    }

    async fn generate(
        &self,
        transcript: &str,
        context: &AgentContext,
    ) -> Result<mpsc::Receiver<AgentToken>, AgentError> {
        let mut last_error = None;
        for (i, adapter) in self.adapters.iter().enumerate() {
            if i >= self.max_hops {
                break;
            }
            match adapter.generate(transcript, context).await {
                Ok(rx) => {
                    if i > 0 {
                        info!(
                            provider = adapter.provider(),
                            hop = i,
                            "Agent fallback succeeded"
                        );
                    }
                    return Ok(rx);
                }
                Err(e) => {
                    warn!(provider = adapter.provider(), hop = i, error = %e, "Agent adapter failed, trying next");
                    last_error = Some(e);
                }
            }
        }
        Err(last_error.unwrap_or(AgentError::Internal("No Agent adapters available".into())))
    }

    async fn cancel(&self) -> Result<(), AgentError> {
        for adapter in &self.adapters {
            let _ = adapter.cancel().await;
        }
        Ok(())
    }

    async fn health(&self) -> HealthReport {
        for adapter in &self.adapters {
            let h = adapter.health().await;
            if h.status == AdapterStatus::Ready {
                return h;
            }
        }
        HealthReport {
            status: AdapterStatus::Down,
            latency_ms: None,
            error_rate_pct: None,
            message: Some("All Agent fallback adapters are down".into()),
        }
    }

    fn provider(&self) -> &str {
        self.adapters
            .first()
            .map(|a| a.provider())
            .unwrap_or("none")
    }

    fn model(&self) -> &str {
        self.adapters.first().map(|a| a.model()).unwrap_or("none")
    }
}

/// Fallback TTS adapter.
pub struct FallbackTts {
    adapters: Vec<Box<dyn TtsAdapter>>,
    max_hops: usize,
}

impl FallbackTts {
    /// Create with a list of adapters (first = primary).
    pub fn new(adapters: Vec<Box<dyn TtsAdapter>>) -> Self {
        let max_hops = adapters.len();
        Self { adapters, max_hops }
    }
}

#[async_trait::async_trait]
impl TtsAdapter for FallbackTts {
    async fn initialize(&mut self) -> Result<(), TtsError> {
        for adapter in &mut self.adapters {
            if let Err(e) = adapter.initialize().await {
                warn!(provider = adapter.provider(), error = %e, "fallback TTS adapter init failed, skipping");
            }
        }
        Ok(())
    }

    async fn synthesize(
        &self,
        request: TtsSynthesisRequest,
    ) -> Result<mpsc::Receiver<TtsChunk>, TtsError> {
        let mut last_error = None;
        for (i, adapter) in self.adapters.iter().enumerate() {
            if i >= self.max_hops {
                break;
            }
            match adapter.synthesize(request.clone()).await {
                Ok(rx) => {
                    if i > 0 {
                        info!(
                            provider = adapter.provider(),
                            hop = i,
                            "TTS fallback succeeded"
                        );
                    }
                    return Ok(rx);
                }
                Err(e) => {
                    warn!(provider = adapter.provider(), hop = i, error = %e, "TTS adapter failed, trying next");
                    last_error = Some(e);
                }
            }
        }
        Err(last_error.unwrap_or(TtsError::Internal("No TTS adapters available".into())))
    }

    async fn cancel(&self) -> Result<(), TtsError> {
        for adapter in &self.adapters {
            let _ = adapter.cancel().await;
        }
        Ok(())
    }

    async fn health(&self) -> HealthReport {
        for adapter in &self.adapters {
            let h = adapter.health().await;
            if h.status == AdapterStatus::Ready {
                return h;
            }
        }
        HealthReport {
            status: AdapterStatus::Down,
            latency_ms: None,
            error_rate_pct: None,
            message: Some("All TTS fallback adapters are down".into()),
        }
    }

    fn provider(&self) -> &str {
        self.adapters
            .first()
            .map(|a| a.provider())
            .unwrap_or("none")
    }

    fn voice(&self) -> &str {
        self.adapters.first().map(|a| a.voice()).unwrap_or("none")
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mock_agent::{MockAgent, MockAgentConfig};
    use crate::mock_asr::{MockAsr, MockAsrConfig};
    use crate::mock_tts::{MockTts, MockTtsConfig};

    #[tokio::test]
    async fn asr_fallback_uses_primary_when_healthy() {
        let primary = Box::new(MockAsr::new(MockAsrConfig {
            latency_ms: 5,
            transcript: "from primary".into(),
            ..Default::default()
        }));
        let backup = Box::new(MockAsr::new(MockAsrConfig {
            latency_ms: 5,
            transcript: "from backup".into(),
            ..Default::default()
        }));

        let mut fb = FallbackAsr::new(vec![primary, backup]);
        fb.initialize().await.expect("BUG: init should not fail");

        let (_tx, mut rx) = fb
            .start_stream("en-US")
            .await
            .expect("BUG: start_stream should not fail");
        // Drain to final
        let mut final_text = String::new();
        while let Some(r) = rx.recv().await {
            if r.is_final {
                final_text = r.transcript;
                break;
            }
        }
        assert_eq!(final_text, "from primary");
    }

    #[tokio::test]
    async fn asr_fallback_uses_backup_when_primary_fails() {
        let primary = Box::new(MockAsr::new(MockAsrConfig {
            inject_error: true,
            ..Default::default()
        }));
        let backup = Box::new(MockAsr::new(MockAsrConfig {
            latency_ms: 5,
            transcript: "from backup".into(),
            ..Default::default()
        }));

        let mut fb = FallbackAsr::new(vec![primary, backup]);
        fb.initialize().await.expect("BUG: init should not fail");

        let (_tx, mut rx) = fb
            .start_stream("en-US")
            .await
            .expect("BUG: start_stream should not fail");
        let mut final_text = String::new();
        while let Some(r) = rx.recv().await {
            if r.is_final {
                final_text = r.transcript;
                break;
            }
        }
        assert_eq!(final_text, "from backup");
    }

    #[tokio::test]
    async fn asr_fallback_all_fail() {
        let a1 = Box::new(MockAsr::new(MockAsrConfig {
            inject_error: true,
            ..Default::default()
        }));
        let a2 = Box::new(MockAsr::new(MockAsrConfig {
            inject_error: true,
            ..Default::default()
        }));

        let mut fb = FallbackAsr::new(vec![a1, a2]);
        fb.initialize().await.expect("BUG: init should not fail");

        let result = fb.start_stream("en-US").await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn agent_fallback_on_primary_failure() {
        let primary = Box::new(MockAgent::new(MockAgentConfig {
            inject_error: true,
            ..Default::default()
        }));
        let backup = Box::new(MockAgent::new(MockAgentConfig {
            first_token_latency_ms: 5,
            response_text: "backup response".into(),
            inject_error: false,
        }));

        let mut fb = FallbackAgent::new(vec![primary, backup]);
        fb.initialize().await.expect("BUG: init should not fail");

        let ctx = crate::agent::AgentContext {
            session_id: "test".into(),
            turn_id: 1,
            language: "en-US".into(),
            system_prompt: None,
            history: vec![],
        };
        let mut rx = fb
            .generate("hello", &ctx)
            .await
            .expect("BUG: generate should not fail");
        let mut last = String::new();
        while let Some(t) = rx.recv().await {
            last = t.cumulative_text;
        }
        assert_eq!(last, "backup response");
    }

    #[tokio::test]
    async fn tts_fallback_on_primary_failure() {
        let primary = Box::new(MockTts::new(MockTtsConfig {
            inject_error: true,
            ..Default::default()
        }));
        let backup = Box::new(MockTts::new(MockTtsConfig {
            first_chunk_latency_ms: 5,
            inject_error: false,
            ..Default::default()
        }));

        let mut fb = FallbackTts::new(vec![primary, backup]);
        fb.initialize().await.expect("BUG: init should not fail");

        let req = crate::tts::TtsSynthesisRequest {
            segment_id: "seg-1.0".into(),
            text: "hello world".into(),
            voice: "test".into(),
            language: "en-US".into(),
            speech_rate: None,
            encoding: "pcm".into(),
            sample_rate: 16000,
        };
        let mut rx = fb
            .synthesize(req)
            .await
            .expect("BUG: synthesize should not fail");
        let mut chunks = vec![];
        while let Some(c) = rx.recv().await {
            chunks.push(c);
        }
        assert!(!chunks.is_empty());
        assert!(chunks.last().is_some_and(|c| c.is_final));
    }
}
