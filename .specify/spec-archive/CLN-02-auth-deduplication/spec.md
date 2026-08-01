# Specification: Deduplicate SIWE/SIWS Auth Flow in AuthContext

**Spec ID:** CLN-02-auth-deduplication
**Date:** 2026-05-29
**Status:** Draft
**Class:** Infrastructure / Refactor
**Priority:** P1 — `runSiwe` and `runSiws` share ~80% identical logic; every change to auth error handling or the re-check guard must be duplicated
**Depends on:** None (touches only `AuthContext.tsx`)
**Series:** CLN-01 through CLN-03 (Journal Frontend Cleanup) — nuclear review findings on `testudo-journal/`

---

## Problem Statement

`src/context/AuthContext.tsx` contains two async functions — `runSiwe` (EVM/SIWE) and `runSiws` (Solana/SIWS) — that are ~80% identical. Each function:

1. Guards against concurrent auth (`user() || loading() || siweInFlight`)
2. Waits for the chain-specific provider with a 20-attempt × 100ms poll loop
3. Re-checks `user()` after the async gap (session may have been restored via `/me`)
4. Fetches a nonce from `/auth/nonce`
5. Constructs a sign-in message
6. Signs via the provider
7. Verifies with the backend
8. Handles rejection/cancellation errors identically
9. Cleans up flags in `finally`

Only steps 5–7 differ meaningfully — the signing mechanism (`personal_sign` vs `signMessage`), the message format (SIWE vs SIWS), and the verify endpoint (`/verify-siwe` vs `/verify-siws`).

This violates rule 6: **"Prefer existing canonical utilities/helpers over bespoke one-offs."** Any bug fix to the guard logic, the provider-wait loop, or the error-handling pattern must be applied twice — and divergence is inevitable.

---

## User Stories

- **As a journal developer**, I want a single `runAuthFlow` function that handles both EVM and Solana sign-in so that I can fix auth bugs once.
- **As a user connecting with MetaMask or Phantom**, I want consistent auth behavior regardless of which chain my wallet is on.

---

## Functional Requirements

| ID | Requirement | Priority | Subsystem |
|----|-------------|----------|-----------|
| FR-1 | Extract a canonical `runAuthFlow(signer: SignerConfig)` function containing the shared guard, provider-wait, nonce-fetch, error-handling, and cleanup logic. | High | `AuthContext.tsx` |
| FR-2 | `runSiwe` and `runSiws` become thin wrappers that pass EVM/Solana-specific `SignerConfig` to `runAuthFlow`. | High | `AuthContext.tsx` |
| FR-3 | Behavior must be identical: same retry loops, same error messages, same `userInitiatedConnect` / `siweInFlight` flag management. | High | `AuthContext.tsx` |
| FR-4 | `bun run build` exits 0. No import changes needed — all changes are internal to `AuthContext.tsx`. | High | `testudo-journal/` |

---

## Technical Implementation

### Vertical Checkpoints

| Checkpoint | Scope | Validates |
|------------|-------|-----------|
| CP-1 | Define `SignerConfig` interface + `runAuthFlow` function. Refactor `runSiwe` to use it. | `bun run build` passes; EVM auth still works |
| CP-2 | Refactor `runSiws` to use `runAuthFlow`. Delete duplicated logic. | Both SIWE and SIWS flows pass build; `AuthContext.tsx` line count reduced |

### FR-1: SignerConfig Interface

```typescript
interface SignerConfig {
  /** Chain namespace for logging ("evm" | "solana"). */
  chain: 'evm' | 'solana'

  /** Provider object to poll for readiness. */
  getProvider: () => unknown
  /** Max poll attempts (default: 20). */
  providerPollAttempts?: number

  /** Build the sign-in message from address + nonce + chainId. */
  buildMessage: (address: string, nonce: string, chainId?: number | string) => string

  /** Sign the message. Returns a string (hex for EVM, base58 for Solana). */
  sign: (provider: unknown, message: string, address: string) => Promise<string>

  /** Backend verify endpoint path. */
  verifyEndpoint: string
  /** Optional extra fields to send to the verify endpoint. */
  verifyExtraFields?: Record<string, unknown>
}
```

### FR-1: runAuthFlow

