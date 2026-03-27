//! Unified configuration system.
//!
//! Priority: defaults → config file → environment variables.
//! Env vars use PRX_VOICE_ prefix (e.g., PRX_VOICE_SERVER__PORT=3001).

use serde::{Deserialize, Serialize};

/// Top-level application settings.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppSettings {
    pub server: ServerSettings,
    pub session: SessionSettings,
    pub adapters: AdapterSettings,
    pub logging: LoggingSettings,
    pub limits: LimitSettings,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ServerSettings {
    /// Listen host.
    pub host: String,
    /// Listen port.
    pub port: u16,
    /// Enable CORS.
    pub cors_enabled: bool,
    /// Graceful shutdown timeout (seconds).
    pub shutdown_timeout_sec: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionSettings {
    /// Default max session duration (seconds).
    pub max_duration_sec: u64,
    /// Default max turns per session.
    pub max_turns: u32,
    /// Enable interrupt by default.
    pub interrupt_enabled: bool,
    /// Default language.
    pub default_language: String,
    /// Default VAD sensitivity (0.0-1.0).
    pub vad_sensitivity: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AdapterSettings {
    /// Default ASR provider.
    pub default_asr_provider: String,
    /// Default Agent provider.
    pub default_agent_provider: String,
    /// Default TTS provider.
    pub default_tts_provider: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LoggingSettings {
    /// Log level (trace, debug, info, warn, error).
    pub level: String,
    /// Output format: "json" or "pretty".
    pub format: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LimitSettings {
    /// Default max concurrent sessions per tenant.
    pub default_max_concurrent_sessions: u32,
    /// Event bus capacity.
    pub event_bus_capacity: usize,
    /// API rate limit (requests per minute).
    pub api_rate_limit_per_min: u64,
}

impl Default for AppSettings {
    fn default() -> Self {
        Self {
            server: ServerSettings {
                host: "0.0.0.0".into(),
                port: 3000,
                cors_enabled: true,
                shutdown_timeout_sec: 30,
            },
            session: SessionSettings {
                max_duration_sec: 1800,
                max_turns: 100,
                interrupt_enabled: true,
                default_language: "en-US".into(),
                vad_sensitivity: 0.5,
            },
            adapters: AdapterSettings {
                default_asr_provider: "mock".into(),
                default_agent_provider: "mock".into(),
                default_tts_provider: "mock".into(),
            },
            logging: LoggingSettings {
                level: "info".into(),
                format: "json".into(),
            },
            limits: LimitSettings {
                default_max_concurrent_sessions: 100,
                event_bus_capacity: 4096,
                api_rate_limit_per_min: 300,
            },
        }
    }
}

impl AppSettings {
    /// Load settings from default → config file → environment.
    /// Config file path can be overridden via PRX_VOICE_CONFIG env var.
    pub fn load() -> Result<Self, config::ConfigError> {
        let config_path =
            std::env::var("PRX_VOICE_CONFIG").unwrap_or_else(|_| "config.yaml".into());

        let settings = config::Config::builder()
            // Start with defaults
            .set_default("server.host", "0.0.0.0")?
            .set_default("server.port", 3000)?
            .set_default("server.cors_enabled", true)?
            .set_default("server.shutdown_timeout_sec", 30)?
            .set_default("session.max_duration_sec", 1800)?
            .set_default("session.max_turns", 100)?
            .set_default("session.interrupt_enabled", true)?
            .set_default("session.default_language", "en-US")?
            .set_default("session.vad_sensitivity", 0.5)?
            .set_default("adapters.default_asr_provider", "mock")?
            .set_default("adapters.default_agent_provider", "mock")?
            .set_default("adapters.default_tts_provider", "mock")?
            .set_default("logging.level", "info")?
            .set_default("logging.format", "json")?
            .set_default("limits.default_max_concurrent_sessions", 100)?
            .set_default("limits.event_bus_capacity", 4096)?
            .set_default("limits.api_rate_limit_per_min", 300)?
            // Layer config file (optional)
            .add_source(config::File::with_name(&config_path).required(false))
            // Layer environment variables (PRX_VOICE_ prefix, __ as separator)
            .add_source(config::Environment::with_prefix("PRX_VOICE").separator("__"))
            .build()?;

        settings.try_deserialize()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_settings_valid() {
        let s = AppSettings::default();
        assert_eq!(s.server.port, 3000);
        assert_eq!(s.session.max_turns, 100);
        assert_eq!(s.adapters.default_asr_provider, "mock");
        assert_eq!(s.limits.event_bus_capacity, 4096);
    }

    #[test]
    fn load_with_defaults() {
        // No config file, no env vars — should use defaults
        let s = AppSettings::load().unwrap();
        assert_eq!(s.server.host, "0.0.0.0");
        assert_eq!(s.server.port, 3000);
        assert_eq!(s.logging.level, "info");
    }

    #[test]
    fn settings_serializes() {
        let s = AppSettings::default();
        let json = serde_json::to_string_pretty(&s).unwrap();
        assert!(json.contains("\"port\": 3000"));
        assert!(json.contains("\"default_language\": \"en-US\""));
    }
}
