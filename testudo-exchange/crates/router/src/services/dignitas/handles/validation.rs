//! Pure handle validation (ENG-01b — T2).
//!
//! No DB interaction. Trims, normalises to lowercase, enforces format,
//! then checks the reserved list and profanity filter.

use std::sync::OnceLock;

use regex::Regex;

use super::profanity::contains_profanity;
use super::reserved::is_reserved;

/// A handle that has passed all validation — guaranteed lowercase.
pub type NormalizedHandle = String;

#[derive(Debug, thiserror::Error, PartialEq)]
pub enum HandleValidationError {
    #[error("handle must be at least 3 characters")]
    TooShort,
    #[error("handle must be at most 24 characters")]
    TooLong,
    #[error("handle must start and end with a letter or digit, and contain only [a-z0-9_-]")]
    InvalidFormat,
    #[error("handle '{0}' is reserved")]
    Reserved(String),
    #[error("handle contains prohibited content")]
    Profanity,
}

static HANDLE_RE: OnceLock<Regex> = OnceLock::new();

fn handle_regex() -> &'static Regex {
    HANDLE_RE.get_or_init(|| {
        Regex::new(r"^[a-z0-9][a-z0-9_-]{1,22}[a-z0-9]$").expect("handle regex is valid")
    })
}

/// Normalise and validate a raw handle string.
///
/// Returns the lowercase handle on success. Errors are distinct so callers
/// (HTTP routes) can map them to the correct status code.
pub fn validate_handle(raw: &str) -> Result<NormalizedHandle, HandleValidationError> {
    let normalized = raw.trim().to_lowercase();

    if normalized.len() < 3 {
        return Err(HandleValidationError::TooShort);
    }
    if normalized.len() > 24 {
        return Err(HandleValidationError::TooLong);
    }
    if !handle_regex().is_match(&normalized) {
        return Err(HandleValidationError::InvalidFormat);
    }
    if is_reserved(&normalized) {
        return Err(HandleValidationError::Reserved(normalized));
    }
    if contains_profanity(&normalized) {
        return Err(HandleValidationError::Profanity);
    }

    Ok(normalized)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn valid_handle_returned_lowercase() {
        assert_eq!(validate_handle("TRADER01"), Ok("trader01".to_string()));
    }

    #[test]
    fn two_chars_too_short() {
        assert_eq!(validate_handle("ab"), Err(HandleValidationError::TooShort));
    }

    #[test]
    fn three_chars_accepted() {
        assert!(validate_handle("abc").is_ok());
    }

    #[test]
    fn twenty_five_chars_too_long() {
        let h = "a".repeat(25);
        assert_eq!(validate_handle(&h), Err(HandleValidationError::TooLong));
    }

    #[test]
    fn twenty_four_chars_accepted() {
        // 1 leading + 22 middle + 1 trailing = 24 chars, all alphanumeric
        let h = format!("a{}a", "b".repeat(22));
        assert!(validate_handle(&h).is_ok(), "24-char handle should be accepted");
    }

    #[test]
    fn leading_dash_rejected() {
        assert_eq!(validate_handle("-handle"), Err(HandleValidationError::InvalidFormat));
    }

    #[test]
    fn trailing_underscore_rejected() {
        assert_eq!(validate_handle("handle_"), Err(HandleValidationError::InvalidFormat));
    }

    #[test]
    fn uppercase_normalized() {
        assert_eq!(validate_handle("WHALE"), Ok("whale".to_string()));
    }

    #[test]
    fn reserved_admin_rejected() {
        assert_eq!(
            validate_handle("admin"),
            Err(HandleValidationError::Reserved("admin".to_string()))
        );
    }

    #[test]
    fn profanity_rejected() {
        assert_eq!(validate_handle("0xshit"), Err(HandleValidationError::Profanity));
    }

    #[test]
    fn underscore_and_dash_allowed_internally() {
        assert!(validate_handle("0x-whale_1").is_ok());
    }

    #[test]
    fn whitespace_trimmed_before_validation() {
        assert_eq!(validate_handle("  whale  "), Ok("whale".to_string()));
    }
}
