use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct DiagnosticLogEntry {
    pub timestamp_unix_ms: u64,
    pub level: String,
    pub category: String,
    pub message: String,
}

pub fn is_sensitive_control_indicator(
    name: &str,
    automation_id: &str,
    class_name: &str,
    role: &str,
) -> bool {
    const SENSITIVE_KEYWORDS: &[&str] = &[
        "password",
        "passwd",
        "secret",
        "token",
        "apikey",
        "api_key",
        "pin",
        "cvv",
        "creditcard",
        "credit_card",
        "ssn",
        "socialsecurity",
        "passcode",
        "authcode",
        "privatekey",
        "private_key",
    ];

    let combined = format!("{name} {automation_id} {class_name} {role}").to_lowercase();
    for keyword in SENSITIVE_KEYWORDS {
        if combined.contains(keyword) {
            return true;
        }
    }
    false
}

pub fn sanitize_sensitive_text(input: &str) -> String {
    let mut result = input.to_string();
    result = redact_tokens(&result);
    result = redact_ssn(&result);
    result = redact_credit_cards(&result);
    result
}

fn redact_ssn(text: &str) -> String {
    let bytes = text.as_bytes();
    let len = bytes.len();
    let mut output = String::with_capacity(len);
    let mut i = 0;

    while i < len {
        // Look for 3 digits, '-', 2 digits, '-', 4 digits: total 11 chars.
        // The end offset must be a char boundary: byte slicing inside a
        // multi-byte character (e.g. an em-dash in a window title) panics.
        if i + 11 <= len && text.is_char_boundary(i + 11) {
            let slice = &text[i..i + 11];
            let chars: Vec<char> = slice.chars().collect();
            if chars.len() == 11
                && chars[0..3].iter().all(|c| c.is_ascii_digit())
                && chars[3] == '-'
                && chars[4..6].iter().all(|c| c.is_ascii_digit())
                && chars[6] == '-'
                && chars[7..11].iter().all(|c| c.is_ascii_digit())
            {
                // Check word boundaries
                let prev_ok = i == 0 || !bytes[i - 1].is_ascii_alphanumeric();
                let next_ok = i + 11 == len || !bytes[i + 11].is_ascii_alphanumeric();
                if prev_ok && next_ok {
                    output.push_str("[protected-ssn]");
                    i += 11;
                    continue;
                }
            }
        }
        output.push(text[i..].chars().next().unwrap());
        i += text[i..].chars().next().unwrap().len_utf8();
    }

    output
}

fn redact_tokens(text: &str) -> String {
    let mut output = String::with_capacity(text.len());
    let words: Vec<&str> = text
        .split_inclusive(|c: char| c.is_whitespace() || c == ',' || c == ';')
        .collect();

    for word in words {
        let trimmed = word.trim_end_matches(|c: char| c.is_whitespace() || c == ',' || c == ';');
        let trailing = &word[trimmed.len()..];

        let lower = trimmed.to_lowercase();
        let is_bearer = lower.starts_with("bearer ") && trimmed.len() > 15;
        let is_key_prefix = (trimmed.starts_with("sk-")
            || trimmed.starts_with("pk-")
            || trimmed.starts_with("ghp_")
            || trimmed.starts_with("gho_")
            || trimmed.starts_with("glpat-")
            || trimmed.starts_with("xoxb-")
            || trimmed.starts_with("xoxp-"))
            && trimmed.len() >= 20;
        let is_google_key = trimmed.starts_with("AIza") && trimmed.len() >= 35;

        if is_bearer || is_key_prefix || is_google_key {
            output.push_str("[protected-token]");
        } else {
            output.push_str(trimmed);
        }
        output.push_str(trailing);
    }

    output
}

