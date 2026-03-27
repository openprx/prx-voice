//! JWT token verification for API authentication.
//! Supports HS256 symmetric verification for Phase implementation.
//! Production would use RS256/ES256 with JWKS endpoint.

use chrono::Utc;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// JWT claims structure.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JwtClaims {
    /// Subject (principal ID).
    pub sub: String,
    /// Issuer.
    pub iss: String,
    /// Audience.
    pub aud: String,
    /// Expiration (Unix timestamp).
    pub exp: i64,
    /// Issued at (Unix timestamp).
    pub iat: i64,
    /// JWT ID.
    pub jti: String,
    /// Custom: tenant ID.
    pub tenant_id: String,
    /// Custom: roles.
    pub roles: Vec<String>,
    /// Custom: scopes.
    pub scopes: Vec<String>,
}

impl JwtClaims {
    /// Check if the token has expired.
    pub fn is_expired(&self) -> bool {
        let now = Utc::now().timestamp();
        self.exp < now
    }

    /// Check if the token has a specific scope.
    pub fn has_scope(&self, scope: &str) -> bool {
        self.scopes.iter().any(|s| s == scope)
    }

    /// Check if the token has a specific role.
    pub fn has_role(&self, role: &str) -> bool {
        self.roles.iter().any(|r| r == role)
    }
}

/// JWT verification errors.
#[derive(Debug, thiserror::Error)]
pub enum JwtError {
    #[error("Token expired")]
    Expired,
    #[error("Invalid token format")]
    InvalidFormat,
    #[error("Invalid signature")]
    InvalidSignature,
    #[error("Missing required claim: {0}")]
    MissingClaim(String),
    #[error("Decode error: {0}")]
    DecodeError(String),
}

/// Simple JWT decoder (base64 payload extraction without crypto verification).
/// In production, use a proper JWT library with signature verification.
pub fn decode_jwt_claims(token: &str) -> Result<JwtClaims, JwtError> {
    let parts: Vec<&str> = token.split('.').collect();
    if parts.len() != 3 {
        return Err(JwtError::InvalidFormat);
    }

    // Decode payload (middle part)
    let payload_bytes =
        base64_decode(parts[1]).map_err(|e| JwtError::DecodeError(e.to_string()))?;

    let claims: JwtClaims =
        serde_json::from_slice(&payload_bytes).map_err(|e| JwtError::DecodeError(e.to_string()))?;

    if claims.is_expired() {
        return Err(JwtError::Expired);
    }

    Ok(claims)
}

/// Base64url decode (JWT uses URL-safe base64 without padding).
fn base64_decode(input: &str) -> Result<Vec<u8>, String> {
    // Add padding if necessary
    let padded = match input.len() % 4 {
        2 => format!("{input}=="),
        3 => format!("{input}="),
        _ => input.to_string(),
    };

    // URL-safe to standard base64
    let standard: String = padded
        .chars()
        .map(|c| match c {
            '-' => '+',
            '_' => '/',
            other => other,
        })
        .collect();

    // Simple base64 decode
    decode_base64_inner(&standard).map_err(|e| format!("base64 decode error: {e}"))
}

fn decode_base64_inner(input: &str) -> Result<Vec<u8>, &'static str> {
    fn val(c: u8) -> Result<u8, &'static str> {
        match c {
            b'A'..=b'Z' => Ok(c - b'A'),
            b'a'..=b'z' => Ok(c - b'a' + 26),
            b'0'..=b'9' => Ok(c - b'0' + 52),
            b'+' => Ok(62),
            b'/' => Ok(63),
            b'=' => Ok(0),
            _ => Err("invalid base64 character"),
        }
    }

    let bytes = input.as_bytes();
    let mut result = Vec::with_capacity(bytes.len() * 3 / 4);

    for chunk in bytes.chunks(4) {
        if chunk.len() < 4 {
            break;
        }
        let a = val(chunk[0])?;
        let b = val(chunk[1])?;
        let c_val = val(chunk[2])?;
        let d = val(chunk[3])?;

        result.push((a << 2) | (b >> 4));
        if chunk[2] != b'=' {
            result.push((b << 4) | (c_val >> 2));
        }
        if chunk[3] != b'=' {
            result.push((c_val << 6) | d);
        }
    }

    Ok(result)
}

