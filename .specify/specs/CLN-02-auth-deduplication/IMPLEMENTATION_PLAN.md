# CLN-02-auth-deduplication — Implementation Plan

## Current State Summary

`src/context/AuthContext.tsx` (334 lines) contains `runSiwe` (lines 131-185, 55 lines) and `runSiws` (lines 188-250, 63 lines) — two async functions that are ~80% identical. Both share: guard check, flag management, 20×100ms provider poll loop, user re-check after async gap, nonce fetch, error handling (same regex for rejection detection), and `finally` cleanup. Only the signing mechanism, message format, verify endpoint, and verify body differ.

Any bug fix to the guard logic or error-handling pattern must be applied in two places. The spec provides a complete `SignerConfig` interface and `runAuthFlow` implementation ready for extraction.

## Checkpoints

### CP-1: Extract SignerConfig + runAuthFlow + refactor runSiwe ✅
- Completed 2026-05-29 by /skill:vox build. Build passes, 56/56 tests pass, runSiwe is 20-line wrapper. AuthContext.tsx = 359 lines (runAuthFlow added).
- **Touches**: `src/context/AuthContext.tsx`
- **Tasks**:
  1. Define `SignerConfig` interface inside `AuthProvider` (captures chain, provider, message builder, sign function, verify endpoint, optional extra fields)
  2. Extract `runAuthFlow(config: SignerConfig)` containing all shared logic: guard, provider poll, user re-check, nonce fetch, sign, verify, error handling, cleanup
  3. Refactor `runSiwe` to be a thin wrapper (~15 lines) passing EVM-specific config to `runAuthFlow`
  4. Leave `runSiws` untouched (CP-2 will refactor it)
- **Verification**: `cd testudo-journal && bun run build` exits 0; EVM auth path still wired
- **Verification**: `wc -l src/context/AuthContext.tsx` < 334 (proves dedup is working)
- **Commit message**: `refactor: extract runAuthFlow from SIWE auth, define SignerConfig`

### CP-2: Refactor runSiws + final cleanup
- **Touches**: `src/context/AuthContext.tsx`
- **Tasks**:
  1. Refactor `runSiws` into a thin wrapper (~20 lines) passing Solana-specific config to `runAuthFlow`
  2. Delete all duplicated logic from `runSiws` (guard, poll, nonce, error handling, cleanup)
  3. Verify `AuthContext.tsx` ≤ 250 lines
- **Verification**: `cd testudo-journal && bun run build` exits 0
- **Verification**: `wc -l src/context/AuthContext.tsx` ≤ 250
- **Commit message**: `refactor: deduplicate SIWS auth flow into runAuthFlow`

---

## Risks

1. **Provider type safety** — `evmProvider` and `solanaProvider` are `any` today; `SignerConfig.sign` uses `unknown`. This preserves the existing type-erasure pattern. Better typing is CLN-03 scope.
2. **Behavioral regression** — the spec's `runAuthFlow` is a direct extraction of the existing code. CP-1 tests EVM first (the more common path), CP-2 adds Solana. No behavioral changes.
3. **`loadWallet()` call in message builder** — SIWE reads `chainId` from `(await loadWallet()).getChainId()`. This is already inside the `try` block and won't change behavior.
