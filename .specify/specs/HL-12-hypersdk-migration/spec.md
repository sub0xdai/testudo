# Specification: Migrate Hyperliquid Integration from hyperliquid-sdk-rs to hypersdk

**Spec ID:** HL-12-hypersdk-migration
**Date:** 2026-09-18
**Status:** Draft
**Class:** Core / Dependency Migration
**Priority:** P1 - the current crate is unmaintained and its transfer action is structurally wrong
**Depends on:** HL-01 through HL-11 (Hyperliquid native integration)
**Series:** HL-01 through HL-12

---

## Problem Statement

The Hyperliquid integration is built on `hyperliquid-sdk-rs` v0.1.2, published by `lhermoso` to crates.io on 2025-12-14. That crate has never been updated. Two versions exist in total, both published on the same day, with 550 lifetime downloads and 81 in the recent window. It is not maintained, and there is no upstream to report defects to.

Two consequences are already visible in production.

**1. The spot-to-perp transfer does not work, for a reason a caller cannot fix.**

`spot_transfer_to_perp` (`vendor/hyperliquid-sdk-rs/src/providers/exchange/mod.rs:866`) builds a `spotUser` / `classTransfer` action from a `ClassTransfer` struct that carries only an amount and a direction. Hyperliquid expects the action body to include `hyperliquidChain` and `signatureChainId`. The action is rejected.

A second defect sits in the same struct: with `#[serde(rename_all = "camelCase")]`, its field `usd_size` serializes as `usdSize`, while the API expects `usdc`. That one is patched locally (see Current Mitigation), but the missing chain fields are not, and patching them into a crate we do not maintain is not a durable answer.

**2. The crate is a build-reproducibility hazard.**

Until this week the `usdSize`/`usdc` fix was delivered by `sed`-ing the shared cargo registry during deploy (`scripts/deploy.sh`). That made production compile a different dependency tree than every other environment, so a green local run said nothing about the droplet, and an incomplete `sed` (struct field renamed, its field-init call site not) produced `E0560` and took `testudo-api` down from 2026-09-16 10:09 to 2026-09-18 11:24 UTC.

That specific hazard is closed by vendoring, but vendoring means this repository now owns a fork of an abandoned crate. Every future fix to it is a fork fix.

**3. The replacement exists and is alive.**

`hypersdk` (repository `infinitefield/hypersdk`, published by `ifdario`) is the maintained Rust SDK for Hyperliquid: 22 releases, version 0.2.15 on 2026-08-26, 113,199 lifetime downloads and 90,820 recent. Its `usd_class_transfer` sends exactly the fields the current crate omits:

```rust
Action::UsdClassTransfer(UsdClassTransferAction {
    signature_chain_id: self.chain.arbitrum_id().to_string(),
    hyperliquid_chain: self.chain,
    amount: amount.to_string(),
    to_perp,
    nonce,
})
```

It does not model `spotUser` / `classTransfer` at all, and uses the documented `usdClassTransfer` action instead. So the migration fixes the transfer properly rather than re-patching the field name.

---

## Evidence

| | current | replacement |
|---|---|---|
| crate | `hyperliquid-sdk-rs` | `hypersdk` |
| repository | `lhermoso/hyperliquid-rust-sdk` | `infinitefield/hypersdk` |
| releases | 2 (0.1.1, 0.1.2) | 22 |
| latest | 0.1.2, 2025-12-14 | 0.2.15, 2026-08-26 |
| downloads | 550 total / 81 recent | 113,199 total / 90,820 recent |
| license | MIT | MPL-2.0 |
| edition / MSRV | 2021 / 1.70 | 2024 / 1.85 |
| spot-to-perp transfer | wrong field name, missing chain fields | correct action shape |

Toolchain check: the production droplet runs `rustc 1.94.1`, local runs `1.98.0`. Both satisfy the 1.85 minimum, so the edition 2024 dependency will build. No toolchain upgrade is required.

