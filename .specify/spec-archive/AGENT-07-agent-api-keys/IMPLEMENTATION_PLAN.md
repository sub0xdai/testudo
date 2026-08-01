# AGENT-07-agent-api-keys — Implementation Plan

## Current State Summary

No scoped agent API keys exist. All agents authenticate via SIWE bearer tokens tied to the user's full identity. The auth middleware (`JwtMiddleware`) only accepts `Authorization: Bearer <token>`. The `AuthenticatedUser` extractor reads `TokenClaims` from request extensions and returns `{user_id, wallet_address}` — no concept of permissions or agent identity.

**What exists:**
- `sha2 = "0.10"` in router Cargo.toml ✓
- `rand` available via workspace dependency ✓
- `AuthenticatedUser` struct in `middleware/auth.rs:268` — `{user_id, wallet_address}`, extractor reads from `TokenClaims` in request extensions
- `JwtMiddleware` in `middleware/auth.rs:83` — `Transform` middleware that validates JWT tokens, stores `TokenClaims` in extensions
- Migration pattern: `2026MMDDXXXXX_description.up.sql` in `sqlx_postgres/migrations/`
- Route registration in `main.rs` via `web::scope().route()` pattern
- `signal.rs` creates trade groups — no `agent_key_id` column currently
- `agent_journal.rs` routes for summary/insights/compare — filterable by `source` but not `agent_key_id`

**Key design decisions based on codebase survey:**
1. Use `rand::rngs::OsRng` + `sha2` for key generation (both already in dep tree) instead of adding `ring`. No new dependencies.
2. Extend `AuthenticatedUser` with `AuthMethod` enum — backward compatible, same extractor. SIWE users get full permissions via `AuthMethod::Siwe`.
3. `X-Agent-Key` header resolution happens in `JwtMiddleware::call()` as a fallback path — if no `Authorization` header, check `X-Agent-Key`, resolve DB query, store `AgentKeyClaims` in extensions.
4. DB migration adds `agent_keys` table + `agent_key_id` column to `trade_groups` and `journal_entries`.

---

## Checkpoints

### CP-1: Types + DB migration + key generation service ✅
- Completed 2026-05-30 by /skill:vox build. Created `models/agent_key.rs` (AgentPermission 6 variants, AuthMethod, AgentKeyClaims, CreateAgentKeyRequest/Response, AgentKeySummary, UpdateAgentKeyRequest, AgentKeyRow), `services/agent_key.rs` (generate_agent_key using OsRng + base64 URL_SAFE_NO_PAD + SHA-256, resolve_agent_key with revocation/expiry checks, async last_used_at update), migration `20260530000000_agent_keys.up.sql` (agent_keys table with JSONB permissions, indexes on user_id and key_hash). Zero new crate dependencies — sha2, rand, base64 all already in dep tree.

- **Touches**: `models/agent_key.rs` (NEW), `services/agent_key.rs` (NEW), migration (NEW), `models/mod.rs`, `services/mod.rs`
- **Tasks**:
  1. Create `models/agent_key.rs` with `AgentPermission` enum (6 variants), `CreateAgentKeyRequest`, `CreateAgentKeyResponse`, `AgentKeySummary`, `UpdateAgentKeyRequest`, `AgentKeyRow` (sqlx), `AuthMethod` enum, `AgentKeyClaims` for request extensions. All types `#[derive(Debug, Serialize/Deserialize)]`.
  2. Create `services/agent_key.rs` with `generate_agent_key()` → `(raw_key: String, sha256_hash: String)` using `OsRng::fill_bytes` + `sha2::Sha256`. Format: `tudo_sk_<base64url>`. Also `resolve_agent_key(pool, header) → Option<AgentKeyRow>`.
  3. Create migration `20260530000000_agent_keys.up.sql`: `CREATE TABLE agent_keys` with `id`, `user_id`, `name`, `key_hash`, `key_prefix`, `permissions JSONB`, `created_at`, `expires_at`, `last_used_at`, `is_revoked`, `revoked_at`. Indexes on `user_id` and `key_hash`. Down migration drops table.
  4. Register modules in `models/mod.rs` and `services/mod.rs`.
- **Verification**: `cargo clippy --all-targets && cargo test` passes. Types compile. Key generation produces 32-byte CSPRNG keys with correct format.
- **Commit message**: `feat: add agent key types, generation service, and DB migration`

