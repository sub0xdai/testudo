use std::borrow::Cow;

use serde::{Deserialize, Serialize};

/// A trading symbol that can be either a compile-time constant or runtime string
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct Symbol(Cow<'static, str>);

impl Symbol {
    /// Create a compile-time constant symbol
    pub const fn from_static(s: &'static str) -> Self {
        Symbol(Cow::Borrowed(s))
    }

    /// Get the symbol as a string slice
    pub fn as_str(&self) -> &str {
        &self.0
    }

    /// Check if this is a spot symbol (starts with @)
    pub fn is_spot(&self) -> bool {
        self.0.starts_with('@')
    }

    /// Check if this is a perpetual symbol
    pub fn is_perp(&self) -> bool {
        !self.is_spot()
    }
}

// Display for nice printing
impl std::fmt::Display for Symbol {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

// AsRef for compatibility
impl AsRef<str> for Symbol {
    fn as_ref(&self) -> &str {
        self.as_str()
    }
}

// Ergonomic conversions
impl From<&'static str> for Symbol {
    fn from(s: &'static str) -> Self {
        Symbol(Cow::Borrowed(s))
    }
}

impl From<String> for Symbol {
    fn from(s: String) -> Self {
        Symbol(Cow::Owned(s))
    }
}

impl From<&String> for Symbol {
    fn from(s: &String) -> Self {
        Symbol(Cow::Owned(s.clone()))
    }
}

// Allow &Symbol to work with Into<Symbol> APIs
impl From<&Symbol> for Symbol {
    fn from(s: &Symbol) -> Self {
        s.clone()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_symbol_creation() {
        let static_sym = Symbol::from_static("BTC");
        assert_eq!(static_sym.as_str(), "BTC");
        assert!(static_sym.is_perp());
        assert!(!static_sym.is_spot());

        let owned_sym = Symbol::from("ETH".to_string());
        assert_eq!(owned_sym.as_str(), "ETH");

        let spot_sym = Symbol::from_static("@107");
        assert!(spot_sym.is_spot());
        assert!(!spot_sym.is_perp());
    }

    #[test]
    fn test_symbol_conversions() {
        // From &'static str
        let sym: Symbol = "BTC".into();
        assert_eq!(sym.as_str(), "BTC");

        // From String
        let sym: Symbol = String::from("ETH").into();
        assert_eq!(sym.as_str(), "ETH");

        // From &String
        let s = String::from("SOL");
        let sym: Symbol = (&s).into();
        assert_eq!(sym.as_str(), "SOL");
    }

    #[test]
    fn test_symbol_equality() {
        let sym1 = Symbol::from_static("BTC");
        let sym2 = Symbol::from("BTC".to_string());
        assert_eq!(sym1, sym2);
    }
}