Licence check: MPL-2.0 is file-level copyleft. Depending on it from an AGPL-3.0 project is compatible, and the obligation only attaches to MPL-covered files, none of which we modify.

---

## Current Mitigation (what exists today)

- `vendor/hyperliquid-sdk-rs/` holds a vendored copy of 0.1.2 at git `d287f094`, with `src`, `Cargo.toml` and `LICENSE` only. The delta from pristine upstream is one field-level `#[serde(rename = "usdc")]` at `src/types/actions.rs:255` plus its comment.
- `[patch.crates-io]` in `testudo-exchange/Cargo.toml` points at it.
- `scripts/deploy.sh` no longer patches the registry.
- `crates/router/src/services/hyperliquid/exchange_api.rs` has `class_transfer_serializes_the_field_name_hyperliquid_expects`, which pins the wire shape and fails if the patch is dropped or the crate is bumped past it.

All of the above is **deleted or reverted by this migration**. It is scaffolding for a crate we are leaving.

---

## Scope

14 files reference the crate, 32 times, but the surface is far smaller than that count suggests. Production code uses ten SDK symbols in total:

`ExchangeProvider`, `InfoProvider`, `Network`, `OrderRequest`, `OrderType`, `Limit`, `Trigger`, `CancelRequest`, `ExchangeDataStatus`, and `types::info_types::UserFillByTime`.

Note that `ClassTransfer` does not appear in production code at all. Production calls `exchange.spot_transfer_to_perp(amount, true)` and lets the crate build the action. `ClassTransfer` is referenced only by the wire-shape test this spec removes.

### Network only (7 files, mechanical)

The `Network` enum is a two-variant type used solely to choose between `https://api.hyperliquid.xyz/info` and `https://api.hyperliquid-testnet.xyz/info`. Replacing it with a local enum, or a base URL resolved once at startup, removes these files from the SDK surface before any provider call is touched.

| File | Uses |
|---|---|
| `main.rs` | `Network::{Mainnet, Testnet}` |
| `routes/exchanges.rs` | `Network::{Mainnet, Testnet}`, six times |
| `services/risk_snapshot.rs` | `Network::{Mainnet, Testnet}`, four times |
| `services/ws_subscription_manager.rs` | `Network` |
| `services/hyperliquid/agent_approval.rs` | `Network` |
| `types/app.rs` | `Network` field |

### One data type only (2 files)

| File | Uses |
|---|---|
| `services/hl_fill_journal.rs` | `types::info_types::UserFillByTime` |
| `services/journal_syncer/hyperliquid.rs` | `InfoProvider`, `Network`, `UserFillByTime` |

### Provider work (the real migration)

| File | Production uses |
|---|---|
| `services/hyperliquid/exchange_api.rs` | `ExchangeProvider`, `InfoProvider`, `Network`, `OrderRequest`, `OrderType`, `Limit`, `Trigger`, `CancelRequest`, `ExchangeDataStatus` |
| `services/hyperliquid/universe.rs` | `InfoProvider`, `Network` |
| `services/hyperliquid/ws_fills.rs` | `InfoProvider`, `Network`, `UserFillByTime` |
| `services/import_worker.rs` | `InfoProvider`, `Network`, `UserFillByTime` |

`exchange_api.rs` is the bulk. It is also the only file that places, modifies, or cancels orders.

### Tests (2 files)

| File | Uses |
|---|---|
| `services/hyperliquid/tests/integration.rs` | the above plus `RawWsProvider`, `types::ws::{Message, Subscription}`, `OrderRequest::limit`, `ExchangeDataStatus::Resting` |
| `services/hyperliquid/tests/agent_wallet_integration.rs` | `ExchangeProvider`, `InfoProvider`, `Network`, `OrderRequest::limit` |

`RawWsProvider` appears only in the integration test. The live fill subscriber uses `tokio-tungstenite` directly.

---

## Functional Requirements

