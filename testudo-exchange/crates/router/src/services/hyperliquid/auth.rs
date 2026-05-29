//! HL-02: Credential Management & EIP-712 Auth
//!
//! Hyperliquid uses Ethereum private key + EIP-712 signing instead of
//! API key + HMAC. This module manages signer construction from the
//! existing `ExchangeAccount` credential storage.
//!
//! Credential mapping (no DB migration needed):
//! - `encrypted_api_key` → Ethereum address (hex, for display/verification)
//! - `encrypted_secret` → Ethereum private key (hex)
//! - `exchange_name` → "hyperliquid"

// @anchor exchange:router:auth
// @tags api

use alloy::primitives::Address;
use alloy::signers::local::PrivateKeySigner;
use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, Instant};
use thiserror::Error;
use tokio::sync::RwLock;
use uuid::Uuid;

/// Errors from Hyperliquid authentication operations.
#[derive(Debug, Error)]
pub enum AuthError {
    #[error("Invalid private key: {0}")]
    InvalidPrivateKey(String),
    #[error("Address mismatch: stored={stored}, derived={derived}")]
    AddressMismatch { stored: String, derived: String },
    #[error("Missing wallet address for agent mode")]
    MissingWalletAddress,
}

/// Authentication mode for Hyperliquid accounts.
#[derive(Clone, Debug)]
pub enum AuthMode {
    /// Direct signing with user's main key (legacy/testing).
    Direct,
    /// Agent wallet: signer is agent key, queries use user's main address.
    Agent { user_address: Address },
}

/// Authenticated Hyperliquid signer ready for SDK use.
///
/// Wraps a `PrivateKeySigner` with the verified Ethereum address.
/// The signer implements `HyperliquidSigner` directly (the SDK provides
/// a blanket impl for `PrivateKeySigner`).
#[derive(Clone)]
pub struct HyperliquidAuth {
    pub signer: PrivateKeySigner,
    pub address: Address,
    pub auth_mode: AuthMode,
}

impl HyperliquidAuth {
    /// Construct from decrypted credentials (Direct mode).
    ///
    /// - `api_key`: Ethereum address (hex, with or without 0x prefix)
    /// - `secret`: Ethereum private key (hex, with or without 0x prefix)
    ///
    /// Derives the address from the private key and verifies it matches
    /// the stored `api_key`. This catches credential corruption early.
    pub fn from_credentials(api_key: &str, secret: &str) -> Result<Self, AuthError> {
        let signer: PrivateKeySigner = secret
            .parse()
            .map_err(|e| AuthError::InvalidPrivateKey(format!("{}", e)))?;

        let derived_address = signer.address();

        // Verify the derived address matches the stored one (if provided).
        // An empty api_key is allowed for initial setup.
        if !api_key.is_empty() {
            let stored_address: Address = api_key
                .parse()
                .map_err(|e| AuthError::InvalidPrivateKey(format!("Invalid address: {}", e)))?;

            if derived_address != stored_address {
                return Err(AuthError::AddressMismatch {
                    stored: format!("{}", stored_address),
                    derived: format!("{}", derived_address),
                });
            }
        }

        Ok(Self {
            signer,
            address: derived_address,
            auth_mode: AuthMode::Direct,
        })
    }

    /// Construct from agent wallet credentials (Agent mode).
    ///
    /// - `agent_key`: Agent private key (hex)
    /// - `wallet_address`: User's main wallet address (hex)
    ///
    /// No address-mismatch check: the signer address is the agent,
    /// not the user's main address.
    pub fn from_agent_credentials(
        agent_key: &str,
        wallet_address: &str,
    ) -> Result<Self, AuthError> {
        let signer: PrivateKeySigner = agent_key
            .parse()
            .map_err(|e| AuthError::InvalidPrivateKey(format!("{}", e)))?;
        let agent_address = signer.address();
        let user_address: Address = wallet_address
            .parse()
            .map_err(|e| AuthError::InvalidPrivateKey(format!("Invalid wallet address: {}", e)))?;

        Ok(Self {
            signer,
            address: agent_address,
            auth_mode: AuthMode::Agent { user_address },
        })
    }

    /// Returns the address to use for info queries (balance, positions, open orders).
    ///
    /// - Direct mode: returns the signer's address (same as `self.address`)
    /// - Agent mode: returns the user's main wallet address
    pub fn query_address(&self) -> Address {
        match &self.auth_mode {
            AuthMode::Direct => self.address,
            AuthMode::Agent { user_address } => *user_address,
        }
    }
}

impl std::fmt::Debug for HyperliquidAuth {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        // Never log the private key
        f.debug_struct("HyperliquidAuth")
            .field("address", &self.address)
            .field("auth_mode", &self.auth_mode)
            .finish()
    }
}