fn redact_credit_cards(text: &str) -> String {
    let bytes = text.as_bytes();
    let len = bytes.len();
    let mut intermediate = String::with_capacity(len);
    let mut i = 0;

    // Check for space-separated 4-digit card format: XXXX XXXX XXXX XXXX (19 chars)
    while i < len {
        if i + 19 <= len && text.is_char_boundary(i + 19) {
            let slice = &text[i..i + 19];
            let chars: Vec<char> = slice.chars().collect();
            if chars.len() == 19
                && chars[0..4].iter().all(|c| c.is_ascii_digit())
                && chars[4] == ' '
                && chars[5..9].iter().all(|c| c.is_ascii_digit())
                && chars[9] == ' '
                && chars[10..14].iter().all(|c| c.is_ascii_digit())
                && chars[14] == ' '
                && chars[15..19].iter().all(|c| c.is_ascii_digit())
            {
                let prev_ok = i == 0 || !bytes[i - 1].is_ascii_alphanumeric();
                let next_ok = i + 19 == len || !bytes[i + 19].is_ascii_alphanumeric();
                if prev_ok && next_ok {
                    intermediate.push_str("[protected-card]");
                    i += 19;
                    continue;
                }
            }
        }
        if let Some(ch) = text[i..].chars().next() {
            intermediate.push(ch);
            i += ch.len_utf8();
        } else {
            break;
        }
    }

    let mut output = String::with_capacity(intermediate.len());
    let words: Vec<&str> = intermediate
        .split_inclusive(|c: char| c.is_whitespace() || c == ',' || c == ';')
        .collect();

    for word in words {
        let trimmed = word.trim_end_matches(|c: char| c.is_whitespace() || c == ',' || c == ';');
        let trailing = &word[trimmed.len()..];

        let digit_count = trimmed.chars().filter(|c| c.is_ascii_digit()).count();
        let valid_chars = trimmed.chars().all(|c| c.is_ascii_digit() || c == '-');

        if (13..=19).contains(&digit_count) && valid_chars {
            output.push_str("[protected-card]");
        } else {
            output.push_str(trimmed);
        }
        output.push_str(trailing);
    }

    output
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn redacts_ssn_correctly() {
        let input = "Employee SSN is 123-45-6789 in record.";
        let redacted = sanitize_sensitive_text(input);
        assert_eq!(redacted, "Employee SSN is [protected-ssn] in record.");
    }

    #[test]
    fn redacts_credit_cards_with_dashes_and_spaces() {
        let input1 = "Card number 4111-2222-3333-4444 charged.";
        assert_eq!(
            sanitize_sensitive_text(input1),
            "Card number [protected-card] charged."
        );

        let input2 = "Card 4111222233334444 approved.";
        assert_eq!(
            sanitize_sensitive_text(input2),
            "Card [protected-card] approved."
        );
    }

    #[test]
    fn redacts_api_tokens() {
        let input = "Bearer token sk-proj-1234567890abcdef1234567890 and key AIzaSyD98765432101234567890123456789012.";
        let redacted = sanitize_sensitive_text(input);
        assert!(redacted.contains("[protected-token]"));
        assert!(!redacted.contains("sk-proj-"));
        assert!(!redacted.contains("AIzaSyD"));
    }

    #[test]
    fn leaves_multibyte_window_titles_intact() {
        // Regression: byte-window redaction sliced inside the em-dash and panicked.
        let input = "DeskFlow Executor Test — Complete 123-45-6789 ✓ 4111 2222 3333 4444 → done";
        let redacted = sanitize_sensitive_text(input);
        assert_eq!(
            redacted,
            "DeskFlow Executor Test — Complete [protected-ssn] ✓ [protected-card] → done"
        );
    }

    #[test]
    fn identifies_sensitive_control_keywords() {
        assert!(is_sensitive_control_indicator(
            "User Password",
            "txt_pass",
            "Edit",
            "edit"
        ));
        assert!(is_sensitive_control_indicator(
            "PIN Entry",
            "",
            "Edit",
            "edit"
        ));
        assert!(is_sensitive_control_indicator(
            "CVV Code",
            "cvv_input",
            "Edit",
            "edit"
        ));
        assert!(is_sensitive_control_indicator(
            "",
            "ApiKeyField",
            "Edit",
            "edit"
        ));
        assert!(!is_sensitive_control_indicator(
            "Save As", "SaveBtn", "Button", "button"
        ));
    }

    #[test]
    fn sanitization_throughput_is_fast_on_large_inputs() {
        let large_input = "User submitted account SSN 123-45-6789 with credit card 4111 2222 3333 4444 and token sk-proj-1234567890abcdef1234567890. ".repeat(100);
        let start = std::time::Instant::now();
        let sanitized = sanitize_sensitive_text(&large_input);
        let elapsed = start.elapsed();
        assert!(sanitized.contains("[protected-ssn]"));
        assert!(sanitized.contains("[protected-card]"));
        assert!(sanitized.contains("[protected-token]"));
        assert!(
            elapsed.as_millis() < 50,
            "Elapsed: {} ms",
            elapsed.as_millis()
        );
    }
}
