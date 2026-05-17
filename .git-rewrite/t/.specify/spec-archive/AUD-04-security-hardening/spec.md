# Specification: Security Hardening

**Spec ID:** AUD-04-security-hardening
**Date:** 2026-03-07
**Status:** Complete
**Class:** Audit
**Phase:** 1 (Safety-Critical)
**Audit Refs:** CORS wildcard, encryption key fallback, idempotency tokens

---

## Overview

Three targeted security fixes that are individually small but collectively critical for a financial application handling exchange API credentials and live trades.

**Current state:**
- CORS uses `Cors::permissive()` (`main.rs:533`) — `Access-Control-Allow-Origin: *` allows any origin to make authenticated requests
- Missing `ENCRYPTION_KEY` env var falls back to ephemeral in-memory key (`main.rs:147-151`) — exchange credentials lost on pod restart, no error
- No idempotency mechanism — duplicate POST requests (network retry, double-click) can create duplicate trades

**Target state:**
- CORS restricted to known origins (web domain + extension IDs)
- Missing encryption key fails fast at startup with a clear error message
- Trade creation accepts an idempotency key; duplicate requests return cached response

---

## Functional Requirements

| ID | Requirement | Priority | Subsystem |
|----|-------------|----------|-----------|
| FR-1 | Replace `Cors::permissive()` with explicit origin allowlist from environment variable `ALLOWED_ORIGINS` | Critical | Router / Main |
| FR-2 | Always include Chrome/Firefox extension origin patterns in CORS allowlist | Critical | Router / Main |
| FR-3 | Remove ephemeral encryption key fallback — `panic!` or return startup error if `ENCRYPTION_KEY` is not set | Critical | Router / Main |
| FR-4 | Add `idempotency_key: Option<String>` field to trade creation request | High | Router / Trade Management |
| FR-5 | Store idempotency keys in a time-bounded cache (TTL: 5 minutes) with their responses | High | Router / Trade Management |
| FR-6 | Return cached response for duplicate idempotency keys instead of creating new trade | High | Router / Trade Management |
| FR-7 | Add test: CORS rejects requests from unknown origins | High | Test |
| FR-8 | Add test: startup fails without ENCRYPTION_KEY | High | Test |
| FR-9 | Add test: duplicate idempotency key returns cached response, not new trade | High | Test |

---

## Technical Implementation

### 1) CORS Allowlist (FR-1, FR-2)

```rust
let allowed_origins = std::env::var("ALLOWED_ORIGINS")
    .unwrap_or_else(|_| "https://testudo.app".to_string());

let cors = Cors::default()
    .allowed_origin_fn(move |origin, _req_head| {
        let origin_str = origin.to_str().unwrap_or("");
        // Check explicit origins
        if allowed_origins.split(',').any(|o| o.trim() == origin_str) {
            return true;
        }
        // Allow Chrome extension origins
        if origin_str.starts_with("chrome-extension://") {
            return true;
        }
        // Allow Firefox extension origins
        if origin_str.starts_with("moz-extension://") {
            return true;
        }
        false
    })
    .allowed_methods(vec!["GET", "POST", "PUT", "DELETE", "OPTIONS"])
    .allowed_headers(vec![header::AUTHORIZATION, header::CONTENT_TYPE])
    .max_age(3600);
```

### 2) Encryption Key Fail-Fast (FR-3)

```rust
// Replace:
let vault = AesGcmVault::from_env().unwrap_or_else(|_| {
    log::warn!("ENCRYPTION_KEY not set, using ephemeral key");
    AesGcmVault::ephemeral()
});

// With:
let vault = AesGcmVault::from_env().expect(
    "ENCRYPTION_KEY environment variable is required. \
     Exchange credentials cannot be stored without it."
);
```

### 3) Idempotency Cache (FR-4, FR-5, FR-6)

```rust
use std::collections::HashMap;
use tokio::sync::RwLock;

pub struct IdempotencyCache {
    entries: RwLock<HashMap<String, (Instant, serde_json::Value)>>,
}

impl IdempotencyCache {
    pub async fn check_or_insert(&self, key: &str) -> Option<serde_json::Value> {
        // Prune expired entries (> 5 min)
        let mut entries = self.entries.write().await;
        let cutoff = Instant::now() - Duration::from_secs(300);
        entries.retain(|_, (t, _)| *t > cutoff);

        if let Some((_, response)) = entries.get(key) {
            return Some(response.clone());
        }
        None
    }

    pub async fn store(&self, key: String, response: serde_json::Value) {
        self.entries.write().await.insert(key, (Instant::now(), response));
    }
}
```

Extension sends `Idempotency-Key` header (UUID generated per trade attempt).

---

## Verification

```bash
cd testudo-exchange && cargo clippy --all-targets && cargo test
```

- [ ] CORS rejects requests from arbitrary origins
- [ ] CORS allows requests from configured web domain
- [ ] CORS allows Chrome/Firefox extension origins
- [ ] Server refuses to start without ENCRYPTION_KEY
- [ ] Duplicate trade request with same idempotency key returns cached response
- [ ] Idempotency cache entries expire after 5 minutes
- [ ] All existing tests still pass