const DEFAULT_TTL_SECS: u64 = 3600; // 1 hour
const DEFAULT_MAX_CAPACITY: usize = 1000;

/// Cache entry wrapping auth with insertion timestamp for TTL eviction.
struct CacheEntry {
    auth: HyperliquidAuth,
    inserted_at: Instant,
}

impl CacheEntry {
    fn new(auth: HyperliquidAuth) -> Self {
        Self {
            auth,
            inserted_at: Instant::now(),
        }
    }

    fn is_expired(&self, ttl: Duration) -> bool {
        self.inserted_at.elapsed() > ttl
    }
}

/// Per-account signer cache with TTL, capacity bounds, and TOCTOU-safe insertion.
///
/// Keyed by `exchange_account_id` → cached `CacheEntry`.
/// Thread-safe via `RwLock` for concurrent read access.
pub struct AuthCache {
    cache: RwLock<HashMap<Uuid, CacheEntry>>,
    ttl: Duration,
    max_capacity: usize,
}

impl AuthCache {
    pub fn new() -> Self {
        Self {
            cache: RwLock::new(HashMap::new()),
            ttl: Duration::from_secs(DEFAULT_TTL_SECS),
            max_capacity: DEFAULT_MAX_CAPACITY,
        }
    }

    #[cfg(test)]
    fn with_config(ttl: Duration, max_capacity: usize) -> Self {
        Self {
            cache: RwLock::new(HashMap::new()),
            ttl,
            max_capacity,
        }
    }

    /// Get a cached signer or build one. Thread-safe: double-checks cache under
    /// write lock after construction to prevent TOCTOU races.
    async fn get_or_build<F>(
        &self,
        account_id: Uuid,
        build: F,
    ) -> Result<HyperliquidAuth, AuthError>
    where
        F: FnOnce() -> Result<HyperliquidAuth, AuthError>,
    {
        // Fast path: read lock
        {
            let cache = self.cache.read().await;
            if let Some(entry) = cache.get(&account_id) {
                if !entry.is_expired(self.ttl) {
                    return Ok(entry.auth.clone());
                }
            }
        }

        // Slow path: construct outside lock, then double-check under write lock
        let auth = build()?;
        {
            let mut cache = self.cache.write().await;
            // Double-check: another thread may have inserted while we were constructing
            if let Some(existing) = cache.get(&account_id) {
                if !existing.is_expired(self.ttl) {
                    return Ok(existing.auth.clone());
                }
            }
            cache.insert(account_id, CacheEntry::new(auth.clone()));

            // Evict oldest if over capacity
            if cache.len() > self.max_capacity {
                Self::evict_oldest(&mut cache);
            }
        }
        Ok(auth)
    }

    /// Evict the entry with the oldest `inserted_at` timestamp.
    fn evict_oldest(cache: &mut HashMap<Uuid, CacheEntry>) {
        if let Some(oldest_id) = cache
            .iter()
            .min_by_key(|(_, entry)| entry.inserted_at)
            .map(|(id, _)| *id)
        {
            cache.remove(&oldest_id);
        }
    }

    /// Get a cached signer or insert one by constructing from credentials (Direct mode).
    pub async fn get_or_insert(
        &self,
        account_id: Uuid,
        api_key: &str,
        secret: &str,
    ) -> Result<HyperliquidAuth, AuthError> {
        let api_key = api_key.to_owned();
        let secret = secret.to_owned();
        self.get_or_build(account_id, move || {
            HyperliquidAuth::from_credentials(&api_key, &secret)
        })
        .await
    }

    /// Get a cached signer or insert one by constructing from agent credentials (Agent mode).
    pub async fn get_or_insert_agent(
        &self,
        account_id: Uuid,
        agent_key: &str,
        wallet_address: &str,
    ) -> Result<HyperliquidAuth, AuthError> {
        let agent_key = agent_key.to_owned();
        let wallet_address = wallet_address.to_owned();
        self.get_or_build(account_id, move || {
            HyperliquidAuth::from_agent_credentials(&agent_key, &wallet_address)
        })
        .await
    }

    /// Invalidate a cached signer (e.g., on credential rotation).
    pub async fn invalidate(&self, account_id: &Uuid) {
        let mut cache = self.cache.write().await;
        cache.remove(account_id);
    }

    /// Number of cached entries (for diagnostics).
    pub async fn len(&self) -> usize {
        self.cache.read().await.len()
    }

    /// Whether the cache is empty.
    pub async fn is_empty(&self) -> bool {
        self.cache.read().await.is_empty()
    }
}

