//! Local (on-device) adapter implementations.
//!
//! Architecture: each adapter wraps a pluggable "engine" trait.
//! Engines can be swapped without changing the adapter logic.
//!
//! ```text
//! AsrAdapter trait
//!   └── LocalAsr (adapter)
//!         └── LocalAsrEngine trait  ← plug engines here
//!               ├── SherpaAsrEngine   (sherpa-rs)
//!               ├── WhisperAsrEngine  (whisper-rs)
//!               └── StubAsrEngine     (for testing)
//!
//! TtsAdapter trait
//!   └── LocalTts (adapter)
//!         └── LocalTtsEngine trait
//!               ├── SherpaTtsEngine   (sherpa-rs)
//!               ├── PiperTtsEngine    (piper-rs)
//!               └── StubTtsEngine
//!
//! AgentAdapter trait
//!   └── OllamaAgent (adapter)  ← uses Ollama HTTP API (OpenAI-compatible)
//! ```

pub mod engine;
pub mod local_asr;
pub mod local_tts;
pub mod ollama_agent;
pub mod sherpa_asr;
pub mod sherpa_tts;
pub mod sherpa_vad;
