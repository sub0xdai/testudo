# Specification: Add Solana Wallet Authentication (SIWS)

**Spec ID:** AUTH-03-solana-auth
**Date:** 2026-04-09
**Status:** Draft
**Class:** Infrastructure / Auth
**Priority:** P1 — enables Solana-native users (pump.fun token holders) to sign in without EVM wallet
**Depends on:** None (SIWE auth already in place)
**Series:** AUTH-03 (standalone, extends AUTH-02 wallet-primary auth)

---

## Problem Statement

Testudo currently authenticates users exclusively via SIWE (Sign In With Ethereum), which requires an EVM-compatible wallet. This creates friction for Solana-native users who hold wallets like Phantom in Solana mode, Solflare, or Backpack. Since Testudo plans to launch a token on pump.fun (Solana), the primary audience for that launch will be Solana users who may not have EVM wallets configured.

The auth system needs to support both EVM (secp256k1/SIWE) and Solana (Ed25519/SIWS) signature verification while maintaining the existing wallet-primary identity model: each wallet address = one user, no cross-chain linking.

The current `wallet_address: String` field on the User model is already a plain string with no EVM-specific constraints in the database, making this extension straightforward at the data layer. The work is primarily in adding a parallel signature verification path (Ed25519) and a Solana wallet adapter on the frontend.

---

## User Stories

- **As a Solana wallet user**, I want to sign in with my Phantom/Solflare wallet, so that I can access Testudo without needing MetaMask or an EVM wallet.
- **As a trader**, I want the wallet connection modal to show both EVM and Solana wallets in one picker, so that I don't have to think about which chain I'm on.
- **As a pump.fun token holder**, I want to sign in with the same wallet that holds my tokens, so that there's zero friction between token ownership and platform access.

---

## Functional Requirements

| ID | Requirement | Priority | Subsystem |
|----|-------------|----------|-----------|
| FR-1 | Backend verifies Ed25519 signatures from Solana wallets via `ed25519-dalek` | High | Router/Auth |
| FR-2 | New `POST /api/v1/auth/verify-siws` endpoint consumes nonce, validates domain/expiry, verifies signature, issues JWT | High | Router/Auth |
| FR-3 | Wallet address normalization accepts both `0x`-prefixed EVM (lowercased) and base58 Solana (case-preserved) formats | High | Router/Auth |
| FR-4 | Frontend Reown AppKit includes `SolanaAdapter` and `solana` network — single modal shows both EVM and Solana wallets | High | Journal/Auth |
| FR-5 | Frontend `AuthContext` detects connected namespace (`eip155` vs `solana`) and runs the appropriate sign-in flow | High | Journal/Auth |
| FR-6 | Existing SIWE flow unchanged — no regression for EVM wallet users | High | All |
| FR-7 | Shared endpoints (`/nonce`, `/me`, `/refresh`, `/logout`) work identically for both wallet types | Medium | Router/Auth |

---

## Technical Implementation

### Vertical Checkpoints

| Checkpoint | Scope | Validates |
|------------|-------|-----------|
| CP-1 | `siws.rs`: parse SIWS message + Ed25519 verify with `ed25519-dalek` + unit tests | Crypto verification works in isolation |
| CP-2 | `routes/auth.rs`: `/verify-siws` endpoint + address normalization + integration | Full backend auth flow for Solana wallets |
| CP-3 | Frontend: Solana adapter + `runSiws()` in AuthContext | End-to-end Phantom Solana sign-in |
| CP-4 | Manual E2E: Phantom (Solana) + MetaMask (EVM) both sign in successfully | No regression, both paths work |

### Backend: SIWS Message Format

Plaintext message signed by Solana `signMessage` (Ed25519 over raw bytes):

```text
testudo.app wants you to sign in with your Solana account:
7xKXtg2CW87d97TXJSDpbD5jBkheTqA83TZRuJosgAsU

Sign in to Testudo

URI: https://testudo.app
Nonce: abc123def456
Issued At: 2026-04-09T12:00:00Z
```

No `Chain ID` (Solana is one chain). No `Version` (no formal spec to version).

### Backend: Key Types