impl Default for AuthCache {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // Known test keypair (from hyperliquid-sdk-rs test suite)
    const TEST_PRIVATE_KEY: &str =
        "e908f86dbb4d55ac876378565aafeabc187f6690f046459397b17d9b9a19688e";

    fn test_address() -> String {
        // Derive the address from the test private key
        let signer: PrivateKeySigner = TEST_PRIVATE_KEY.parse().unwrap();
        format!("{}", signer.address())
    }

    #[test]
    fn from_credentials_valid_key() {
        let address = test_address();
        let auth = HyperliquidAuth::from_credentials(&address, TEST_PRIVATE_KEY).unwrap();
        assert_eq!(format!("{}", auth.address), address);
    }

    #[test]
    fn from_credentials_with_0x_prefix() {
        let address = test_address();
        let secret_with_prefix = format!("0x{}", TEST_PRIVATE_KEY);
        let auth =
            HyperliquidAuth::from_credentials(&address, &secret_with_prefix).unwrap();
        assert_eq!(format!("{}", auth.address), address);
    }

    #[test]
    fn from_credentials_empty_api_key_skips_verification() {
        let auth = HyperliquidAuth::from_credentials("", TEST_PRIVATE_KEY).unwrap();
        // Should succeed — address derived from key, no verification needed
        assert!(!auth.address.is_zero());
    }

    #[test]
    fn from_credentials_address_mismatch() {
        let wrong_address = "0x0000000000000000000000000000000000000001";
        let err = HyperliquidAuth::from_credentials(wrong_address, TEST_PRIVATE_KEY).unwrap_err();
        assert!(matches!(err, AuthError::AddressMismatch { .. }));
    }

    #[test]
    fn from_credentials_invalid_key() {
        let err = HyperliquidAuth::from_credentials("", "not_a_valid_key").unwrap_err();
        assert!(matches!(err, AuthError::InvalidPrivateKey(_)));
    }

    #[test]
    fn debug_does_not_leak_private_key() {
        let address = test_address();
        let auth = HyperliquidAuth::from_credentials(&address, TEST_PRIVATE_KEY).unwrap();
        let debug_output = format!("{:?}", auth);
        assert!(!debug_output.contains(TEST_PRIVATE_KEY));
        assert!(debug_output.contains("HyperliquidAuth"));
    }

    #[tokio::test]
    async fn cache_get_or_insert() {
        let cache = AuthCache::new();
        let account_id = Uuid::new_v4();
        let address = test_address();

        // First call: constructs and caches
        let auth1 = cache
            .get_or_insert(account_id, &address, TEST_PRIVATE_KEY)
            .await
            .unwrap();
        assert_eq!(cache.len().await, 1);

        // Second call: returns cached
        let auth2 = cache
            .get_or_insert(account_id, &address, TEST_PRIVATE_KEY)
            .await
            .unwrap();
        assert_eq!(auth1.address, auth2.address);
        assert_eq!(cache.len().await, 1);
    }

    #[tokio::test]
    async fn cache_invalidate() {
        let cache = AuthCache::new();
        let account_id = Uuid::new_v4();
        let address = test_address();

        cache
            .get_or_insert(account_id, &address, TEST_PRIVATE_KEY)
            .await
            .unwrap();
        assert_eq!(cache.len().await, 1);

        cache.invalidate(&account_id).await;
        assert_eq!(cache.len().await, 0);
    }

    // ==================== Agent Mode Tests ====================

    const TEST_WALLET_ADDRESS: &str = "0x0000000000000000000000000000000000001234";

    #[test]
    fn from_agent_credentials_valid() {
        let auth =
            HyperliquidAuth::from_agent_credentials(TEST_PRIVATE_KEY, TEST_WALLET_ADDRESS)
                .unwrap();
        // Signer address is the agent's derived address (not the wallet address)
        assert!(!auth.address.is_zero());
        assert!(matches!(auth.auth_mode, AuthMode::Agent { .. }));
    }

    #[test]
    fn from_agent_credentials_no_address_mismatch_check() {
        // Agent mode should NOT check that signer address matches wallet_address
        // (they're deliberately different: agent key vs user's wallet)
        let auth =
            HyperliquidAuth::from_agent_credentials(TEST_PRIVATE_KEY, TEST_WALLET_ADDRESS)
                .unwrap();
        let wallet: Address = TEST_WALLET_ADDRESS.parse().unwrap();
        assert_ne!(auth.address, wallet);
    }

    #[test]
    fn query_address_direct_returns_signer() {
        let address = test_address();
        let auth = HyperliquidAuth::from_credentials(&address, TEST_PRIVATE_KEY).unwrap();
        assert_eq!(format!("{}", auth.query_address()), address);
        assert_eq!(auth.query_address(), auth.address);
    }