| ID | Requirement | Priority |
|----|-------------|----------|
| FR-1 | `hyperliquid-sdk-rs` is removed from `Cargo.toml`, `Cargo.lock`, and the dependency graph | High |
| FR-2 | `vendor/hyperliquid-sdk-rs/` and the `[patch.crates-io]` entry are deleted | High |
| FR-3 | Spot-to-perp transfer is implemented against `hypersdk`'s `usd_class_transfer`, which supplies `hyperliquidChain` and `signatureChainId` | High |
| FR-4 | Perp-to-spot transfer and every existing order path (entry, stop-loss, take-profit, cancel, close) behave unchanged | High |
| FR-5 | Agent-wallet EIP-712 signing continues to work, including the agent wrapper and L1 signing domain | High |
| FR-6 | The `Network` enum is replaced by a local type or resolved base URL, so SDK types do not leak into URL selection | Medium |
| FR-7 | `TRADES`, `EXCHANGES`, `RISK`, and `MARKET DATA` routes in `testudo-exchange/README.md` return the same shapes after the migration | High |
| FR-8 | No new `sed`-style patching of any dependency, in any script | High |
| FR-9 | The migration is reviewable as one change with the SDK swap isolated from behavioural changes | Medium |

---

## Acceptance Criteria

### Locally verifiable

- [ ] `cargo tree -p router -i hyperliquid-sdk-rs` fails (crate absent), `cargo tree -p router -i hypersdk` resolves to crates.io 0.2.x
- [ ] `crates/router` builds with no warnings introduced
- [ ] `cargo test -p router --bin router` passes, with the existing HL test files adapted
- [ ] Order and transfer request bodies are asserted at the serde level, in the style of the `ClassTransfer` test this spec deletes, so the wire shape of `order`, `usdClassTransfer`, `cancel`, and `modify` is pinned by tests rather than by inspection
- [ ] The EIP-712 signing domain and agent wrapper for an order are asserted against a fixed expected hash, so a signing-model change cannot pass silently
- [ ] `__check.sh` reports no violations on changed lines

### Requires live testnet verification

Hyperliquid testnet is reachable at `api.hyperliquid-testnet.xyz`, and the router already supports `Network::Testnet`, so these are executable rather than aspirational. They need a funded testnet account and an approved agent wallet.

- [ ] Agent wallet approval succeeds against testnet
- [ ] A limit entry with a linked stop-loss and take-profit places, fills, and is cancelled cleanly
- [ ] Spot-to-perp transfer of a small testnet amount succeeds and the resulting balances match the exchange's reported state
- [ ] Perp-to-spot transfer still succeeds (the currently working direction)
- [ ] A fill is picked up by the WebSocket subscriber and journalled
- [ ] Balance and clearinghouse queries return values consistent with the exchange UI

### Requires production verification after a canary

- [ ] The first production trade placed after the migration matches the pre-migration behaviour for entry, stop, target, and status transitions
- [ ] No `422` from `POST /exchange` in the first 24 hours after deployment

---

## Explicit Non-Goals

- Changing position sizing, the Decision Loop, the shadow engine, Dignitas, or the coach
- Changing the TS-01 TypeSafe integration
- Replacing the CEX path, `safe-cex`, or the sidecar
- Adding new exchange capabilities that `hypersdk` offers but we do not use, such as HyperEVM, Morpho, or Uniswap
- Improving test coverage outside the Hyperliquid adapter

---

## Technical Notes

### Risks

