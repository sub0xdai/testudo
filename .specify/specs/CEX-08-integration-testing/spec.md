# Specification: Integration Testing — WOO X Testnet End-to-End

**Spec ID:** CEX-08-integration-testing
**Date:** 2026-03-15
**Status:** Draft
**Class:** Testing / Validation
**Priority:** P0 — final validation before production
**Depends on:** CEX-01 through CEX-07 (all prior specs)
**Series:** CEX-01 through CEX-08 (safe-cex migration)

---

## Problem Statement

The safe-cex migration replaces the entire exchange communication layer. Before deploying to production, the full trade lifecycle must be verified end-to-end on WOO X testnet, then validated on live WOO X with a small position. This spec defines the integration test plan.

---

## User Stories

- **As a developer**, I want automated integration tests that verify the full trade lifecycle, so that I can confidently deploy the migration.
- **As a trader**, I want the OCO behavior verified (SL triggers -> TP cancelled), so that orphaned orders are finally eliminated.

---

## Functional Requirements

| ID | Requirement | Priority | Subsystem |
|----|-------------|----------|-----------|
| FR-1 | New sidecar builds and starts (`bun install && bun run build && bun run start`) | High | Sidecar |
| FR-2 | Health check responds correctly (`GET /health`) | High | Sidecar |
| FR-3 | Rust backend tests pass (`cargo clippy --all-targets && cargo test`) | High | Backend |
| FR-4 | Place bracket order (entry + SL + TP) on WOO X testnet | High | Integration |
| FR-5 | Verify entry fill event received by Rust fill_detector | High | Integration |
| FR-6 | Verify SL fill event received by Rust fill_detector (algo stream) | High | Integration |
| FR-7 | Verify OCO: SL triggers -> TP cancelled automatically | High | Integration |
| FR-8 | Verify reconciler catches orphaned orders (simulate dropped WebSocket) | Medium | Integration |
| FR-9 | No orphaned orders after trade lifecycle completes | High | Integration |
| FR-10 | Deploy and verify on live WOO X with small position | Medium | Production |

---

## Technical Implementation

### Test Sequence

#### Phase 1: Build Verification

```bash
# 1. New sidecar builds and starts
cd testudo-cex && bun install && bun run build && bun run start

# 2. Health check
curl http://127.0.0.1:3100/health
# Expected: {"ok": true}

# 3. Backend tests
cd testudo-exchange && cargo clippy --all-targets && cargo test
```

#### Phase 2: Testnet Integration

```bash
# 4. Place bracket order on WOO X testnet
# Use testnet credentials
# Place: LIMIT BUY 0.001 BTC at current price - 1%
#   SL: current price - 2%
#   TP: current price + 1%

# Verify via WebSocket:
# - Entry fill event received
# - SL and TP order IDs returned
# - OrderGroup promoted to Active
```

#### Phase 3: OCO Verification

```
# Trigger SL by moving price on testnet
# Verify:
# - SL fill event received by fill_detector
# - TP automatically cancelled (OCO)
# - No orphaned orders remain
# - OrderGroup moved to Closed
```

#### Phase 4: Reconciler Test

```
# Simulate dropped WebSocket packet:
# 1. Place bracket order
# 2. Trigger SL manually on exchange
# 3. Temporarily block WebSocket events
# 4. Wait for reconciler cycle (15s)
# 5. Verify reconciler detects orphaned TP and cancels it
```

#### Phase 5: Production Validation

```
# Deploy to live with small position:
# 1. Place bracket order with minimal size
# 2. Verify all events arrive
# 3. Cancel trade manually to close
# 4. Verify clean state — no orphans
```

---

## Acceptance Criteria

- [ ] Sidecar builds, starts, and health check passes
- [ ] All Rust tests pass (800+)
- [ ] Bracket order (entry + SL + TP) placed on WOO X testnet
- [ ] Entry fill event received by Rust fill_detector
- [ ] SL fill event received by Rust fill_detector (algo stream)
- [ ] OCO fires: SL triggers -> TP cancelled automatically
- [ ] Reconciler catches orphaned orders when WebSocket packet dropped
- [ ] No orphaned orders after trade lifecycle completes
- [ ] Live WOO X validation with small position succeeds

---

## Risks

1. **WOO X testnet availability** — testnet may be down or behave differently from production. Mitigation: validate on live with minimal position as final step.
2. **Testnet API differences** — some endpoints may not be available on testnet. Mitigation: document any testnet limitations found during testing.
3. **Timing sensitivity** — fill events depend on market conditions. Mitigation: use market orders for instant fills in testing.

---

## Completion Signal

This spec is complete when:
1. All build verification passes
2. Testnet bracket order lifecycle verified
3. OCO behavior confirmed (SL -> TP cancelled)
4. Reconciler tested
5. Live validation with small position succeeds
6. No orphaned orders in any test scenario