```rust
// crates/router/src/services/auth/siws.rs

use chrono::{DateTime, Utc};
use common_utils::auth::AuthError;
use ed25519_dalek::{Signature, VerifyingKey};

pub struct SiwsMessage {
    pub domain: String,
    pub address: String,       // base58 Solana public key
    pub statement: Option<String>,
    pub uri: String,
    pub nonce: String,
    pub issued_at: DateTime<Utc>,
    pub expiration_time: Option<DateTime<Utc>>,
}

pub fn parse_siws_message(message: &str) -> Result<SiwsMessage, AuthError> {
    // Similar line-by-line parsing as siwe.rs
    // Header: "{domain} wants you to sign in with your Solana account:"
    // Line 1: base58 address
    // Remaining: URI, Nonce, Issued At, optional Expiration Time
}

pub fn validate_siws_message(
    msg: &SiwsMessage,
    expected_domain: &str,
    nonce_valid: bool,
) -> Result<(), AuthError> {
    // Domain match, nonce validity, expiration check
    // Same logic as validate_siwe_message minus version check
}

pub fn verify_siws_signature(
    message: &str,
    signature_b58: &str,
    claimed_address: &str,
) -> Result<String, AuthError> {
    let pubkey_bytes = bs58::decode(claimed_address)
        .into_vec()
        .map_err(|_| AuthError::Unauthorized("invalid base58 address".into()))?;

    let verifying_key = VerifyingKey::from_bytes(
        &pubkey_bytes.try_into()
            .map_err(|_| AuthError::Unauthorized("pubkey must be 32 bytes".into()))?
    ).map_err(|e| AuthError::Unauthorized(format!("invalid pubkey: {e}")))?;

    let sig_bytes = bs58::decode(signature_b58)
        .into_vec()
        .map_err(|_| AuthError::Unauthorized("invalid base58 signature".into()))?;

    let signature = Signature::from_bytes(
        &sig_bytes.try_into()
            .map_err(|_| AuthError::Unauthorized("signature must be 64 bytes".into()))?
    );

    verifying_key.verify_strict(message.as_bytes(), &signature)
        .map_err(|_| AuthError::Unauthorized("Ed25519 signature verification failed".into()))?;

    Ok(claimed_address.to_string())
}
```

### Backend: Address Normalization

```rust
// crates/router/src/services/auth/mod.rs or siws.rs

pub fn normalize_wallet_address(addr: &str) -> Result<String, AuthError> {
    if addr.starts_with("0x") && addr.len() == 42 {
        Ok(addr.to_lowercase())
    } else if addr.len() >= 32 && addr.len() <= 44 {
        bs58::decode(addr).into_vec()
            .map_err(|_| AuthError::Unauthorized("invalid wallet address".into()))?;
        Ok(addr.to_string())
    } else {
        Err(AuthError::Unauthorized("unrecognized wallet address format".into()))
    }
}
```

### Backend: Verify Endpoint

```rust
// In routes/auth.rs

#[derive(Deserialize)]
pub struct VerifySiwsRequest {
    message: String,
    signature: String,  // base58-encoded 64-byte Ed25519 sig
    address: String,    // base58 Solana pubkey
}

async fn verify_siws(
    body: web::Json<VerifySiwsRequest>,
    // ... same deps as verify_siwe
) -> Result<HttpResponse, ApiError> {
    let parsed = parse_siws_message(&body.message)?;
    let nonce_valid = consume_nonce(&parsed.nonce).await;
    validate_siws_message(&parsed, &expected_domain, nonce_valid)?;
    verify_siws_signature(&body.message, &body.signature, &body.address)?;

    // Ensure claimed address matches message address
    if parsed.address != body.address {
        return Err(AuthError::Unauthorized("address mismatch"));
    }

    let wallet = normalize_wallet_address(&body.address)?;
    let user = find_or_create_user(&wallet).await?;
    // Issue JWT cookies — identical to SIWE flow
}
```

### Frontend: Wallet Config

```typescript
// src/config/wallet.ts
import { SolanaAdapter } from '@reown/appkit-adapter-solana'
import { solana } from '@reown/appkit/networks'

const solanaAdapter = new SolanaAdapter()

export const appKit = createAppKit({
  adapters: [ethersAdapter, solanaAdapter],
  networks: [mainnet, arbitrum, base, polygon, solana],
  // ... rest unchanged
})
```

### Frontend: AuthContext Branching