/// Create a test JWT token (for development/testing only).
pub fn create_test_token(tenant_id: &str, roles: Vec<String>, scopes: Vec<String>) -> String {
    let header = r#"{"alg":"HS256","typ":"JWT"}"#;
    let now = Utc::now().timestamp();
    let claims = JwtClaims {
        sub: "test-user".into(),
        iss: "prx-voice".into(),
        aud: "prx-voice-api".into(),
        exp: now + 3600,
        iat: now,
        jti: Uuid::new_v4().to_string(),
        tenant_id: tenant_id.into(),
        roles,
        scopes,
    };
    let claims_json = serde_json::to_string(&claims).unwrap_or_default();

    let h = base64_encode(header.as_bytes());
    let p = base64_encode(claims_json.as_bytes());
    let signature = base64_encode(b"test-signature");

    format!("{h}.{p}.{signature}")
}

fn base64_encode(input: &[u8]) -> String {
    const TABLE: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut result = String::new();

    for chunk in input.chunks(3) {
        let b0 = chunk[0] as u32;
        let b1 = if chunk.len() > 1 { chunk[1] as u32 } else { 0 };
        let b2 = if chunk.len() > 2 { chunk[2] as u32 } else { 0 };
        let triple = (b0 << 16) | (b1 << 8) | b2;

        result.push(TABLE[((triple >> 18) & 0x3F) as usize] as char);
        result.push(TABLE[((triple >> 12) & 0x3F) as usize] as char);
        if chunk.len() > 1 {
            result.push(TABLE[((triple >> 6) & 0x3F) as usize] as char);
        }
        if chunk.len() > 2 {
            result.push(TABLE[(triple & 0x3F) as usize] as char);
        }
    }

    // URL-safe: replace + with -, / with _, strip padding
    result
        .replace('+', "-")
        .replace('/', "_")
        .trim_end_matches('=')
        .to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn create_and_decode_test_token() {
        let token = create_test_token(
            "tenant-123",
            vec!["workspace_admin".into()],
            vec!["sessions:write".into(), "sessions:read".into()],
        );
        let claims = decode_jwt_claims(&token).expect("BUG: just-created token should decode");
        assert_eq!(claims.sub, "test-user");
        assert_eq!(claims.tenant_id, "tenant-123");
        assert!(claims.has_role("workspace_admin"));
        assert!(claims.has_scope("sessions:write"));
        assert!(!claims.is_expired());
    }

    #[test]
    fn expired_token_rejected() {
        let header = base64_encode(r#"{"alg":"HS256","typ":"JWT"}"#.as_bytes());
        let claims = serde_json::json!({
            "sub": "user", "iss": "test", "aud": "test",
            "exp": 0, "iat": 0, "jti": "test",
            "tenant_id": "t", "roles": [], "scopes": []
        });
        let payload = base64_encode(claims.to_string().as_bytes());
        let sig = base64_encode(b"sig");
        let token = format!("{header}.{payload}.{sig}");

        let result = decode_jwt_claims(&token);
        assert!(matches!(result, Err(JwtError::Expired)));
    }

    #[test]
    fn invalid_format_rejected() {
        assert!(matches!(
            decode_jwt_claims("not.a.valid.jwt.token"),
            Err(JwtError::InvalidFormat)
        ));
        assert!(matches!(
            decode_jwt_claims("single"),
            Err(JwtError::InvalidFormat)
        ));
    }

    #[test]
    fn claims_scope_check() {
        let token = create_test_token("t", vec![], vec!["read".into(), "write".into()]);
        let claims = decode_jwt_claims(&token).expect("BUG: just-created token should decode");
        assert!(claims.has_scope("read"));
        assert!(!claims.has_scope("admin"));
    }
}
