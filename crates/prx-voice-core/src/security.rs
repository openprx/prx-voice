//! Security configuration: encryption, secrets, media security.

use serde::{Deserialize, Serialize};

/// TLS configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TlsConfig {
    pub enabled: bool,
    pub cert_path: Option<String>,
    pub key_path: Option<String>,
    pub min_version: String,
}

impl Default for TlsConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            cert_path: None,
            key_path: None,
            min_version: "1.3".into(),
        }
    }
}

/// Encryption at rest configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EncryptionConfig {
    pub algorithm: String,
    pub key_rotation_days: u32,
    pub kms_provider: Option<String>,
}

impl Default for EncryptionConfig {
    fn default() -> Self {
        Self {
            algorithm: "AES-256-GCM".into(),
            key_rotation_days: 90,
            kms_provider: None,
        }
    }
}

/// Secrets management.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SecretsConfig {
    pub provider: SecretsProvider,
    pub refresh_interval_sec: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SecretsProvider {
    Environment,
    Vault,
    AwsSecretsManager,
    GcpSecretManager,
}

impl Default for SecretsConfig {
    fn default() -> Self {
        Self {
            provider: SecretsProvider::Environment,
            refresh_interval_sec: 3600,
        }
    }
}

/// Media security configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MediaSecurityConfig {
    pub srtp_enabled: bool,
    pub dtls_enabled: bool,
    pub recording_encryption: bool,
}

impl Default for MediaSecurityConfig {
    fn default() -> Self {
        Self {
            srtp_enabled: true,
            dtls_enabled: true,
            recording_encryption: true,
        }
    }
}

/// Complete security settings.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SecuritySettings {
    pub tls: TlsConfig,
    pub encryption_at_rest: EncryptionConfig,
    pub secrets: SecretsConfig,
    pub media: MediaSecurityConfig,
    pub ip_allowlist: Vec<String>,
    pub ip_denylist: Vec<String>,
    pub max_token_lifetime_sec: u64,
    pub require_mfa_for_admin: bool,
}

impl Default for SecuritySettings {
    fn default() -> Self {
        Self {
            tls: TlsConfig::default(),
            encryption_at_rest: EncryptionConfig::default(),
            secrets: SecretsConfig::default(),
            media: MediaSecurityConfig::default(),
            ip_allowlist: Vec::new(),
            ip_denylist: Vec::new(),
            max_token_lifetime_sec: 3600,
            require_mfa_for_admin: true,
        }
    }
}

/// Check if an IP is allowed.
pub fn check_ip_access(ip: &str, settings: &SecuritySettings) -> bool {
    if !settings.ip_denylist.is_empty() && settings.ip_denylist.contains(&ip.to_string()) {
        return false;
    }
    if !settings.ip_allowlist.is_empty() {
        return settings.ip_allowlist.contains(&ip.to_string());
    }
    true
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_security_settings() {
        let s = SecuritySettings::default();
        assert!(s.tls.enabled);
        assert_eq!(s.encryption_at_rest.algorithm, "AES-256-GCM");
        assert!(s.require_mfa_for_admin);
    }

    #[test]
    fn ip_allowlist() {
        let s = SecuritySettings {
            ip_allowlist: vec!["10.0.0.1".into()],
            ..Default::default()
        };
        assert!(check_ip_access("10.0.0.1", &s));
        assert!(!check_ip_access("10.0.0.2", &s));
    }

    #[test]
    fn ip_denylist() {
        let s = SecuritySettings {
            ip_denylist: vec!["bad.ip".into()],
            ..Default::default()
        };
        assert!(!check_ip_access("bad.ip", &s));
        assert!(check_ip_access("good.ip", &s));
    }
}