```typescript
// src/context/AuthContext.tsx

let evmProvider: any = null
let solanaProvider: any = null

const unsubProviders = appKit.subscribeProviders((state: Record<string, any>) => {
  evmProvider = state['eip155'] ?? null
  solanaProvider = state['solana'] ?? null
})

// In subscribeAccount callback:
if (state.isConnected && state.address && !user() && !siweInFlight && userInitiatedConnect) {
  const chainNs = appKit.getCaipNetwork()?.chainNamespace
  if (chainNs === 'solana' && solanaProvider) {
    runSiws(state.address)
  } else if (evmProvider) {
    runSiwe(state.address)
  }
}

async function runSiws(address: string) {
  if (user() || loading() || siweInFlight) return
  siweInFlight = true
  setSiweError(null)

  try {
    let attempts = 0
    while (!solanaProvider && attempts < 20) {
      await new Promise(r => setTimeout(r, 100))
      attempts++
    }
    if (!solanaProvider) throw new Error('Solana provider not ready')
    if (user()) { siweInFlight = false; return }

    const { nonce } = await fetchAuth('/nonce').then(r => r.json())

    const message = [
      `${window.location.host} wants you to sign in with your Solana account:`,
      address, '', 'Sign in to Testudo', '',
      `URI: ${window.location.origin}`,
      `Nonce: ${nonce}`,
      `Issued At: ${new Date().toISOString()}`,
    ].join('\n')

    const encoded = new TextEncoder().encode(message)
    const sig = await solanaProvider.signMessage(encoded)
    // sig may be Uint8Array or { signature: Uint8Array } depending on adapter
    const sigBytes = sig instanceof Uint8Array ? sig : sig.signature

    const verifyRes = await fetchAuth('/verify-siws', {
      method: 'POST',
      body: JSON.stringify({
        message,
        signature: bs58encode(sigBytes),
        address,
      }),
    })
    if (!verifyRes.ok) throw new Error('SIWS verification failed')

    const { user: u } = await verifyRes.json()
    setUser(u)
  } catch (err) {
    const msg = err instanceof Error ? err.message : 'Authentication failed'
    console.error('[SIWS] auth failed:', msg)
    setSiweError(
      /reject|denied|cancel/i.test(msg)
        ? 'Signature rejected — click Connect to retry'
        : msg
    )
    appKit.disconnect()
  } finally {
    siweInFlight = false
    userInitiatedConnect = false
  }
}
```

### Paved Roads

- SIWE parsing pattern: `crates/router/src/services/auth/siwe.rs` — SIWS mirrors this structure
- Nonce management: existing `/nonce` endpoint + consumption logic — shared by both flows
- JWT issuance: existing `generate_tokens()` + cookie setting — chain-agnostic
- AppKit provider subscription: existing `subscribeProviders` pattern in `AuthContext.tsx`

### Files

- `crates/router/src/services/auth/siws.rs` — **new** — parse + verify SIWS
- `crates/router/src/services/auth/mod.rs` — **modified** — add `normalize_wallet_address()`, export siws
- `crates/router/src/routes/auth.rs` — **modified** — add `/verify-siws` endpoint
- `testudo-journal/src/config/wallet.ts` — **modified** — add SolanaAdapter + solana network
- `testudo-journal/src/context/AuthContext.tsx` — **modified** — track Solana provider, add `runSiws()`, branch on namespace
- `testudo-journal/src/utils/bs58.ts` — **new** — tiny base58 encode helper (or use `bs58` npm package)

### Dependencies Added

**Backend (Cargo.toml):**
- `ed25519-dalek = "2"` — Ed25519 signature verification
- `bs58 = "0.5"` — base58 encode/decode for Solana addresses and signatures

**Frontend (package.json):**
- `@reown/appkit-adapter-solana` — Solana wallet adapter for AppKit
- `bs58` — base58 encoding for signature serialization (if not bundled with adapter)

---

## Acceptance Criteria

- [ ] `parse_siws_message()` correctly parses the SIWS plaintext format
- [ ] `verify_siws_signature()` verifies a real Ed25519 signature (unit test with `ed25519-dalek` keypair)
- [ ] `POST /verify-siws` returns JWT cookies on valid Solana signature
- [ ] `POST /verify-siws` rejects invalid signatures, expired nonces, wrong domains
- [ ] `normalize_wallet_address()` accepts both `0x` EVM and base58 Solana, rejects garbage
- [ ] Existing SIWE flow unchanged — MetaMask login still works
- [ ] AppKit modal shows both EVM and Solana wallet options
- [ ] Phantom (Solana mode) can complete full sign-in flow
- [ ] `cargo clippy --all-targets && cargo test` passes
- [ ] `cd testudo-journal && bun run build` passes

---

## Risks

1. **Reown AppKit Solana adapter maturity** — `@reown/appkit-adapter-solana` may have rough edges or undocumented behavior (e.g. `signMessage` return type varies). Mitigation: test with Phantom specifically, handle both `Uint8Array` and `{ signature: Uint8Array }` return shapes.
2. **Signature encoding mismatch** — Solana wallets may return signatures as base58 or raw bytes depending on adapter. Mitigation: normalize on frontend before sending to backend, accept both base58 and hex on backend.
3. **Namespace detection** — `appKit.getCaipNetwork()?.chainNamespace` may not reliably distinguish EVM vs Solana at the moment of account connection. Mitigation: fall back to address format detection (`0x` prefix = EVM, else Solana).

---

## Completion Signal

This spec is complete when:
1. Solana wallet users can sign in via Phantom and receive a valid session
2. EVM wallet users experience no change to their auth flow
3. All acceptance criteria met
4. Both `cargo test` and `bun run build` pass
5. Code committed to master