    #[test]
    fn query_address_agent_returns_user_wallet() {
        let auth =
            HyperliquidAuth::from_agent_credentials(TEST_PRIVATE_KEY, TEST_WALLET_ADDRESS)
                .unwrap();
        let wallet: Address = TEST_WALLET_ADDRESS.parse().unwrap();
        assert_eq!(auth.query_address(), wallet);
        assert_ne!(auth.query_address(), auth.address);
    }

    #[test]
    fn from_agent_credentials_invalid_key() {
        let err =
            HyperliquidAuth::from_agent_credentials("not_valid", TEST_WALLET_ADDRESS).unwrap_err();
        assert!(matches!(err, AuthError::InvalidPrivateKey(_)));
    }

    #[test]
    fn from_agent_credentials_invalid_wallet_address() {
        let err =
            HyperliquidAuth::from_agent_credentials(TEST_PRIVATE_KEY, "not_an_address")
                .unwrap_err();
        assert!(matches!(err, AuthError::InvalidPrivateKey(_)));
    }

    #[tokio::test]
    async fn cache_get_or_insert_agent() {
        let cache = AuthCache::new();
        let account_id = Uuid::new_v4();

        let auth1 = cache
            .get_or_insert_agent(account_id, TEST_PRIVATE_KEY, TEST_WALLET_ADDRESS)
            .await
            .unwrap();
        assert_eq!(cache.len().await, 1);
        assert!(matches!(auth1.auth_mode, AuthMode::Agent { .. }));

        // Second call: returns cached
        let auth2 = cache
            .get_or_insert_agent(account_id, TEST_PRIVATE_KEY, TEST_WALLET_ADDRESS)
            .await
            .unwrap();
        assert_eq!(auth1.address, auth2.address);
        assert_eq!(cache.len().await, 1);
    }

    #[test]
    fn debug_agent_mode_does_not_leak_private_key() {
        let auth =
            HyperliquidAuth::from_agent_credentials(TEST_PRIVATE_KEY, TEST_WALLET_ADDRESS)
                .unwrap();
        let debug_output = format!("{:?}", auth);
        assert!(!debug_output.contains(TEST_PRIVATE_KEY));
        assert!(debug_output.contains("Agent"));
    }

    // ==================== FIX-05: Cache Hardening Tests ====================

    #[tokio::test]
    async fn cache_concurrent_access() {
        let cache = Arc::new(AuthCache::new());
        let account_id = Uuid::new_v4();
        let address = test_address();

        let mut handles = vec![];
        for _ in 0..10 {
            let cache = Arc::clone(&cache);
            let addr = address.clone();
            handles.push(tokio::spawn(async move {
                cache
                    .get_or_insert(account_id, &addr, TEST_PRIVATE_KEY)
                    .await
            }));
        }

        let mut results = vec![];
        for handle in handles {
            results.push(handle.await.unwrap().unwrap());
        }

        // All results should have the same address
        let first = &results[0].address;
        for result in &results {
            assert_eq!(&result.address, first);
        }
        // Only one entry in cache
        assert_eq!(cache.len().await, 1);
    }

    #[tokio::test]
    async fn cache_expired_entry_refreshed() {
        // TTL of 0ms = everything expires immediately
        let cache = AuthCache::with_config(Duration::from_millis(0), 1000);
        let account_id = Uuid::new_v4();
        let address = test_address();

        // Insert
        cache
            .get_or_insert(account_id, &address, TEST_PRIVATE_KEY)
            .await
            .unwrap();
        assert_eq!(cache.len().await, 1);

        // Entry is expired, so get_or_insert should rebuild
        // (but still succeed and keep count at 1)
        let auth = cache
            .get_or_insert(account_id, &address, TEST_PRIVATE_KEY)
            .await
            .unwrap();
        assert_eq!(format!("{}", auth.address), address);
        assert_eq!(cache.len().await, 1);
    }

    #[tokio::test]
    async fn cache_lru_eviction() {
        // Max capacity of 3
        let cache = AuthCache::with_config(Duration::from_secs(3600), 3);
        let address = test_address();

        let ids: Vec<Uuid> = (0..4).map(|_| Uuid::new_v4()).collect();

        // Fill to capacity
        for id in &ids[..3] {
            cache
                .get_or_insert(*id, &address, TEST_PRIVATE_KEY)
                .await
                .unwrap();
        }
        assert_eq!(cache.len().await, 3);

        // Insert 4th — should evict oldest, keeping at 3
        cache
            .get_or_insert(ids[3], &address, TEST_PRIVATE_KEY)
            .await
            .unwrap();
        assert_eq!(cache.len().await, 3);
    }
}
