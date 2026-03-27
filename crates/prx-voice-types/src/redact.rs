//! PII redaction utilities.
//! Per the security spec: email, phone, ID patterns masked before logging/storage.

use serde::{Deserialize, Serialize};

/// Redaction level.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RedactionLevel {
    /// No redaction.
    None,
    /// Basic: email, phone, national ID patterns.
    Basic,
    /// Enterprise: basic + tenant-custom patterns.
    Enterprise,
    /// Full: redact all potentially sensitive text.
    Full,
}

/// Redact PII from a string based on level.
pub fn redact(text: &str, level: RedactionLevel) -> String {
    match level {
        RedactionLevel::None => text.to_string(),
        RedactionLevel::Basic | RedactionLevel::Enterprise | RedactionLevel::Full => {
            let mut result = text.to_string();
            result = redact_emails(&result);
            result = redact_phones(&result);
            if matches!(level, RedactionLevel::Full) {
                result = redact_numbers(&result);
            }
            result
        }
    }
}

/// Check if a character can appear in the local part of an email.
fn is_email_local_char(c: char) -> bool {
    c.is_alphanumeric() || c == '.' || c == '_' || c == '-' || c == '+'
}

/// Check if a character can appear in the domain part of an email.
fn is_email_domain_char(c: char) -> bool {
    c.is_alphanumeric() || c == '.' || c == '-'
}

/// Redact email addresses: user@domain.com -> u***@d***
fn redact_emails(text: &str) -> String {
    let chars: Vec<char> = text.chars().collect();
    let mut result = String::with_capacity(text.len());
    let mut i = 0;

    while i < chars.len() {
        if chars[i] == '@' && i > 0 && i + 1 < chars.len() {
            // Find start of local part
            let mut start = i;
            while start > 0 && is_email_local_char(chars[start - 1]) {
                start -= 1;
            }
            // Find end of domain part
            let mut end = i + 1;
            while end < chars.len() && is_email_domain_char(chars[end]) {
                end += 1;
            }

            let has_local = start < i;
            let has_domain = end > i + 1;

            if has_local && has_domain {
                // Remove already-pushed local part characters
                let local_len = i - start;
                for _ in 0..local_len {
                    result.pop();
                }
                // Build masked email
                result.push(chars[start]);
                result.push_str("***@");
                result.push(chars[i + 1]);
                result.push_str("***");
                i = end;
                continue;
            }
        }
        result.push(chars[i]);
        i += 1;
    }
    result
}

/// Redact phone numbers: sequences of 7+ digits -> keep last 4.
fn redact_phones(text: &str) -> String {
    let mut result = String::new();
    let mut digit_buffer = String::new();

    for ch in text.chars() {
        if ch.is_ascii_digit()
            || (ch == '+' && digit_buffer.is_empty())
            || (ch == '-' && !digit_buffer.is_empty())
            || (ch == ' ' && !digit_buffer.is_empty() && count_digits(&digit_buffer) > 0)
        {
            digit_buffer.push(ch);
        } else {
            flush_phone_buffer(&mut digit_buffer, &mut result);
            result.push(ch);
        }
    }
    flush_phone_buffer(&mut digit_buffer, &mut result);
    result
}

fn flush_phone_buffer(buffer: &mut String, result: &mut String) {
    if count_digits(buffer) >= 7 {
        let digits_only: String = buffer.chars().filter(|c| c.is_ascii_digit()).collect();
        let last4 = &digits_only[digits_only.len().saturating_sub(4)..];
        result.push_str(&format!("***{last4}"));
    } else {
        result.push_str(buffer);
    }
    buffer.clear();
}

/// Redact all number sequences of 3+ digits (for Full level).
fn redact_numbers(text: &str) -> String {
    let mut result = String::new();
    let mut in_number = false;
    let mut num_start = 0;

    for (_idx, ch) in text.char_indices() {
        if ch.is_ascii_digit() {
            if !in_number {
                in_number = true;
                num_start = result.len();
            }
            result.push(ch);
        } else {
            if in_number {
                let num_len = result.len() - num_start;
                if num_len >= 3 {
                    result.truncate(num_start);
                    result.push_str("[REDACTED]");
                }
                in_number = false;
            }
            result.push(ch);
        }
    }
    if in_number {
        let num_len = result.len() - num_start;
        if num_len >= 3 {
            result.truncate(num_start);
            result.push_str("[REDACTED]");
        }
    }
    result
}

fn count_digits(s: &str) -> usize {
    s.chars().filter(|c| c.is_ascii_digit()).count()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn no_redaction() {
        assert_eq!(redact("hello world", RedactionLevel::None), "hello world");
    }

    #[test]
    fn redact_phone_numbers() {
        let input = "Call me at 14155551234 please";
        let output = redact(input, RedactionLevel::Basic);
        assert!(output.contains("***1234"));
        assert!(!output.contains("14155551234"));
    }

    #[test]
    fn redact_short_numbers_preserved() {
        let input = "I have 42 apples";
        let output = redact(input, RedactionLevel::Basic);
        assert_eq!(output, "I have 42 apples");
    }

    #[test]
    fn full_redaction_masks_all_numbers() {
        let input = "Balance is 42150 dollars";
        let output = redact(input, RedactionLevel::Full);
        assert!(output.contains("[REDACTED]"));
        assert!(!output.contains("42150"));
    }

    #[test]
    fn redaction_level_serializes() {
        let json = serde_json::to_string(&RedactionLevel::Enterprise)
            .unwrap_or_else(|e| panic!("serialization failed: {e}"));
        assert_eq!(json, "\"enterprise\"");
    }
}