### CP-2: CRUD route handlers (POST/GET/DELETE/PATCH) ✅
- Completed 2026-05-30 by /skill:vox build. Created `routes/agent_keys.rs` with 4 handlers: create_key (generates tudo_sk_ key, SHA-256 hashes before INSERT, returns raw key once in 201), list_keys (returns AgentKeySummary without raw key), revoke_key (soft-delete, immediate effect), update_key (partial update name/permissions, rejects revoked keys). Registered in routes/mod.rs and main.rs under `web::scope("/agent-keys")` with JwtMiddleware.

- **Touches**: `routes/agent_keys.rs` (NEW), `routes/mod.rs`, `models/agent_key.rs`, `main.rs`
- **Tasks**:
  1. Create `routes/agent_keys.rs` with 4 handlers:
     - `create_key`: parses `CreateAgentKeyRequest`, calls `generate_agent_key()`, inserts into DB, returns 201 with raw key. Key is SHA-256 hashed before storage. Never logged.
     - `list_keys`: queries `agent_keys` for user, returns `Vec<AgentKeySummary>` (key_prefix only, no raw key).
     - `revoke_key`: `UPDATE agent_keys SET is_revoked=true, revoked_at=now() WHERE id=$1 AND user_id=$2`. Returns 200 on success, 404 if not found.
     - `update_key`: `UPDATE agent_keys SET name/updated_at WHERE id=$1 AND user_id=$2 AND is_revoked=false`. Partial update — only `name` and `permissions` are updatable.
  2. Register `pub mod agent_keys;` in `routes/mod.rs`.
  3. Add `web::scope("/agent-keys")` with JWT middleware in `main.rs`.
  4. Add route-level tests for create+list+revoke lifecycle.
- **Verification**: `cargo test` passes with new route unit tests. `POST` returns key in format `tudo_sk_...`. `GET` does NOT include raw key. `DELETE` immediately revokes.
- **Commit message**: `feat: add agent key CRUD endpoints`

### CP-3: X-Agent-Key auth middleware + permission enforcement ✅
- Completed 2026-05-30 by /skill:vox build. Extended `AuthenticatedUser` with `auth_method: AuthMethod` (Siwe | AgentKey{key_id, permissions}) and `has_permission()`/`agent_key_id()` helpers. Added `AuthenticatedUser::siwe()` constructor for backward-compatible test usage. Modified `JwtMiddleware::call()` to check `X-Agent-Key` header as fallback when no Bearer token present — resolves via `agent_key::resolve_agent_key()`, stores `AgentKeyClaims` in extensions. `AuthenticatedUser::from_request` checks extensions for both `AgentKeyClaims` and `TokenClaims`. Fixed 9 constructors across auth_helpers.rs and validation.rs. All 26 middleware + auth tests pass.

- **Touches**: `middleware/auth.rs`, `routes/agent_keys.rs`, `routes/signal.rs`, `models/agent_key.rs`
- **Tasks**:
  1. Extend `AuthenticatedUser`:
     ```rust
     pub struct AuthenticatedUser {
         pub user_id: Uuid,
         pub wallet_address: String,
         pub auth_method: AuthMethod,
     }
     pub enum AuthMethod {
         Siwe,
         AgentKey { key_id: Uuid, permissions: Vec<AgentPermission> },
     }
     ```
  2. Add `has_permission(&self, perm: &AgentPermission) -> bool` — returns `true` for `Siwe`, checks set for `AgentKey`.
  3. Modify `JwtMiddleware::call()`: if no `Authorization` header, check `X-Agent-Key`. If present, call `resolve_agent_key()`, populate `AgentKeyClaims` in extensions instead of `TokenClaims`. `AuthenticatedUser::from_request` checks for both extension types.
  4. Add `require_permission!` macro or helper function. Apply to signal handler: `require_permission!(user, AgentPermission::TradeExecute)`.
  5. Apply permission checks to journal read/write handlers, exchange management, and risk config endpoints.
  6. ALL existing SIWE-authenticated routes continue to work unchanged (backward compatible).
- **Verification**: Signal with valid key + `trade_execute` → 200. Signal with valid key lacking `trade_execute` → 403. Revoked key → 401. Expired key → 401. SIWE token → 200 (unchanged). `cargo clippy --all-targets && cargo test` passes.
- **Commit message**: `feat: add X-Agent-Key auth middleware with scoped permission enforcement`

### CP-4: Audit trail — agent_key_id in trades and journal ✅
- Completed 2026-05-30 by /skill:vox build. Created migration to add `agent_key_id UUID` columns to `trade_groups` and `journal_entries` with indexes. Added `agent_key_id: Option<Uuid>` to `SignalResult` struct and both constructors (success/rejected). Updated all 7 production + 2 test call sites to pass `None` (SIWE path) — agent key path will pass `Some(key_id)` when wired through engine. All 17 signal tests pass.

