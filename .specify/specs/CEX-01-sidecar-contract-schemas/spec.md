# Specification: Sidecar Contract Schemas — Single Source of Truth for CEX Wire Shapes

**Spec ID:** CEX-01-sidecar-contract-schemas
**Date:** 2026-04-24
**Status:** Draft — parked (tech-debt / insurance; not a live incident)
**Class:** Refactor / Cross-Cutting
**Priority:** P2 — preventative. Does not fix any currently-observed production symptom. Hedges against a future safe-cex upstream field rename or shape change.
**Depends on:** None
**Siblings:** FIX-08 (adjacent concern — FIX-08 was semantic drift in business logic; this spec covers wire-shape drift).

---

## Problem Statement

The CCXT sidecar has three independent consumers of its HTTP/WS wire contract:

1. **Rust `CexClient`** (`crates/router/src/services/cex_client.rs`) — hand-maintained serde structs (`SidecarBalanceEntry`, `SidecarPositionResponse`, `SidecarOrderResponse`, `SidecarOpenOrderResponse`, `SidecarFetchOrderResponse`, `OrderUpdateEvent`). Tolerates missing/null fields via `Option<T>` — **silently drops** drifted fields.
2. **Browser extension** — Zod schemas in `testudo-extension/src/schemas.ts` (incoming validation at its own boundary). Strict.
3. **Sidecar vitests** — `testudo-cex/tests/handlers.test.ts`, using `toEqual(...)` on hand-crafted fixtures. Catch drift only when the test is up to date.

No single source of truth. The three definitions can drift from each other and from the safe-cex upstream that feeds them. Evidence surfaced during FIX-08:

- `/balance` vitest fails on the `upnl` field — sidecar handler transform dropped or renamed it; Rust consumer would silently absorb.
- `/position` vitest fails on `leverage: "10"` stringified — sidecar transform shape changed; Rust `leverage: Option<String>` silently drops.

Rust's permissive deserialization is the highest-risk point: drift reaches the router undetected until a downstream computation produces a wrong value. The journal P&L class of bugs that FIX-08 addressed is the *semantic* version of this failure mode; the *shape* version hasn't bitten production yet but is latent.

## Goal

Single Zod-authored contract per sidecar response, enforced on the producer side (sidecar) and consumed by both other sides (extension import; Rust codegen). Shape drift becomes a build failure in dev/CI and a logged alert in prod — not a silent wrong value.

---

## Design (locked via brainstorm 2026-04-24)

### Posture
**Strict everywhere, one shared schema** (option A of the failure-mode matrix). Drift breaks builds, not runtimes.

### Authoring
**Zod-first inside `testudo-cex/src/contracts/`.** One file per endpoint response. Sidecar owns the contract because it is the producer. No new workspace/package.

### Rust codegen
Router crate grows a `build.rs`:

1. `cargo:rerun-if-changed=../../../testudo-cex/src/contracts/`
2. Shell out: `bun x zod-to-json-schema --input contracts/index.ts --output $OUT_DIR/schemas.json`
3. `typify --input $OUT_DIR/schemas.json --output $OUT_DIR/contracts.rs`
4. `crates/router/src/contracts/mod.rs`: `include!(concat!(env!("OUT_DIR"), "/contracts.rs"));`

**Vendored-schema fallback.** `schemas.json` is also committed under `crates/router/contracts/schemas.json`. If `bun` is not on `$PATH`, `build.rs` uses the vendored copy and emits a warning. CI always runs Bun to catch vendored-vs-source drift. Typify pinned in `[build-dependencies]`.

### Sidecar enforcement — env-gated
```ts
// testudo-cex/src/contracts/validate.ts
export function validateResponse<T>(
  schema: z.ZodSchema<T>,
  obj: unknown,
  context: { endpoint: string; exchange: string; user_id?: string }
): T {
  const parsed = schema.safeParse(obj);
  if (parsed.success) return parsed.data;

  const isProd = process.env.NODE_ENV === "production";
  if (!isProd) throw new SchemaValidationError(schema, parsed.error.issues, context);

  logger.error({ event: "sidecar.contract.drift", ...context,
                 issues: parsed.error.issues, sample: safeSample(obj) });
  return obj as T;
}
```

