# Specification: AuthCache Hardening — TOCTOU Fix and Eviction

**Spec ID:** FIX-05-auth-cache-hardening
**Date:** 2026-03-16
**Status:** Complete
**Class:** Refactor / Reliability
**Priority:** P1 — unbounded memory growth + stale credential race
**Depends on:** None
**Series:** FIX-01 through FIX-07 (Hyperliquid audit remediation)
**Audit Refs:** High #7, High #8, High #14

---

## Problem Statement

The `AuthCache` has three issues:

1. **TOCTOU race** (`auth.rs:153-174`): `get_or_insert` reads under read-lock, drops the lock, constructs a signer, then writes under write-lock. Between drop and reacquire, N concurrent threads all see the cache empty, all construct signers redundantly. During credential rotation, a stale signer can overwrite a fresh one if `invalidate()` runs between the read and write.

2. **Unbounded growth** (`auth.rs:141-143`): `HashMap<Uuid, HyperliquidAuth>` with no TTL, no capacity limit, no LRU eviction. Only `invalidate()` removes entries (called on migrate/revoke). Deleted or inactive accounts leak forever.

3. **Fail-open default** (`routes/exchanges.rs:121,218`): `is_active: row.is_active.unwrap_or(true)` defaults NULL to active. A security-sensitive field should fail-safe to inactive.

---

## User Stories

- **As a platform operator**, I want the auth cache to have bounded memory usage, so that long-running processes don't leak memory.
- **As a developer**, I want credential caching to be race-free, so that rotation operations are reliable.

---

## Functional Requirements

| ID | Requirement | Priority | Subsystem |
|----|-------------|----------|-----------|
| FR-1 | Fix TOCTOU: double-check cache under write lock, or use entry API | High | Router (auth.rs) |
| FR-2 | Deduplicate `get_or_insert` and `get_or_insert_agent` into a single generic method | High | Router (auth.rs) |
| FR-3 | Add TTL-based eviction (entries expire after configurable duration) | High | Router (auth.rs) |
| FR-4 | Add max capacity bound (LRU eviction when full) | Medium | Router (auth.rs) |
| FR-5 | Change `is_active` default from `true` to `false` (fail-safe) | High | Router (routes/exchanges.rs) |

---

## Technical Implementation

### TOCTOU Fix + Deduplication

```rust
impl AuthCache {
    /// Get a cached signer or build one. Thread-safe: checks cache under write lock
    /// after construction to prevent duplicate inserts during concurrent access.
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
                if !entry.is_expired() {
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
                if !existing.is_expired() {
                    return Ok(existing.auth.clone());
                }
            }
            cache.insert(account_id, CacheEntry::new(auth.clone()));

            // Evict if over capacity
            if cache.len() > self.max_capacity {
                self.evict_oldest(&mut cache);
            }
        }
        Ok(auth)
    }

    pub async fn get_or_insert(...) -> Result<HyperliquidAuth, AuthError> {
        self.get_or_build(account_id, || HyperliquidAuth::from_credentials(api_key, secret)).await
    }

    pub async fn get_or_insert_agent(...) -> Result<HyperliquidAuth, AuthError> {
        self.get_or_build(account_id, || HyperliquidAuth::from_agent_credentials(agent_key, wallet_address)).await
    }
}
```

### Cache Entry with TTL

```rust
struct CacheEntry {
    auth: HyperliquidAuth,
    inserted_at: Instant,
}

impl CacheEntry {
    fn new(auth: HyperliquidAuth) -> Self {
        Self { auth, inserted_at: Instant::now() }
    }

    fn is_expired(&self) -> bool {
        self.inserted_at.elapsed() > Duration::from_secs(3600) // 1 hour default
    }
}
```

### Fail-Safe Default

```rust
// BEFORE
is_active: row.is_active.unwrap_or(true),
// AFTER
is_active: row.is_active.unwrap_or(false),
```

### Files

- `crates/router/src/services/hyperliquid/auth.rs` — TOCTOU fix, dedup, eviction
- `crates/router/src/routes/exchanges.rs` — is_active default change

---

## Acceptance Criteria

- [x] `get_or_insert` and `get_or_insert_agent` delegate to a single `get_or_build` method
- [x] Write lock path double-checks the cache before inserting
- [x] Cache entries have a configurable TTL (default 1 hour)
- [x] Cache has a max capacity (default 1000) with LRU eviction
- [x] `is_active` defaults to `false` on NULL
- [x] Concurrent access test: two tasks calling `get_or_insert` simultaneously produce correct results
- [x] Expired entry test: entry is refreshed after TTL
- [x] `cargo clippy --all-targets && cargo test` passes

---

## Risks

1. **Write lock held during eviction** — evicting under write lock blocks readers. Mitigation: eviction is O(1) if using LRU, and the cache is small (<1000 entries).
2. **is_active default change** — existing NULL rows will now default to inactive. Mitigation: verify that no existing rows have NULL `is_active` in production (the migration sets a default).

---

## Completion Signal

This spec is complete when:
1. TOCTOU race is eliminated
2. Cache has bounded growth with TTL and capacity limits
3. Fail-safe defaults are in place
4. All tests pass
5. Code committed to master