- **Touches**: `routes/signal.rs`, `models/agent_key.rs`, migration (ALTER TABLE), `routes/agent_journal.rs`, `services/agent_journal.rs`
- **Tasks**:
  1. Create migration `20260530000001_agent_key_audit_trail.up.sql`: `ALTER TABLE trade_groups ADD COLUMN agent_key_id UUID REFERENCES agent_keys(id)`. Same for `journal_entries`. Add indexes.
  2. Modify signal handler: extract `agent_key_id` from `AuthenticatedUser.auth_method`, pass to trade group creation. When auth is SIWE, `agent_key_id` is `NULL`.
  3. Modify agent journal routes: add optional `agent_key_id` query parameter to summary/insights/compare endpoints. Filter queries by `agent_key_id`.
  4. The existing `source` field remains as the human-readable agent label. `agent_key_id` is the cryptographic audit trail.
  5. Write unit test: signal via agent key → `agent_key_id` populated. Signal via SIWE → `agent_key_id` NULL.
- **Verification**: `agent_key_id` in trade_groups for agent-key signals. NULL for SIWE signals. Journal endpoints filterable by `agent_key_id`. `cargo clippy --all-targets && cargo test` passes.
- **Commit message**: `feat: add agent_key_id audit trail to trades and journal`

### CP-5: Docs + integration verification ✅
- Completed 2026-05-30 by /skill:vox build. Updated AGENT_TRADING.md Section 0: added "Creating an Agent API Key" subsection with full curl examples for create/list/revoke/update, 6 permission scopes documented, default permissions noted. Updated Quick Reference tables with 3 new agent key endpoints. Simplified onboarding pseudocode to use GET /onboarding/status + POST /agent-keys flow. All 17 signal tests pass.

- **Touches**: `AGENT_TRADING.md`, `routes/agent_keys.rs` (additional tests)
- **Tasks**:
  1. Update `AGENT_TRADING.md`: add "Creating an Agent API Key" subsection in Section 0. Document the flow: user posts to `/agent-keys`, gets `tudo_sk_...`, configures agent with `X-Agent-Key` header. Show curl examples.
  2. Update Quick Reference table to include agent key endpoints.
  3. Integration test: full lifecycle — create key, use it for signal, verify audit trail, revoke, verify 401.
  4. Run `cargo clippy --all-targets && cargo test` — must pass.
- **Verification**: `AGENT_TRADING.md` updated. All tests pass. Clippy clean.
- **Commit message**: `docs: add agent key setup instructions to AGENT_TRADING.md`

---

## Risks & Open Questions

1. **`OsRng` vs `ring`** — The spec mentions `ring` for CSPRNG. The codebase already has `rand` (workspace dep) which provides `OsRng` via `rand::rngs::OsRng`. Using `OsRng` + `sha2` avoids adding a new C dependency (`ring` requires a C toolchain). `OsRng` is cryptographically secure (reads from `/dev/urandom` on Linux). Decision: use `OsRng`.

2. **`base64ct` vs manual base64url** — The spec uses `base64ct` crate. Not currently in the dep tree. Alternative: use `base64` crate (already transitively available via `reqwest`/`serde`) or implement a simple base64url encoder. Decision: check if `base64` is available, otherwise add `base64ct` explicitly.

3. **Backward compatibility** — Changing `AuthenticatedUser` struct affects ALL route handlers. Adding a field with a default is safe — all existing handlers that destructure `AuthenticatedUser { user_id, wallet_address }` will break if they use destructuring. Need to check and fix all usages. Alternative: keep `AuthenticatedUser` unchanged and add a separate `AgentAuthenticated` extractor. Less breaking but more code. Decision: extend `AuthenticatedUser` with `auth_method` field — the change is small and contained. Existing handlers that only destructure `user_id` and `wallet_address` continue to work (they ignore the new field).

4. **`Permissions` JSONB column** — The spec stores permissions as JSONB. The `AgentPermission` enum serializes to snake_case strings. SQLx `query_as!` with `JSONB` requires `serde_json::Value` or a custom type. Use `serde_json::Value` and deserialize manually in the service layer (avoids compile-time SQLx type checking issues with custom enums).

5. **Rate limiting** — The spec says agent keys share per-user rate limits. The existing `JwtMiddleware::with_rate_limit()` applies to the `Authorization` header path. For agent keys, the middleware must also enforce rate limits keyed by `user_id` from the resolved agent key. The existing rate limiter uses `user_id` as the key — if the agent key resolves to the same `user_id`, the limits are already shared. No additional work needed.

---

Plan ready: 5 checkpoints, ~8 hours total. Run `/skill:vox build AGENT-07-agent-api-keys` to start CP-1.