- **Dev/staging/CI:** throw → 500 `SchemaValidationError` → Rust's existing 500-handling path retries. Drift is loud.
- **Prod:** log `sidecar.contract.drift` structured event, ship the object unvalidated. Grafana/Loki alert on `count_over_time({app="testudo-cex"} |= "sidecar.contract.drift" [15m]) > 0`.
- **`safeSample`:** redacts keys matching `/key|secret|token|credential/i`, truncates arrays to first 5 elements.

### Consumer wiring
- **Extension:** update `testudo-extension/src/schemas.ts` to re-export canonical schemas from `testudo-cex/src/contracts/` via relative path through the monorepo. Existing `ExchangePositionSchema` becomes a re-export.
- **Rust:** call sites in `cex_client.rs` import `router::contracts::BalanceResponse` (etc.) instead of `SidecarBalanceEntry`. Old hand-written structs deleted per endpoint.

### Schema rules (non-negotiable)
- Every field has a comment pointing at its safe-cex origin.
- `nullable()` / `.optional()` only when the upstream contract genuinely permits absence.
- No `z.any()` or `z.unknown()` in response schemas.

---

## Scope — Read-First Ratchet

Write endpoints and WS are **deferred** to a follow-up spec (CEX-02 or similar). Rationale: a schema mistake on a read endpoint causes a retryable error; a mistake on a write endpoint rejects a valid trade response and could orphan exchange state.

### CP-1 — Infrastructure + `/balance` reference

- `testudo-cex/src/contracts/{index,balance,validate}.ts`
- `testudo-cex/src/contracts/SchemaValidationError` + Express error handler wiring
- Router `build.rs` + `typify` + vendored-fallback
- Router `src/contracts/mod.rs`
- Router smoke test: `tests/contracts_smoke.rs` (deserialize a known-good fixture)
- Delete `SidecarBalanceEntry`, migrate call sites
- Re-align failing `/balance` vitest to the schema-driven shape

**Estimate:** 1 day. Hardest CP; everything after is mechanical.

### CP-2 — `/position`
- `contracts/position.ts` with `leverage: z.string().nullable()` per per-exchange drift
- Migrate handler + `SidecarPositionResponse` callers
- Re-align failing `/position` vitest

**Estimate:** 2–3h.

### CP-3 — `/orders/open`
- `contracts/orders-open.ts`
- Migrate handler + `SidecarOpenOrderResponse` callers

**Estimate:** 2–3h.

### CP-4 — `/order/fetch`
- `contracts/order-fetch.ts` with Bybit-only shape + 501 NotImplemented variant
- Migrate handler + `SidecarFetchOrderResponse` callers

**Estimate:** 2–3h.

### CP-5 — Extension integration + LEARNINGS
- Extension imports from `testudo-cex/src/contracts/`
- LEARNINGS.md: Bun-as-build-dep tradeoff, vendored-fallback rationale, env-gated enforcement policy, alert wiring (Grafana/Loki), deferred work (writes + WS)

**Estimate:** 3h.

**Total:** ~2 days of real work.

---

## Deferred (CEX-02 or later)

- `/order`, `/order/edit`, `/order/cancel`, `/leverage` write endpoints
- WS `OrderUpdateEvent` payload (note: FIX-01/02 already tightened this semantically; shape-level Zod is additive)
- Alert wiring in `testudo-ops` (Grafana rule + notification target)

---

## Non-Goals

- **Preventing semantic drift.** FIX-08 territory. This spec catches wire-shape mismatches (field names, types, required/optional). It does not catch a field that carries an unexpected *meaning* (e.g. `price: "0"` being a trigger-price sentinel).
- **Enforcing schemas on untrusted inbound HTTP.** The sidecar already parses its request envelope; that's a separate Zod surface.
- **Replacing the extension's existing Zod file wholesale in CP-1.** Extension migration rolls up in CP-5 to keep Rust-side CPs surgical.

---

## Open Questions

1. **Monorepo layout for extension import.** Currently `testudo-extension` is a sibling of `testudo-cex`, not a workspace package. Relative import (`../testudo-cex/src/contracts`) works but is ugly. Alternative: symlink or a lightweight `package.json` declaring `testudo-cex` as a local dep. Decide during CP-5.
2. **Alert target.** `testudo-ops` currently has Prometheus+Grafana. Is there a notification channel wired (Slack, email, PagerDuty)? If not, CP-5 ships the log line only and calls out the alert-wiring dependency in LEARNINGS.
3. **Bun version in router's CI.** The Rust CI container will need Bun available in `$PATH`. Add to Dockerfile / GitHub Actions setup step.
