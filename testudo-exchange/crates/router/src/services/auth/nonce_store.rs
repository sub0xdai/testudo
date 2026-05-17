use dashmap::DashMap;
use rand::Rng;
use std::time::{Duration, Instant};

const DEFAULT_NONCE_TTL_SECS: u64 = 300; // 5 minutes
const NONCE_LENGTH: usize = 32;

/// In-memory nonce store with automatic TTL expiry.
/// Prevents SIWE replay attacks by ensuring each nonce is single-use.
pub struct NonceStore {
    nonces: DashMap<String, Instant>,
    ttl: Duration,
}

impl NonceStore {
    pub fn new() -> Self {
        Self {
            nonces: DashMap::new(),
            ttl: Duration::from_secs(DEFAULT_NONCE_TTL_SECS),
        }
    }

    /// Generate a random alphanumeric nonce and store it with TTL.
    /// Triggers cleanup of expired entries.
    pub fn generate(&self) -> String {
        self.cleanup();

        let nonce: String = rand::thread_rng()
            .sample_iter(&rand::distributions::Alphanumeric)
            .take(NONCE_LENGTH)
            .map(char::from)
            .collect();

        self.nonces.insert(nonce.clone(), Instant::now());
        nonce
    }

    /// Consume a nonce (one-time use). Returns true if the nonce was valid and not expired.
    pub fn consume(&self, nonce: &str) -> bool {
        if let Some((_, created_at)) = self.nonces.remove(nonce) {
            created_at.elapsed() < self.ttl
        } else {
            false
        }
    }

    fn cleanup(&self) {
        let ttl = self.ttl;
        self.nonces.retain(|_, created_at| created_at.elapsed() < ttl);
    }
}

impl Default for NonceStore {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_generate_returns_32_char_alphanumeric() {
        let store = NonceStore::new();
        let nonce = store.generate();
        assert_eq!(nonce.len(), 32);
        assert!(nonce.chars().all(|c| c.is_ascii_alphanumeric()));
    }

    #[test]
    fn test_generate_unique_nonces() {
        let store = NonceStore::new();
        let n1 = store.generate();
        let n2 = store.generate();
        assert_ne!(n1, n2);
    }

    #[test]
    fn test_consume_valid_nonce() {
        let store = NonceStore::new();
        let nonce = store.generate();
        assert!(store.consume(&nonce));
    }

    #[test]
    fn test_consume_removes_nonce() {
        let store = NonceStore::new();
        let nonce = store.generate();
        assert!(store.consume(&nonce));
        // Second consume should fail (one-time use)
        assert!(!store.consume(&nonce));
    }

    #[test]
    fn test_consume_unknown_nonce() {
        let store = NonceStore::new();
        assert!(!store.consume("nonexistent"));
    }

    #[test]
    fn test_expired_nonce_rejected() {
        let store = NonceStore {
            nonces: DashMap::new(),
            ttl: Duration::from_millis(1),
        };
        let nonce = store.generate();
        std::thread::sleep(Duration::from_millis(5));
        assert!(!store.consume(&nonce));
    }

    #[test]
    fn test_cleanup_removes_expired() {
        let store = NonceStore {
            nonces: DashMap::new(),
            ttl: Duration::from_millis(1),
        };
        store.nonces.insert("old".to_string(), Instant::now() - Duration::from_secs(60));
        store.generate(); // triggers cleanup
        assert!(!store.nonces.contains_key("old"));
    }
}