| Risk | Why it matters | Mitigation |
|---|---|---|
| Silent signing-model change | HL-08 established that the current crate signs agent actions over a `connection_id` built from msgpack of the action plus nonce, using the L1 domain with `chain_id = 1337`. `hypersdk` uses `Action::sign_sync(signer, nonce, vault_address, expires_after, chain)`. A mismatch produces HTTP 422 on every order, which is the same failure HL-08 spent a spec diagnosing | Assert the signing hash against a fixed expected value in a test before any live run |
| The transfer fix cannot be proven locally | Fixing the transfer is the main reason this spec exists, and no local check exercises it. A migration that compiles and passes unit tests can still leave the transfer broken | Testnet acceptance criteria; do not treat compile success as evidence the spec is done |
| Different type shapes | `OrderRequest::limit`, `ExchangeDataStatus::{Resting, Filled}`, `FilledOrder`, `RestingOrder`, `UserFillByTime` have no guarantee of matching fields | Migrate call site by call site; the compiler enumerates the surface |
| Untested paths | `services/hyperliquid/tests/` are integration tests that need credentials, so they provide less regression safety than they appear to | Treat the testnet list as the real safety net, not the existing test files |
| Licence drift | MIT to MPL-2.0 | Confirmed compatible; no action, recorded so it is a decision rather than an accident |
| One large change to a money path | Order placement, stops, and reconciliation all sit behind this adapter | Isolate the SDK swap from the transfer change where possible; that is FR-9 |

### Rollback

The migration is reversible by reverting the commit: the vendored crate and `[patch.crates-io]` entry are restored from git, and the previous binary is already on the droplet. A partially migrated tree should not be deployed, so the branch either builds the whole adapter or does not merge.

### Ordering Constraint

The `ClassTransfer` wire-shape test is deleted by this spec because the struct it pins will not exist. Its replacement, per the acceptance criteria, is a set of assertions covering every action we send. Removing the guard without replacing it would trade one uncovered defect class for a wider one.

### Files to Modify

- `testudo-exchange/Cargo.toml` - swap the dependency, drop `[patch.crates-io]`
- `testudo-exchange/Cargo.lock` - regenerated
- `testudo-exchange/vendor/` - deleted
- `testudo-exchange/crates/router/src/services/hyperliquid/exchange_api.rs` - the bulk of the migration
- `testudo-exchange/crates/router/src/services/hyperliquid/{universe,ws_fills,agent_approval}.rs`
- `testudo-exchange/crates/router/src/services/{import_worker,risk_snapshot,ws_subscription_manager,hl_fill_journal}.rs`
- `testudo-exchange/crates/router/src/services/journal_syncer/hyperliquid.rs`
- `testudo-exchange/crates/router/src/services/hyperliquid/tests/{integration,agent_wallet_integration}.rs`
- `testudo-exchange/crates/router/src/{main.rs,types/app.rs,routes/exchanges.rs}`
- `scripts/deploy.sh` - remove the comment describing the deleted registry patch

### Assumptions

- `hypersdk` 0.2.x exposes equivalents for every capability currently used. Its tree contains `place`/`market_order`, `cancel`, `clearinghouse`, agent approval, `usd_class_transfer`, websocket streaming and market metadata, so this is expected to hold, but it is unverified until the compiler says so.
- Hyperliquid testnet supports the same action set as mainnet for the paths under test.
- No production positions are in a state that blocks a deployment at the time of migration.

---

## Completion Signal

### Implementation Checklist

- [ ] All functional requirements implemented
- [ ] All locally verifiable acceptance criteria verified
- [ ] No new linting warnings introduced

### Testing Requirements

- [ ] `cargo test -p router --bin router` passes
- [ ] New wire-shape and signing-hash assertions added
- [ ] Testnet verification completed and recorded in the spec's LEARNINGS.md

### Done Signal

When every criterion above is satisfied, output:

```
<promise>DONE</promise>
```

---

## Clarifications Needed

- [CLARIFY] Is a funded Hyperliquid testnet account with an approved agent wallet available? Three acceptance criteria cannot be met without one, and without them the transfer fix is unverifiable, which is the main reason this spec exists.
- [CLARIFY] Should the known-broken spot-to-perp control be hidden in the journal and extension UI until the migration lands? It currently offers an action that cannot succeed.
- [CLARIFY] Is the 49-hour outage a reason to add a service-liveness alert before this migration, so a future dependency failure is noticed in minutes rather than days?

---

*Template version: 1.0*
