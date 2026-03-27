//! PRX Voice adapter interfaces.
//!
//! Defines the trait contracts for ASR, Agent, and TTS adapters.
//! Includes mock implementations for testing.

pub mod agent;
pub mod asr;
pub mod azure_tts;
pub mod deepgram_asr;
pub mod factory;
pub mod fallback;
pub mod health;
pub mod local;
pub mod mock_agent;
pub mod mock_asr;
pub mod mock_tts;
pub mod openai_agent;
pub mod tts;
pub mod vad;
