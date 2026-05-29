//! Static reserved-handle list (ENG-01b).
//!
//! Backend is the enforcement gate; the frontend mirrors this list for UX
//! only. The set is loaded once into a `OnceLock<HashSet>`.

// @anchor exchange:router:reserved
// @tags api

use std::collections::HashSet;
use std::sync::OnceLock;

static RESERVED: OnceLock<HashSet<&'static str>> = OnceLock::new();

fn reserved() -> &'static HashSet<&'static str> {
    RESERVED.get_or_init(|| {
        [
            // System / product names
            "admin", "testudo", "api", "www", "root", "support", "help",
            "mod", "team", "official", "moderator", "staff", "system",
            // High-impersonation-risk personalities (per spec risk 1)
            "cz", "sbf", "vitalik",
            // Common web paths / identifiers
            "null", "undefined", "test", "me", "you", "about", "home",
            "login", "logout", "signup", "register", "settings", "profile",
            "account", "dashboard", "d", "desk",
        ]
        .into_iter()
        .collect()
    })
}

/// Returns `true` if `handle` (already normalised to lowercase) is reserved.
pub fn is_reserved(handle: &str) -> bool {
    reserved().contains(handle)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn spec_minimum_set_reserved() {
        for word in &[
            "admin", "testudo", "api", "www", "root", "support", "help",
            "mod", "team", "official", "cz", "sbf", "vitalik",
        ] {
            assert!(is_reserved(word), "{word} must be reserved");
        }
    }

    #[test]
    fn ordinary_handle_not_reserved() {
        assert!(!is_reserved("0xwhale"));
        assert!(!is_reserved("tradingpro"));
    }
}
