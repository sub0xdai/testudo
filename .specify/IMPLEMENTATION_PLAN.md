# Implementation Plan

> Last updated: 2026-04-09
> Current spec: AUTH-03-solana-auth
> Phase: BUILD

---

## Active Spec: AUTH-03-solana-auth

### Track A: Backend (Rust) — independent, no frontend dependency

| Task | Description | Status | Files |
|------|-------------|--------|-------|
| T1 | Add `ed25519-dalek` + `bs58` deps to router Cargo.toml | pending | `crates/router/Cargo.toml` |
| T2 | Implement `siws.rs` — parse, validate, verify + unit tests | pending | `services/auth/siws.rs` (new), `services/auth/mod.rs` |
| T3 | Add `normalize_wallet_address()` + tests | pending | `services/auth/mod.rs` |
| T4 | Add `POST /verify-siws` endpoint + route registration | pending | `routes/auth.rs`, `main.rs` |

### Track B: Frontend (TypeScript) — independent, no backend dependency

| Task | Description | Status | Files |
|------|-------------|--------|-------|
| T5 | Add Solana adapter + network to AppKit config | pending | `config/wallet.ts`, `package.json` |
| T6 | Add `runSiws()` + namespace branching to AuthContext | pending | `context/AuthContext.tsx` |

### Track C: Verification — depends on A + B

| Task | Description | Status | Files |
|------|-------------|--------|-------|
| T7 | E2E verification: clippy, tests, build, manual Phantom + MetaMask test | pending | spec.md |

---

## Task Details

### T1: Add ed25519-dalek and bs58 dependencies
- Add `ed25519-dalek = "2"` and `bs58 = "0.5"` to `testudo-exchange/crates/router/Cargo.toml`
- Verify `cargo check` passes

### T2: Implement siws.rs — parse + verify + unit tests
- Create `testudo-exchange/crates/router/src/services/auth/siws.rs`
- Implement `SiwsMessage` struct, `parse_siws_message()`, `validate_siws_message()`, `verify_siws_signature()`
- Mirror patterns from `siwe.rs` — same error types, same validation flow
- Header format: `"{domain} wants you to sign in with your Solana account:"`
- No Chain ID or Version fields (Solana is one chain, no spec to version)
- Fields: domain, address (base58), statement (optional), URI, Nonce, Issued At, Expiration Time (optional)
- Write comprehensive unit tests:
  - Parse valid SIWS message with/without statement
  - Parse with expiration
  - Reject invalid/short/malformed messages
  - Reject missing fields
  - Reject invalid base58 address
  - Verify real Ed25519 signature (generate keypair in test)
  - Reject tampered signature
  - Domain match/mismatch
  - Nonce valid/invalid
  - Expired message rejection
  - Full sign + verify round-trip with real keypair
- Export from `services/auth/mod.rs`

### T3: Add normalize_wallet_address + tests
- Add to `services/auth/mod.rs`
- EVM: `0x`-prefixed, 42 chars → lowercase
- Solana: 32-44 chars, valid base58 → preserve case
- Reject anything else
- Tests for: EVM valid, Solana valid, too short, bad chars, empty

### T4: Add /verify-siws endpoint + route registration
- `VerifySiwsRequest { message, signature, address }` — all Strings
- Handler flow mirrors `verify_siwe()`:
  1. `parse_siws_message()`
  2. `nonce_store.consume()`
  3. `validate_siws_message()` with `SIWE_DOMAIN` env var
  4. `verify_siws_signature()` with message, signature, address
  5. Assert `parsed.address == body.address`
  6. `normalize_wallet_address()`
  7. `user_repo.find_or_create_by_wallet()`
  8. `create_session_tokens()` + cookies
- Register: `.route("/verify-siws", web::post().to(auth::verify_siws))` in main.rs at line ~867

### T5: Add Solana adapter + network to AppKit config
- `bun add @reown/appkit-adapter-solana` (check if bs58 also needed)
- `wallet.ts`: import `SolanaAdapter`, `solana` network
- Add to adapters and networks arrays
- `bun run build` must pass

### T6: Add runSiws() + namespace branching to AuthContext
- Track `solanaProvider` via `subscribeProviders` state `['solana']`
- In `subscribeAccount`: detect `appKit.getCaipNetwork()?.chainNamespace`
- `'solana'` → `runSiws(address)`, else → `runSiwe(address)`
- Fallback: if chainNamespace unavailable, detect by address format
- `runSiws()` implementation:
  - Fetch nonce, build SIWS message (Solana header, no Chain ID/Version)
  - `TextEncoder().encode(message)` → `solanaProvider.signMessage()`
  - Handle `Uint8Array` return (AppKit unwraps it)
  - Base58-encode signature → POST `/verify-siws` with `{ message, signature, address }`
  - Same error handling as `runSiwe()`

### T7: E2E verification
- `cargo clippy --all-targets && cargo test`
- `cd testudo-journal && bun run build`
- Manual: Phantom Solana sign-in works
- Manual: MetaMask EVM sign-in unchanged
- Update spec status → Complete
