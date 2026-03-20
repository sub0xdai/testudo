# Specification: Credential Management & EIP-712 Auth

**Spec ID:** HL-02-credential-auth
**Date:** 2026-03-16
**Status:** Complete
**Class:** Infrastructure / Exchange Integration
**Priority:** P1 — prerequisite for HL-03 and HL-04
**Depends on:** HL-01 (module structure)
**Series:** HL-01 through HL-06 (native Hyperliquid integration)

---

## Problem Statement

Hyperliquid uses Ethereum private key + EIP-712 signing, not API key + HMAC. The existing `ExchangeAccount` stores `encrypted_api_key` + `encrypted_secret` — we reuse these fields without schema changes.

Credential mapping (no DB migration needed):
- `encrypted_api_key` → Ethereum address (hex, for display/verification)
- `encrypted_secret` → Ethereum private key (hex)
- `exchange_name` → `"hyperliquid"`

---

## User Stories

- **As a developer**, I want to construct Hyperliquid signers from existing encrypted credentials, so that no database migration is needed.
- **As a developer**, I want signers cached per account, so that private key parsing isn't repeated on every API call.
- **As a developer**, I want address verification on construction, so that credential corruption is caught early.

---

## Functional Requirements

| ID | Requirement | Priority | Subsystem |
|----|-------------|----------|-----------|
| FR-1 | `HyperliquidAuth` struct wrapping `alloy::signers::local::PrivateKeySigner` | High | Router |
| FR-2 | `from_credentials(api_key, secret) → Result<HyperliquidAuth>` factory | High | Router |
| FR-3 | Derive Ethereum address from private key and verify against stored `api_key` | High | Router |
| FR-4 | Accept hex private keys with or without `0x` prefix | Medium | Router |
| FR-5 | Allow empty `api_key` (skips verification, for initial setup) | Medium | Router |
| FR-6 | `AuthCache` with `get_or_insert(account_id, api_key, secret)` — per-account caching | High | Router |
| FR-7 | `invalidate(account_id)` for credential rotation | Medium | Router |
| FR-8 | Debug impl must not leak private key material | High | Security |

---

## Technical Implementation

### Key Types

```rust
pub struct HyperliquidAuth {
    pub signer: PrivateKeySigner,
    pub address: alloy::primitives::Address,
}

pub struct AuthCache {
    cache: RwLock<HashMap<Uuid, HyperliquidAuth>>,
}
```

### Files

- `crates/router/src/services/hyperliquid/auth.rs` — HyperliquidAuth + AuthCache

### Reuse

- `ExchangeAccountRepository::load_credentials()` — AES-GCM vault decryption
- Existing `encrypted_api_key`/`encrypted_secret` columns — no migration

---

## Acceptance Criteria

- [x] `from_credentials(address, private_key)` constructs signer
- [x] Address mismatch returns `AuthError::AddressMismatch`
- [x] Invalid key returns `AuthError::InvalidPrivateKey`
- [x] Empty api_key skips verification
- [x] `0x`-prefixed keys work
- [x] Debug output never contains private key material
- [x] AuthCache caches on first call, returns cached on second
- [x] `invalidate()` removes cached entry
- [x] Unit tests pass

---

## Completion Signal

This spec is complete when:
1. HyperliquidAuth constructs signers from existing credential storage
2. AuthCache provides per-account caching with invalidation
3. All unit tests pass
4. Code committed to master
