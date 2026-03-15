# Specification: Fork safe-cex and Strip Broker IDs

**Spec ID:** CEX-01-fork-safe-cex
**Date:** 2026-03-15
**Status:** Complete
**Class:** Infrastructure / Migration
**Priority:** P1 — prerequisite for all CEX specs
**Depends on:** None (first in series)
**Series:** CEX-01 through CEX-08 (safe-cex migration)

---

## Problem Statement

The current CCXT sidecar (`testudo-ccxt/`) uses the CCXT library which **does not implement `watchOrders` for WOO X** — the base class throws `NotSupported`. The entire WebSocket fill detection path has never worked for WOO X. OCO cancellation has never fired. 15 fix attempts over 2 weeks addressed placement logic, but the root cause is CCXT's missing WebSocket support.

The `safe-cex` library (MIT, gmtech-xyz/tuleep.trade) subscribes to **both** `executionreport` AND `algoexecutionreportv2` WebSocket topics for WOO X, providing fill events for regular orders AND algo/stop orders. It also maintains an internal reactive Store.

This spec forks safe-cex, strips third-party broker IDs, and prepares it as a vendored dependency.

---

## User Stories

- **As a developer**, I want a clean fork of safe-cex without broker ID injection, so that trades are attributed to our own accounts.
- **As a developer**, I want the fork to build cleanly with Bun, so it integrates with our existing tooling.

---

## Functional Requirements

| ID | Requirement | Priority | Subsystem |
|----|-------------|----------|-----------|
| FR-1 | Fork `gmtech-xyz/safe-cex` to `sub0xdai/safe-cex` on GitHub | High | Git |
| FR-2 | Clone fork to `safe-cex-sub0/` (adjacent to testudo-cex) | High | Git |
| FR-3 | Strip broker ID injection from WOO X API interceptor (`src/exchanges/woo/woo.api.ts`) | High | safe-cex |
| FR-4 | Strip broker ID injection from Binance API interceptor | High | safe-cex |
| FR-5 | Strip broker ID injection from Bybit API interceptor | High | safe-cex |
| FR-6 | Strip broker ID injection from any other exchange interceptors (OKX, Bitget, Gate, Blofin, Phemex) | Medium | safe-cex |
| FR-7 | Verify fork builds cleanly (`bun run build`) | High | Build |
| FR-8 | Verify all existing safe-cex tests pass | High | Testing |

---

## Technical Implementation

### 1) Fork Repository (FR-1)

```bash
# Fork gmtech-xyz/safe-cex to sub0xdai/safe-cex on GitHub
# Then clone as submodule
git submodule add git@github.com:sub0xdai/safe-cex.git testudo-cex/vendor/safe-cex
```

### 2) Strip Broker IDs (FR-3 through FR-6)

Search for broker ID injection patterns in all exchange API files:

```bash
# Find all broker ID references
grep -r "broker_id\|brokerId\|broker" testudo-cex/vendor/safe-cex/src/exchanges/ --include="*.ts"
```

Key files to inspect:
- `src/exchanges/woo/woo.api.ts` — axios interceptor injects `broker_id`/`brokerId`
- `src/exchanges/binance/binance.api.ts` — similar interceptor pattern
- `src/exchanges/bybit/bybit.api.ts` — similar interceptor pattern
- All other exchange dirs under `src/exchanges/`

Remove the interceptor logic that adds broker IDs to API requests. Keep everything else — the exchange implementations are battle-tested.

### 3) Build Verification (FR-7, FR-8)

```bash
cd testudo-cex/vendor/safe-cex
bun install
bun run build
bun test
```

---

## Acceptance Criteria

- [ ] Forked repo exists at `sub0xdai/safe-cex`
- [ ] Submodule added at `testudo-cex/vendor/safe-cex`
- [ ] No broker IDs in any exchange API interceptor (grep returns zero matches)
- [ ] `bun run build` succeeds with zero errors
- [ ] All existing safe-cex tests pass

---

## Risks

1. **Upstream drift** — our fork diverges from gmtech-xyz/safe-cex updates. Mitigation: periodically merge upstream, re-strip broker IDs.
2. **Hidden broker ID locations** — broker IDs may be injected in non-obvious locations. Mitigation: comprehensive grep across entire source tree.

---

## Completion Signal

This spec is complete when:
1. Fork exists and is cloned as submodule
2. All broker IDs stripped
3. Build and tests pass
4. Changes committed to master