```typescript
async function runAuthFlow(config: SignerConfig): Promise<void> {
  // These are still module-level closures (same as today)
  if (user() || loading() || siweInFlight) return
  siweInFlight = true
  setSiweError(null)

  try {
    // Provider wait loop
    let attempts = 0
    const maxAttempts = config.providerPollAttempts ?? 20
    while (!config.getProvider() && attempts < maxAttempts) {
      await new Promise(r => setTimeout(r, 100))
      attempts++
    }
    if (!config.getProvider()) {
      throw new Error(`${config.chain} provider not ready — please try again`)
    }

    // Re-check user after async gap
    if (user()) { siweInFlight = false; return }

    // Nonce
    const nonceRes = await fetchAuth('/nonce')
    if (!nonceRes.ok) throw new Error('Failed to get nonce')
    const { nonce } = await nonceRes.json() as { nonce: string }

    // Chain ID (optional — SIWE uses it, SIWS doesn't)
    const chainId = (await loadWallet()).getChainId() ?? undefined

    // Build message + sign
    const message = config.buildMessage(address, nonce, chainId)
    const signature = await config.sign(config.getProvider(), message, address)

    // Verify
    const body: Record<string, unknown> = {
      message,
      signature,
      address,
      ...config.verifyExtraFields,
    }
    const verifyRes = await fetchAuth(config.verifyEndpoint, {
      method: 'POST',
      body: JSON.stringify(body),
    })
    if (!verifyRes.ok) {
      const errBody = await verifyRes.text().catch(() => '')
      throw new Error(`Verification failed: ${errBody || verifyRes.statusText}`)
    }

    const { user: u } = await verifyRes.json() as { user: User }
    setUser(u)
  } catch (err) {
    const msg = err instanceof Error ? err.message : 'Authentication failed'
    console.error(`[${config.chain.toUpperCase()}] auth failed:`, msg)
    setSiweError(
      /reject|denied|cancel/i.test(msg)
        ? 'Signature rejected — click Connect to retry'
        : msg
    )
    ;(await loadWallet()).disconnect()
  } finally {
    siweInFlight = false
    userInitiatedConnect = false
  }
}
```

### FR-2: Thin Wrappers

```typescript
async function runSiwe(address: string) {
  await runAuthFlow({
    chain: 'evm',
    getProvider: () => evmProvider,
    buildMessage: (addr, nonce, chainId) =>
      [
        `${window.location.host} wants you to sign in with your Ethereum account:`,
        addr, '', 'Sign in to Testudo', '',
        `URI: ${window.location.origin}`,
        'Version: 1',
        `Chain ID: ${chainId ?? 1}`,
        `Nonce: ${nonce}`,
        `Issued At: ${new Date().toISOString()}`,
      ].join('\n'),
    sign: (provider, message, addr) =>
      (provider as any).request({
        method: 'personal_sign',
        params: [message, addr],
      }),
    verifyEndpoint: '/verify-siwe',
  })
}

async function runSiws(address: string) {
  await runAuthFlow({
    chain: 'solana',
    getProvider: () => solanaProvider,
    buildMessage: (addr, nonce) =>
      [
        `${window.location.host} wants you to sign in with your Solana account:`,
        addr, '', 'Sign in to Testudo', '',
        `URI: ${window.location.origin}`,
        `Nonce: ${nonce}`,
        `Issued At: ${new Date().toISOString()}`,
      ].join('\n'),
    sign: async (provider, message, _addr) => {
      const encoded = new TextEncoder().encode(message)
      const sig = await (provider as any).signMessage(encoded)
      const sigBytes: Uint8Array = sig instanceof Uint8Array ? sig : sig.signature
      return base58.encode(sigBytes)
    },
    verifyEndpoint: '/verify-siws',
    verifyExtraFields: {}, // address already in message
  })
}
```

Note: the current `runSiws` includes `address` in the verify body explicitly. The new `runAuthFlow` always includes `address` in the body, so the extra field becomes unnecessary — but we preserve the behavior by passing it through `verifyExtraFields` if the backend requires it.

### Files

- `src/context/AuthContext.tsx` — refactor `runSiwe` + `runSiws` into `runAuthFlow` + `SignerConfig`

### Dependencies Added

None.

---

## Acceptance Criteria

- [ ] `runAuthFlow` contains all shared logic: guard, provider poll, nonce fetch, verify, error handling, cleanup
- [ ] `runSiwe` and `runSiws` are thin wrappers (≤ 30 lines each)
- [ ] Behavior is identical — same error messages, same flag management
- [ ] `bun run build` exits 0
- [ ] `AuthContext.tsx` line count reduced (target: ~250 lines, down from 331)

---

## Risks

1. **Behavioral regression** — the refactored flow might have subtle timing differences. Mitigation: the `SignerConfig` interface captures exactly the variation points; the shared flow is a direct extraction of the existing code. CP-1 tests EVM first (the more common path), CP-2 adds Solana.
2. **Provider type safety** — `evmProvider` and `solanaProvider` are typed as `any` today; the `SignerConfig.sign` function uses `unknown` and casts internally. This preserves the existing type-erasure pattern. Better typing is out of scope for this cleanup.

---

## Completion Signal

This spec is complete when:
1. `runAuthFlow` + `SignerConfig` extracted and tested
2. `runSiwe` and `runSiws` are thin wrappers
3. `bun run build` passes
4. Code committed to master with message `refactor: extract runAuthFlow from duplicated SIWE/SIWS logic`
