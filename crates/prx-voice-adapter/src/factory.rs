//! Adapter factory — creates adapter instances based on provider names.
//!
//! This is the bridge between config/policy and actual adapter creation.
//! The API layer uses this instead of hardcoded Mock adapters.

use crate::agent::AgentAdapter;
use crate::asr::AsrAdapter;
use crate::azure_tts::{AzureTts, AzureTtsConfig};
use crate::deepgram_asr::{DeepgramAsr, DeepgramConfig};
use crate::fallback::{FallbackAgent, FallbackAsr, FallbackTts};
use crate::local::local_asr::LocalAsr;
use crate::local::local_tts::LocalTts;
use crate::local::ollama_agent::{OllamaAgent, OllamaConfig};
use crate::mock_agent::{MockAgent, MockAgentConfig};
use crate::mock_asr::{MockAsr, MockAsrConfig};
use crate::mock_tts::{MockTts, MockTtsConfig};
use crate::openai_agent::{OpenAiAgent, OpenAiConfig};
use crate::tts::TtsAdapter;
use tracing::info;

/// Create an ASR adapter by provider name.
pub fn create_asr(provider: &str) -> Box<dyn AsrAdapter> {
    match provider {
        "deepgram" => Box::new(DeepgramAsr::new(DeepgramConfig::default())),
        "sherpa" => {
            let engine = Box::new(crate::local::sherpa_asr::SherpaAsrEngine::new());
            Box::new(LocalAsr::with_engine(
                engine,
                crate::local::engine::AsrEngineConfig {
                    engine: "sherpa".into(),
                    model_path: Some(
                        "models/sherpa-onnx-streaming-zipformer-zh-14M-2023-02-23".into(),
                    ),
                    language: "zh-CN".into(),
                    sample_rate: 16000,
                    streaming: true,
                },
            ))
        }
        "local" | "whisper" => Box::new(LocalAsr::stub()),
        _ => Box::new(MockAsr::new(MockAsrConfig::default())),
    }
}

/// Create an Agent adapter by provider name.
pub fn create_agent(provider: &str) -> Box<dyn AgentAdapter> {
    match provider {
        "openai" => Box::new(OpenAiAgent::new(OpenAiConfig::default())),
        "ollama" | "local" => Box::new(OllamaAgent::new(OllamaConfig::default())),
        _ => Box::new(MockAgent::new(MockAgentConfig::default())),
    }
}

/// Create a TTS adapter by provider name.
pub fn create_tts(provider: &str) -> Box<dyn TtsAdapter> {
    match provider {
        "azure" => Box::new(AzureTts::new(AzureTtsConfig::default())),
        "sherpa" => {
            let engine = Box::new(crate::local::sherpa_tts::SherpaTtsEngine::new());
            Box::new(LocalTts::with_engine(
                engine,
                crate::local::engine::TtsEngineConfig {
                    engine: "sherpa".into(),
                    model_path: Some("models/vits-zh-hf-theresa".into()),
                    voice: "default".into(),
                    language: "zh-CN".into(),
                    sample_rate: 16000,
                    speed: 1.0,
                },
            ))
        }
        "local" | "piper" => Box::new(LocalTts::stub()),
        _ => Box::new(MockTts::new(MockTtsConfig::default())),
    }
}

/// Create an ASR adapter with fallback chain.
/// First provider in the list is primary, rest are fallbacks.
pub fn create_asr_with_fallback(providers: &[String]) -> Box<dyn AsrAdapter> {
    if providers.len() <= 1 {
        return create_asr(providers.first().map(|s| s.as_str()).unwrap_or("mock"));
    }
    let adapters: Vec<Box<dyn AsrAdapter>> = providers.iter().map(|p| create_asr(p)).collect();
    info!(providers = ?providers, "Created ASR fallback chain");
    Box::new(FallbackAsr::new(adapters))
}

/// Create an Agent adapter with fallback chain.
pub fn create_agent_with_fallback(providers: &[String]) -> Box<dyn AgentAdapter> {
    if providers.len() <= 1 {
        return create_agent(providers.first().map(|s| s.as_str()).unwrap_or("mock"));
    }
    let adapters: Vec<Box<dyn AgentAdapter>> = providers.iter().map(|p| create_agent(p)).collect();
    info!(providers = ?providers, "Created Agent fallback chain");
    Box::new(FallbackAgent::new(adapters))
}

/// Create a TTS adapter with fallback chain.
pub fn create_tts_with_fallback(providers: &[String]) -> Box<dyn TtsAdapter> {
    if providers.len() <= 1 {
        return create_tts(providers.first().map(|s| s.as_str()).unwrap_or("mock"));
    }
    let adapters: Vec<Box<dyn TtsAdapter>> = providers.iter().map(|p| create_tts(p)).collect();
    info!(providers = ?providers, "Created TTS fallback chain");
    Box::new(FallbackTts::new(adapters))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn factory_creates_mock_by_default() {
        let asr = create_asr("mock");
        assert_eq!(asr.provider(), "mock");

        let agent = create_agent("mock");
        assert_eq!(agent.provider(), "mock");

        let tts = create_tts("mock");
        assert_eq!(tts.provider(), "mock");
    }

    #[test]
    fn factory_creates_real_adapters() {
        let asr = create_asr("deepgram");
        assert_eq!(asr.provider(), "deepgram");

        let agent = create_agent("openai");
        assert_eq!(agent.provider(), "openai");

        let tts = create_tts("azure");
        assert_eq!(tts.provider(), "azure");
    }

    #[test]
    fn factory_creates_fallback_chain() {
        let asr = create_asr_with_fallback(&["deepgram".into(), "mock".into()]);
        // FallbackAsr reports primary provider
        assert_eq!(asr.provider(), "deepgram");
    }

    #[test]
    fn factory_unknown_provider_falls_back_to_mock() {
        let asr = create_asr("unknown_provider");
        assert_eq!(asr.provider(), "mock");
    }
}
