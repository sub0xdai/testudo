//! Profanity trip-wire filter (ENG-01b).
//!
//! Not exhaustive — catches the obvious. False positives on legitimate
//! names are accepted over a weak filter (per spec risk 3).
//! Substring search on the normalised (lowercase) handle.

use std::sync::OnceLock;

static SUBSTRINGS: OnceLock<Vec<&'static str>> = OnceLock::new();

fn substrings() -> &'static Vec<&'static str> {
    SUBSTRINGS.get_or_init(|| {
        vec![
            "shit", "fuck", "cunt", "nigger", "nigga", "faggot",
            "bitch", "asshole", "retard", "whore", "slut", "rape",
            "nazi", "pedo", "pedophile", "cock", "pussy", "bastard",
            "twat", "wank",
        ]
    })
}

/// Returns `true` if `handle` (already normalised to lowercase) contains
/// a prohibited substring.
pub fn contains_profanity(handle: &str) -> bool {
    substrings().iter().any(|sub| handle.contains(sub))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn profane_substring_detected() {
        assert!(contains_profanity("0xshit"));
        assert!(contains_profanity("fucktrader"));
    }

    #[test]
    fn clean_handle_passes() {
        assert!(!contains_profanity("0xwhale"));
        assert!(!contains_profanity("disciplined-trader"));
    }
}
