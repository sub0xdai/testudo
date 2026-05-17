use dashmap::DashMap;
use rand::Rng;
use std::time::{Duration, Instant};
use uuid::Uuid;

// 5 minutes is the standard for human-in-the-loop OTP flows. The user must
// switch to the extension popup, enter the code, and submit — the prior 60-second
// window made any distraction (tab switch, phone notification, password manager
// prompt) cause an "expired" failure that read as the extension being broken.
// Code remains one-time-use and POST /auth/extension-pair is rate-limited
// (5 attempts / 60s / IP) — the TTL bump doesn't reduce brute-force resistance.
const DEFAULT_PAIRING_TTL_SECS: u64 = 300; // 5 minutes
const CODE_LENGTH: usize = 6;

/// In-memory pairing code store for extension device pairing.
/// Codes are 6-digit numeric, one-time use, with automatic TTL expiry.
/// Tracks recently-redeemed user IDs so the web app can poll for completion.
pub struct PairingStore {
    codes: DashMap<String, (Uuid, Instant)>,
    redeemed: DashMap<Uuid, Instant>,
    ttl: Duration,
}

impl PairingStore {
    pub fn new() -> Self {
        Self {
            codes: DashMap::new(),
            redeemed: DashMap::new(),
            ttl: Duration::from_secs(DEFAULT_PAIRING_TTL_SECS),
        }
    }

    /// Generate a 6-digit numeric pairing code for the given user.
    /// Triggers cleanup of expired entries.
    pub fn generate(&self, user_id: Uuid) -> String {
        self.cleanup();

        let mut rng = rand::thread_rng();
        let code: String = (0..CODE_LENGTH)
            .map(|_| rng.gen_range(0..10u8).to_string())
            .collect();

        self.codes.insert(code.clone(), (user_id, Instant::now()));
        code
    }

    /// Take (consume) a pairing code. Returns the user_id if valid and not expired.
    /// One-time use: the code is removed regardless.
    /// Records the user_id in `redeemed` so the web app can poll for completion.
    pub fn take(&self, code: &str) -> Option<Uuid> {
        if let Some((_, (user_id, created_at))) = self.codes.remove(code) {
            if created_at.elapsed() < self.ttl {
                self.redeemed.insert(user_id, Instant::now());
                return Some(user_id);
            }
        }
        None
    }

    /// Check whether a user was recently paired (redeemed within TTL).
    /// Used by the web app to poll for pairing completion.
    pub fn check_paired(&self, user_id: &Uuid) -> bool {
        self.redeemed
            .get(user_id)
            .is_some_and(|entry| entry.value().elapsed() < self.ttl)
    }

    fn cleanup(&self) {
        let ttl = self.ttl;
        self.codes
            .retain(|_, (_, created_at)| created_at.elapsed() < ttl);
        self.redeemed
            .retain(|_, created_at| created_at.elapsed() < ttl);
    }
}

impl Default for PairingStore {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_generate_6_digit_numeric() {
        let store = PairingStore::new();
        let user_id = Uuid::new_v4();
        let code = store.generate(user_id);
        assert_eq!(code.len(), 6);
        assert!(code.chars().all(|c| c.is_ascii_digit()));
    }

    #[test]
    fn test_take_valid_code() {
        let store = PairingStore::new();
        let user_id = Uuid::new_v4();
        let code = store.generate(user_id);
        assert_eq!(store.take(&code), Some(user_id));
    }

    #[test]
    fn test_take_consumes_code() {
        let store = PairingStore::new();
        let user_id = Uuid::new_v4();
        let code = store.generate(user_id);
        assert_eq!(store.take(&code), Some(user_id));
        // Second take fails (one-time use)
        assert_eq!(store.take(&code), None);
    }

    #[test]
    fn test_take_unknown_code() {
        let store = PairingStore::new();
        assert_eq!(store.take("000000"), None);
    }

    #[test]
    fn test_expired_code_rejected() {
        let store = PairingStore {
            codes: DashMap::new(),
            redeemed: DashMap::new(),
            ttl: Duration::from_millis(1),
        };
        let user_id = Uuid::new_v4();
        let code = store.generate(user_id);
        std::thread::sleep(Duration::from_millis(5));
        assert_eq!(store.take(&code), None);
    }

    #[test]
    fn test_cleanup_removes_expired() {
        let store = PairingStore {
            codes: DashMap::new(),
            redeemed: DashMap::new(),
            ttl: Duration::from_millis(1),
        };
        let user_id = Uuid::new_v4();
        store.codes.insert(
            "123456".to_string(),
            (user_id, Instant::now() - Duration::from_secs(60)),
        );
        store.generate(Uuid::new_v4()); // triggers cleanup
        assert!(!store.codes.contains_key("123456"));
    }

    #[test]
    fn test_multiple_users_different_codes() {
        let store = PairingStore::new();
        let u1 = Uuid::new_v4();
        let u2 = Uuid::new_v4();
        let c1 = store.generate(u1);
        let c2 = store.generate(u2);
        // Both should be retrievable
        let r1 = store.take(&c1);
        let r2 = store.take(&c2);
        assert_eq!(r1, Some(u1));
        assert_eq!(r2, Some(u2));
    }

    // --- Redeemed tracking tests ---

    #[test]
    fn test_check_paired_after_take() {
        let store = PairingStore::new();
        let user_id = Uuid::new_v4();
        let code = store.generate(user_id);
        store.take(&code);
        assert!(store.check_paired(&user_id));
    }

    #[test]
    fn test_check_paired_without_take() {
        let store = PairingStore::new();
        let user_id = Uuid::new_v4();
        store.generate(user_id);
        // Never redeemed — should be false
        assert!(!store.check_paired(&user_id));
    }

    #[test]
    fn test_check_paired_expired() {
        let store = PairingStore {
            codes: DashMap::new(),
            redeemed: DashMap::new(),
            ttl: Duration::from_millis(1),
        };
        let user_id = Uuid::new_v4();
        let code = store.generate(user_id);
        store.take(&code);
        std::thread::sleep(Duration::from_millis(5));
        assert!(!store.check_paired(&user_id));
    }

    #[test]
    fn test_cleanup_prunes_redeemed() {
        let store = PairingStore {
            codes: DashMap::new(),
            redeemed: DashMap::new(),
            ttl: Duration::from_millis(1),
        };
        let user_id = Uuid::new_v4();
        // Manually insert an expired redeemed entry
        store
            .redeemed
            .insert(user_id, Instant::now() - Duration::from_secs(60));
        // Trigger cleanup via generate
        store.generate(Uuid::new_v4());
        assert!(!store.redeemed.contains_key(&user_id));
    }
}
