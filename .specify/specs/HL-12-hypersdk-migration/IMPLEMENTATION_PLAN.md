# HL-12-hypersdk-migration - Implementation Plan

**Spec:** `.specify/specs/HL-12-hypersdk-migration/spec.md` (legacy layout)
**Date:** 2026-09-20
**Status:** Draft - awaiting advisor Gate 1
**Layout note:** legacy (`.specify/specs/`), so no delta specs, no `vox:covered` markers, no `vox_jev.sh check` gate. Archive mode does not apply to this spec.

---

## Delta Summary

No delta layout. The change is a dependency migration plus one behavioural fix in the existing project state.

Functional requirements: 9 (FR-1 … FR-9). All are new state; none modify or remove an existing requirement document. The spec's own `## Current Mitigation` section is the removal surface.

---

## Current State Summary

The SDK surface matches the spec exactly: 14 files, 32 references, ten production symbols (`ExchangeProvider`, `InfoProvider`, `Network`, `OrderRequest`, `OrderType`, `Limit`, `Trigger`, `CancelRequest`, `ExchangeDataStatus`, `UserFillByTime`). `ClassTransfer` appears only in the wire-shape test the spec deletes. `hypersdk` 0.2.15 was downloaded and read during this gap analysis; every capability the spec assumes exists does exist - `Chain`, `HttpClient` with `all_mids`, `user_fills_by_time`, `clearinghouse_state`, `open_orders` (frontendOpenOrders), `perps()` (meta), `place`, `cancel`, `cancel_by_cloid`, `modify`, `usd_class_transfer`, `approve_agent`, and `websocket()`.

Three facts change the shape of the work, and none of them are in the spec.

**1. The alloy bump is mandatory and the spec's file list omits its blast radius.** `hypersdk` requires `alloy = "2"`; the router is on `alloy = "0.1"`. A `PrivateKeySigner` built by alloy 0.1 cannot satisfy alloy 2's `SignerSync`, so the router's alloy must move. That drags in `services/auth/siwe.rs` (EIP-191 hashing, `Signature`, address parsing, four test blocks) and the two test signers in `routes/exchanges.rs` - files the spec's `Files to Modify` does not list.

**2. The MSRV is an exact match, not headroom.** alloy 2.0.0–2.1.1 need rustc 1.91; alloy 2.2.0 through the current 2.4.2 need rustc **1.94.1**. The droplet runs exactly 1.94.1; this machine runs 1.98.0. A caret range resolves to the newest 2.x, so a `cargo update` on a newer local toolchain can lock an alloy whose `rust-version` the droplet cannot build. That is the same class of failure as the 2026-09-16 outage: green locally, `203/EXEC` on the droplet. The dependency must be pinned, not ranged.

**3. The old and new crates sign the *same* order differently, and both are correct.** Both build `Agent(source, connectionId)` over the `Exchange` / `"1"` / `1337` / `0x0` domain, and both compute `connectionId` as `keccak(msgpack(action) ‖ nonce_be ‖ vault_flag ‖ vault)`. But the hashed action body differs: the old crate msgpacks an `ActionWrapper::Order(&T)` carrying the old struct's field names, while `hypersdk` msgpacks `Action::Order(BatchOrder)` with one-character keys (`a`, `b`, `p`, `s`, `r`, `t`, `c`) and normalized decimals. Each crate hashes the same bytes it POSTs as JSON, so each is self-consistent and no `422` is expected from the swap. The consequence for this plan is that a regression test **cannot** compare old signature against new. The expected value must come from an independent reference.

Two money-path hazards surfaced that the spec does not mention.

**A. Unit change on the transfer.** The old `ClassTransfer.usd_size` is a `u64` in micro-USDC, and `exchange_api.rs` converts with `(dec * 1_000_000).trunc()`. `hypersdk`'s `UsdClassTransferAction.amount` is a decimal string built from a `Decimal`. Passing the existing micro-unit value through the new API transfers a **million times** the intended amount.

**B. The working perp→spot direction is not a class transfer.** `exchange.usd_transfer(signer.address(), amount)` sends a `UsdSend` - a core perp-to-perp USDC transfer to a destination address - addressed to the signer itself. The spec's acceptance criterion calls this "the currently working direction". It returns `ok`; it does not move funds between spot and perp.

Verified baselines on this machine (`SQLX_OFFLINE=true`, workspace at `testudo-exchange/`):

| Gate | Command | Baseline |
|---|---|---|
| Tests | `cargo test -p router --bin router` | 960 tests, 0 benchmarks |
| Lint | `cargo clippy -p router --all-targets` | `router` bin 5 warnings, test bin 6 warnings (workspace total: `common_utils` 2, `db-processor` 1) |
| Old crate present | `cargo tree -p router -i hyperliquid-sdk-rs` | resolves to `vendor/hyperliquid-sdk-rs` |
| New crate absent | `cargo tree -p router -i hypersdk` | `did not match any packages` |

Note on Jev: `~/dotfiles/scripts/vox_jev.sh gaps HL-12-hypersdk-migration` returns *"no requirements with scenarios found"*. This spec uses an FR table and acceptance checkboxes, not `### Requirement:` / `#### Scenario:` blocks, so the judgment has no representation to work from. No Jev judgments were available for this spec; the gap analysis below is a hand audit against the code.

---

## Gap Analysis

| Requirement | Current state | Gap |
|---|---|---|
| FR-1 remove `hyperliquid-sdk-rs` | In `crates/router/Cargo.toml:51`, in `Cargo.lock`, in the dependency graph | Not started |
| FR-2 delete `vendor/` + `[patch.crates-io]` | `testudo-exchange/Cargo.toml:38-39`, `vendor/hyperliquid-sdk-rs/` | Not started |
| FR-3 spot→perp via `usd_class_transfer` | `exchange.spot_transfer_to_perp(usdc, true)` on a `spotUser`/`classTransfer` action missing `hyperliquidChain`/`signatureChainId`; amount in micro-units | Wrong action, wrong units. Also see risk A |
| FR-4 every other path unchanged | `place_order`, `place_trigger_order`, `amend_order`, `cancel_order`, `close_position`, `get_balance` on `ExchangeProvider`/`InfoProvider`; `build_order_request` returns a string-typed order struct | Full rewrite of the typed surface under line 199. Response type changes from `Vec<ExchangeDataStatus>` (tuple variants) to `Vec<OrderResponseStatus>` (struct variants), so `extract_order_id`, `extract_avg_price`, `normalize_status` and their unit tests move too |
| FR-5 agent-wallet EIP-712 signing | `AuthCache` holds an alloy-0.1 `PrivateKeySigner`; the old crate signs internally | Signer must become alloy 2 for `hypersdk`. `agent_approval.rs` is unaffected - it only builds EIP-712 JSON for MetaMask and POSTs it, it does not sign |
| FR-6 replace the `Network` enum | 15 references across 7 files, all of them choosing between two URL strings | Mechanical once a local type exists |
| FR-7 route shapes unchanged | `routes/exchanges.rs` has live tests using `PrivateKeySigner`; `risk_snapshot.rs` and `ws_subscription_manager.rs` consume `Network` | Compile-visible ripple; needs the route tests to stay green |
| FR-8 no `sed` of any dependency | `scripts/deploy.sh:41-45` describes the removed registry patch; no `sed` remains | Only the stale comment block to delete |
| FR-9 swap isolated from behaviour change | Single tree, nothing shipped | The order-path rewrite and the transfer fix must be separate commits |

---

## Checkpoints

Six checkpoints. Each leaves a tree that builds and passes the full suite. CP-2 through CP-5 have no dependency on testnet credentials; CP-6 is the only one that does.

**Plan revised 2026-09-20 after CP-2 failed.** The original CP-2 assumed `hypersdk` could be added alongside `hyperliquid-sdk-rs` under an `alloy2` alias, with the old crate removed in a later checkpoint. Cargo forbids that: alloy 0.1 and alloy 2 cannot share a dependency graph. The swap is therefore atomic, and this plan places a router-owned boundary in front of it so the swap's diff stays concentrated. Evidence and the six experiments behind the conclusion are in `LEARNINGS.md`.

### On slicing - Jev adjudication

`~/dotfiles/scripts/vox_jev.sh slices HL-12-hypersdk-migration` returned `HORIZONTAL LAYER - re-slice` for every checkpoint, at confidence 0.29 to 0.42. That is above the 0.2 floor, so it is a real signal and it is recorded rather than dismissed.

It is also the expected answer, and the reason is structural. Vox's vertical-slice test assumes a feature with a user-facing path that can be walked end to end. This spec's deliverable is *the same behaviour on a different dependency*: nothing the user can see changes, so no checkpoint can terminate in a new user-visible outcome. What each checkpoint terminates in instead is one observable contract - a signing hash, an order lifecycle response, a fill conversion, a transfer body - pinned by a new assertion and wrapped in an unchanged baseline suite. Decomposing further along the obvious axis (one CP per order method, one CP per URL call site) would produce genuinely horizontal work: N checkpoints each repeating the same signer wiring and response mapping.

Two honest consequences follow. First, CP-1 and CP-5 really are convoys, not slices - a type swap and a deletion - and their only verification is that the suite count does not move. They are prerequisites for the slices around them, and they are called out as such in the sequencing rules. Second, the risk Jev is pointing at is real and is covered elsewhere: because no checkpoint produces a user-visible change, the failure mode that matters is a checkpoint that compiles and passes while changing behaviour silently. That is what CP-2's conversion tests, CP-3's pinned signing hash and body assertions, CP-4's amount assertion, and the baseline-count check on every checkpoint exist to catch.

CP-3 cannot be re-sliced. The dependency swap is atomic, so the order-path rewrite has to land with it; the mitigation is CP-2's boundary, which keeps five files out of CP-3 rather than splitting CP-3 itself.

### CP-1: Network resolution becomes local ✅

Completed 2026-09-20 by /skill:vox build.

- **Touches**: `crates/router/src/types/` (new `hl_network.rs`), `types/app.rs`, `main.rs`, `routes/exchanges.rs`, `services/risk_snapshot.rs`, `services/ws_subscription_manager.rs`, `services/hyperliquid/agent_approval.rs`, `services/hyperliquid/{universe,ws_fills,exchange_api}.rs`
- **Tasks**:
  1. Record the baseline for all four gates in the table above; paste the numbers into the spec's `LEARNINGS.md`.
  2. Add `HlNetwork { Mainnet, Testnet }` with `info_url()`, `ws_url()`, `arbitrum_chain_id()`, and `ws_url()` covering the four URL constants currently inline in `routes/exchanges.rs` (2 sites x 2 functions), `risk_snapshot.rs` (2 sites), `exchange_api.rs:362`, `ws_fills.rs:70-73`, `agent_approval.rs:24`.
  3. Add one temporary `HlNetwork::to_sdk()` returning the old `Network`, so the not-yet-migrated provider files still compile. It is the only place in the tree that will still name the SDK type for network selection, and it is deleted in CP-6.
  4. Swap the field type in `types/app.rs`, the constructor in `main.rs:360-364`, `agent_approval.rs::signature_chain_id`, `ws_subscription_manager.rs:47,81`, and every match arm in `routes/exchanges.rs` and `risk_snapshot.rs`.
  5. Add a unit test asserting the four URLs for each variant.
- **Verification**: `SQLX_OFFLINE=true cargo test -p router --bin router` produces `960 tests` plus the new URL test, and `grep -rn "hyperliquid_sdk_rs::Network" crates/router/src/{types/app.rs,main.rs,routes/exchanges.rs,services/risk_snapshot.rs,services/ws_subscription_manager.rs,services/hyperliquid/agent_approval.rs}` produces nothing.
- **Commit message**: `refactor(hl): resolve hyperliquid network locally, not through the sdk`

### CP-2: Read paths behind a router-owned boundary ✅

Completed 2026-09-20 by /skill:vox build.

The read paths are the only place the old SDK's types leak outside the Hyperliquid adapter. Sealing them costs one small port and removes five files from the swap's blast radius, so CP-3's diff is concentrated rather than scattered. The boundary is not scaffolding: after CP-3 it is implemented over `hypersdk`, which is exactly the SDK boundary FR-6 asks for.

- **Touches**: new `crates/router/src/services/hyperliquid/market_data.rs`, `services/hyperliquid/universe.rs`, `services/hyperliquid/ws_fills.rs`, `services/import_worker.rs`, `services/hl_fill_journal.rs`, `services/journal_syncer/hyperliquid.rs`
- **Tasks**:
  1. Add `HlFill` and `HlPerp` as router-owned types carrying only the fields the router actually consumes, plus a typed `HlMarketDataError`. Enumerate the fields from the consumers (`convert_hl_fill`, `compute_avg_price`, `build_event_from_fills`, `hl_fill_journal`, `import_worker`), not from the SDK struct.
  2. Add `HlMarketData` with `perps()` and `fills_since(user, since_ms)`, implemented over the old `InfoProvider`. Conversion out of the SDK type happens here and nowhere else, which is TB-16 applied at the boundary.
  3. RED first: one unit test converting a fixed SDK fill into `HlFill` and asserting every field, and one asserting the perp mapping.
  4. Migrate `universe.rs` (`meta()` to `perps()`), `ws_fills.rs`, `import_worker.rs`, `journal_syncer/hyperliquid.rs`, and `hl_fill_journal.rs` off `InfoProvider` and `UserFillByTime`, including their `make_fill` test helpers.
  5. Leave `exchange_api.rs` alone. Its `all_mids`, `user_state`, and `frontend_open_orders` calls are internal to that file, which already returns router-owned types through `ExchangeApi`.
- **Verification**: `SQLX_OFFLINE=true cargo test -p router --bin router` reports the baseline 960 listed tests plus 5 new conversion tests, and `grep -rln "hyperliquid_sdk_rs" crates/router/src --include=*.rs` names only `services/hyperliquid/market_data.rs`, `services/hyperliquid/exchange_api.rs`, `types/hl_network.rs`, and the two `services/hyperliquid/tests/` integration files, which are testnet-only and migrate in CP-3.
- **Commit message**: `refactor(hl): resolve network locally and seal the read paths` (CP-1 and CP-2 landed together, so this is the actual subject)

### CP-3: Atomic swap to hypersdk ✅

Completed 2026-09-20 by /skill:vox build.

**Needs a dedicated session.** Decision 2026-09-20: keep this checkpoint atomic rather than splitting it into a reads-only commit with order placement failing closed. Nothing ships in a state where trading is disabled at any point. The cost is that this checkpoint cannot be worked partially; it is a three to four hour block that must run to completion, because the tree does not build in the middle of it.

Everything it depends on has already been measured, so it is a writing task rather than a discovery task. See the verified mapping below.

The dependency swap cannot be split. alloy 0.1 and alloy 2 cannot share a dependency graph, so `hypersdk` in and `hyperliquid-sdk-rs` out land together, and everything that names the old crate moves with them. See `LEARNINGS.md` for the evidence.

The transfer is deliberately *not* fixed here. `spotUser`/`classTransfer` has no hypersdk equivalent, so rather than silently shipping an untested replacement inside a large mechanical commit, `transfer_usdc` returns a typed "not yet implemented" error for one commit. That keeps this commit behaviourally neutral and leaves the transfer change isolated in CP-4, which is what FR-9 asks for.

- **Touches**: `testudo-exchange/Cargo.toml`, `Cargo.lock`, `crates/router/Cargo.toml`, `vendor/` (deleted), `services/hyperliquid/{market_data,exchange_api,auth}.rs`, `services/auth/siwe.rs`, `services/exchange_api.rs`, `routes/{exchanges,trade_management}.rs`, `services/hyperliquid/tests/*`, `types/hl_network.rs`

  **Corrected 2026-09-20 after the fact.** The original list omitted `services/exchange_api.rs` and `routes/trade_management.rs`. The typed "not implemented" transfer error of task 3 needs a variant on `ExchangeApiError`, and `routes/trade_management.rs` matches that enum exhaustively in two places, so both files move. Four lines of real change in total.
- **Tasks**:
  1. Remove `hyperliquid-sdk-rs` from `crates/router/Cargo.toml`, remove the `[patch.crates-io]` block and its comment from `testudo-exchange/Cargo.toml`, and delete `testudo-exchange/vendor/`.
  2. Add `hypersdk = "0.2.15"` and `alloy = { version = "=2.4.2", default-features = false, features = ["signers", "signer-local", "k256"] }`. The exact pin is load-bearing: alloy 2.2+ declares `rust-version = 1.94.1`, which is exactly what the droplet runs, so a caret range can lock a version the droplet refuses to build. `default-features = false` matters too: alloy's defaults pull `essentials`, and with it the AWS KMS, GCP, Turnkey, and Ledger signer backends and a kzg stack the router never uses.
  3. Reimplement `HlMarketData` over hypersdk's `HttpClient` (`perps()`, `user_fills_by_time()`), leaving `HlFill` and `HlPerp` unchanged. That the boundary makes this a small, contained change is the point of CP-2.
  4. Rewrite `exchange_api.rs` internals against `Client::place`/`modify`/`cancel`/`cancel_by_cloid`/`clearinghouse_state`/`open_orders`: `build_order_request` to hypersdk's typed `OrderRequest` wrapped in `BatchOrder`, `place_order`, `place_trigger_order`, `amend_order`, `cancel_order`, `get_balance`, `find_oid_by_cloid`, and the response mapping for `OrderResponseStatus`'s struct variants, with its unit tests moved onto the new variants.
  5. Keep the existing tick rounding (`tick_size_from_mid` plus `round_dp(0) * tick`). Do not switch to `PerpMarket::round_price`; it rounds midpoint-toward-zero and would be a behaviour change under FR-4.
  6. Migrate the signer to alloy 2. `routes/exchanges.rs` needs no change (`PrivateKeySigner::random()` and `::address()` are unchanged). `services/auth/siwe.rs` needs exactly one: alloy 2's `Signature::from_bytes_and_parity` is infallible and returns `Signature`, not `Result`, so the `?` and its error mapping go. Delete `HlNetwork::to_sdk()`.
  7. RED first, then pin. Assert the EIP-712 digest directly, and assert `ActionRequest::recover()` returns the signing key's address. **Do not pin a signature.** The plan originally proposed pinning the vendored crate's expected signature `fa8a41f6...`; that value does not survive the alloy bump and was corrected on 2026-09-20 after measurement. The digest is pinned in two layers: `agent_signing_hash(Chain::Mainnet, 0xde6c4037...)` against the vendored crate's external vector, which is the exchange-verified value, and `Action::Order(fixture).prehash(...)` plus its msgpack connection id against measured goldens. The plan originally described the second digest as "the order fixture described in the verified mapping below"; no such fixture was ever written down, and `0x6c43321e...` is the *agent* hash for connection id `0xde6c4037...`, not a `prehash` of any order. Corrected 2026-09-20 rather than written into a test that would have failed. Add serde-level body assertions for `order`, `cancel`, `modify`, and `cancelByCloid`.
- **Verification**: `cargo tree -p router -i hyperliquid-sdk-rs` fails; `cargo tree -p router -i hypersdk` resolves to 0.2.15; `SQLX_OFFLINE=true cargo clippy -p router --all-targets` reports no more warnings than the CP-1 baseline; `SQLX_OFFLINE=true cargo test -p router --bin router` passes with the new assertions.

#### Verified API mapping (measured 2026-09-20)

A scratch crate outside this repo (`/tmp/alloy2probe`) compiled the whole call-site shape against `hypersdk 0.2.15` + `alloy =2.4.2` before any of this was written into the tree. It builds, so none of the mapping below is guesswork. `alloy 0.1` and `2` were also run side by side in separate crates to compare signing digests.

**Confirmed working unchanged.** `alloy::primitives::{eip191_hash_message, Address, Signature}`; `Signature::recover_address_from_prehash`; `PrivateKeySigner::{random, address, from_str}`; and `PrivateKeySigner` satisfies hypersdk's `SignerSync`, so `place`, `cancel`, `modify`, and `sign_sync` accept it directly.

**Confirmed working, new to us.** `Cloid` is `pub type Cloid = B128`, so `Cloid::from_str("0x...")` and `Cloid::ZERO` replace the old string field. `OidOrCloid = Either<u64, Cloid>`, used as `OidOrCloid::Left(oid)` in a `Modify`. `hypercore::NonceHandler` is public and `Send + Sync`; the router must now supply nonces via `NonceHandler::next()`, which the old crate generated internally. `BatchOrder { orders, grouping: OrderGrouping::Na, builder: None }`. `Side` serialises as `"B"`/`"A"` and `FillDirection` as `"Open Long"` etc., so `HlFill`'s string fields are recoverable exactly. `clearinghouse_state().margin_summary.account_value` and `BasicOrder.cloid` both work as the rewrite needs.

**Two shapes differ from the plan's assumptions.** `BatchCancelCloid` requires a `fast: bool` field (set it `false`, matching the old crate's implicit behaviour). The trigger variant names `TpSl`, not `Tpsl`.

**Fill conversion needs explicit wire formatting.** hypersdk's `Fill` parses the wire into `Decimal`, `Side`, and `FillDirection`, so `HlFill` must be rebuilt with `normalize().to_string()` for the five numeric fields and the enum matches above. Hyperliquid sends normalised numeric strings, so this should be byte-identical, but it is an assumption rather than a proof: CP-6 should compare one live `raw_json` against the exchange payload.

The signing digests were then compared directly: hypersdk's `signing::agent_signing_hash(Chain::Mainnet, connection_id)` and the vendored crate's `Agent { source: "a", .. }.eip712_signing_hash(&domain)` both return `0x6c43321e0f67bbafa52f8ad0cca68ce5e6700cac04fd5f8ccc1df6d4d3e799b0` for the same connection id. Domain and Agent encoding are byte-identical, which closes the spec's headline risk. The two crates produce *different signatures* over that identical digest, because alloy 2 derives the deterministic nonce differently; both recover to the same address, and ECDSA signatures over a digest are not unique, so the exchange accepts either. That is why the fixture pins the digest and not the signature.

**Two more facts, measured 2026-09-20.** `decimal_normalized` serialises a `Decimal` as a *string* with trailing zeros stripped (`serialize_str(&normalized.to_string())`). So hypersdk's JSON body and the msgpack bytes it hashes agree in both form and convention, which is what Hyperliquid requires and what the old crate's `limit_px: String` did. This closes the order-encoding question by construction, not by argument. Separately, `normalize_status`, `extract_order_id`, `extract_avg_price`, and `build_order_request` are referenced nowhere outside `exchange_api.rs`, so no other module depends on the SDK's order types and that file can be migrated on its own.

#### Execution order, chosen to minimise compiler iterations

1. Manifests first (tasks 1 and 2). Nothing builds from there until the end.
2. `types/hl_network.rs`: drop `to_sdk`, add `chain() -> hypersdk::hypercore::Chain`.
3. `market_data.rs`: swap `InfoProvider` for `HttpClient`, rewrite `perps()`, rewrite `Fill` to `HlFill` using `normalize().to_string()` for the numeric fields, and call `user_fills_by_time(user, start_ms, end_ms)` (three arguments, not four). The CP-2 conversion tests stay valid apart from their input type.
4. `exchange_api.rs`: hold `HttpClient` plus a `NonceHandler`; delete `build_exchange`; thread `&auth.signer` and a fresh nonce into each call; move the four internal helpers and the response mapping onto `OrderResponseStatus`'s struct variants.
5. `services/auth/siwe.rs`: drop the `?` on `Signature::from_bytes_and_parity`.
6. The two `services/hyperliquid/tests/` files, which drive the SDK directly and need rewriting against `HttpClient` and `hypercore::WebSocket`.
7. The signing fixture (task 7), last, since it needs the crate in place to run.
8. `cargo check -p router --bin router`, then work the error list. The compiler is the enumeration; do not hand-audit for missed call sites.
- **Commit message**: `refactor(hl): migrate the hyperliquid adapter to hypersdk`

### CP-4: Spot and perp transfer on usd_class_transfer ✅

Completed 2026-09-20 by /skill:vox build.

Isolated from the swap so the behavioural change is reviewable on its own, which is FR-9. Both directions move together: the perp to spot path is not a class transfer today either, it is a `UsdSend` addressed to the signer's own address, so it returns `ok` without moving funds between classes.

- **Touches**: `crates/router/src/services/hyperliquid/exchange_api.rs` (`transfer_usdc`), `crates/router/src/services/exchange_api.rs`, `crates/router/src/routes/trade_management.rs`
- **Tasks**:
  1. Replace the temporary typed error from CP-3 with `Client::usd_class_transfer(&signer, amount, to_perp, nonce, vault, expires)` for both directions, and delete the `debug_assert`/`tracing::warn` stub around it.
  2. Remove `ExchangeApiError::TransferNotImplemented` and both match arms that CP-3 added in `routes/trade_management.rs` (line 165, which maps to a user-facing message, and line 482, which maps to the `transfer_not_implemented` status string). Leaving the variant behind is dead code once nothing constructs it, and both matches are exhaustive so they will not warn.
  3. **Delete the `* 1_000_000` conversion** that fed the old `u64` micro-unit field. `usd_class_transfer` takes a decimal USDC amount, so passing micro-units transfers a million times the intended value. This is the highest-severity hazard in the migration and it is silent and expensive.
  4. Assert the `usdClassTransfer` action body, including `hyperliquidChain`, `signatureChainId`, and the amount in decimal USDC units, in the same place the deleted `class_transfer_serializes_*` guard stood. The spec's ordering constraint requires the replacement to land with the removal.
- **Verification**: `SQLX_OFFLINE=true cargo test -p router --bin router transfer` shows the three transfer tests passing. The greps must be precise: `grep -rn "\* *Decimal::from(1_000_000\|trunc()" crates/router/src/services/hyperliquid/exchange_api.rs` produces nothing (the original criterion was `grep -rn "1_000_000"`, which also matches a millisecond-to-nanosecond conversion in `hl_fill_journal.rs`, a nanosecond-to-millisecond one in `cex_history.rs`, and this file's own comments about the hazard), `grep -rn "\bClassTransfer\b" crates/router/src` matches only a doc comment that names the deleted guard (`UsdClassTransfer` does not match, because the word boundary falls inside it), and `grep -rn "spot_transfer_to_perp\|TransferNotImplemented" crates/router/src` produces nothing.
- **Commit message**: `fix(hl): transfer between spot and perp with the correct action shape`

### CP-5: Desk cleanup ✅

Completed 2026-09-20 by /skill:vox build.

- **Touches**: `scripts/deploy.sh`, `.cargo/audit.toml`, `Cargo.lock`, `crates/router/src/types/hl_network.rs`
- **Tasks**:
  1. Delete the stale comment block at `scripts/deploy.sh:41-45` describing the removed registry patch. FR-8.

     **Done.** The paragraph is gone. `bash -n` passes, no `vendor/hyperliquid-sdk-rs` or `patch.crates-io` reference remains, and the only `sed` left in the file formats a log tail. The file still carries a historical comment naming `hyperliquid-sdk-rs` as the crate whose compile error `tail -3` swallowed; that stays, because it is accurate history explaining why the log exists.
  2. Revise `.cargo/audit.toml`.

     **Done, and the answer differed from the task.** See the triage below. Three entries were dead and removed, two were still live and kept, and the gate itself needed dependency upgrades rather than pruning.
  3. Update the `hl_network.rs` doc comment.

     **Already correct from CP-3.** Verified, nothing to do.
- **Verification**: `grep -n "sed" scripts/deploy.sh` shows only log formatting, and `cargo audit` **exits 0**.
- **Commit message**: `chore(hl): close the audit gate and drop the dependency-patching clutter`

#### The audit triage

The task said five `RUSTSEC` entries existed only because of alloy 0.1.x. Re-derived by emptying the ignore list and reading what actually matched:

| Entry | Verdict |
|---|---|
| `RUSTSEC-2025-0073` alloy-dyn-abi | dead, removed |
| `RUSTSEC-2024-0370` proc-macro-error | dead, removed (crate left the lock) |
| `RUSTSEC-2026-0002` lru | dead, removed |
| `RUSTSEC-2024-0388` derivative | **still fires**, kept |
| `RUSTSEC-2024-0436` paste | **still fires**, kept |

Deleting the last two as written would have put `cargo audit` on live advisories while looking like housekeeping.

The gate itself was red with six vulnerabilities, and red before the swap, so pruning could never have closed it. Eight dependencies were upgraded instead: anyhow 1.0.104, rustls 0.23.45, event-listener 5.4.2, ruint 1.20.0, quinn-proto 0.11.15, h2 0.4.16, actix-http 3.13.6, validator_derive 0.20.1. One more was fixed by a dependency move rather than an upgrade: `rust_decimal 1.43.0` dropped its optional `rkyv ^0.7`, which removed `RUSTSEC-2026-0235` outright instead of justifying it.

Six vulnerabilities became one:

| Remaining | Crate | Built | Disposition |
|---|---|---|---|
| `RUSTSEC-2026-0258` | h2 0.3.27 via actix-http | yes | justified ignore, no reachable fix |
| `RUSTSEC-2023-0071` | rsa via sqlx-mysql | yes | justified ignore, no upstream fix |
| `RUSTSEC-2024-0388` | derivative | no | justified ignore, unmaintained |
| `RUSTSEC-2024-0436` | paste | no | justified ignore, unmaintained |

`h2 0.3.27` is the only one left carrying real exposure. `actix-http 3.13.6` is the newest 3.x release and still declares `h2 = "^0.3.27"`, so the fix needs actix-http 4 and actix-web 5. Verified against crates.io on 2026-09-20 rather than assumed. Each entry in `.cargo/audit.toml` now states its reason, and the file records the measurement method.

#### Lock is not the build graph

Worth remembering for the next audit. `cargo audit` scans `Cargo.lock`, which records optional dependencies whether or not their features are enabled. `cargo tree` measures what is built. On a crate with a large optional surface the two disagree widely: the swap added alloy 2's optional signer backends to the lock while leaving zero compiled artifacts for gcloud, aws-kms, turnkey, or hidapi. Reading the lock's advisory list as a list of shipped problems overstates it; reading `cargo tree` alone understates it, because lock-only entries still cost upgrades.

### CP-6: Production canary

**Scope changed 2026-09-20.** This checkpoint was "testnet acceptance", running six live testnet criteria. Testnet testing was declined, so it is now the spec's two production criteria instead. Those are the stronger check anyway: a real trade exercises the same order path against the live venue, and it is the only thing that can show that the exchange accepts the signatures `hypersdk` produces. The local work pins the digest against the production vector, but pinning a digest is not the exchange verifying a signature.

The six testnet criteria are recorded as **not run**, with that reason. Their checkboxes in the spec stay unticked, so the gap is visible rather than assumed closed.

- **Touches**: `LEARNINGS.md`, and the spec's acceptance checkboxes.
- **Tasks**:
  1. Deploy. `scripts/deploy.sh` runs `git pull --ff-only` and builds in place, so the push is the prerequisite. Watch the first build: `alloy 2.4.2` declares `rust-version = 1.94.1` and the droplet runs exactly `1.94.1`, so this is the one check no local gate can make. Disk was at 86% before the build. Do not run `cargo update` on the droplet; `Cargo.lock` carries the pins.
  2. Place one minimum-size order: about $10 notional, so roughly $1 of margin at 10x, with a linked stop-loss and take-profit. Then cancel it. This is the first live order this migration's code has ever placed, and `exchange_api.rs` is 1,291 changed lines covering place, trigger, amend, cancel, and close.
  3. Watch `POST /exchange` for 422s for 24 hours. A 422 means the exchange rejected a signature, which is the spec's headline risk and precisely what the digest pin cannot prove on its own.
  4. Optionally verify the transfer, since FR-3 is the reason this spec exists. It has no route, so the only way to exercise our implementation is the harness with a mainnet key. That moves your own USDC between your own spot and perp accounts: fee-free, reversible, and if the units were wrong the failure mode is a rejection rather than a loss, because the exchange will not conjure the funds.
  5. Record every result, including any failure, in `LEARNINGS.md`.
- **Verification**: `LEARNINGS.md` records the deploy, the canary trade with its status transitions, and the 422 watch. The spec's six testnet boxes stay unticked and carry the reason.
- **Commit message**: `docs(hl): record the production canary for the hypersdk migration`

**What this does not prove.** The order path is proven by task 2 and the signature acceptance by task 3. The transfer is proven only if task 4 runs; otherwise FR-3 rests on the local action-body assertion and the unit test, which pin the wire shape and the amount units but never contact an exchange. The harness that was written for the testnet criteria stays in the tree as an `#[ignore]`d test, unused by this checkpoint.

## Sequencing Rules

- CP-1 has landed. It must precede CP-3, which would otherwise re-touch the seven network files.
- CP-2 must precede CP-3, or the swap carries the read paths too.
- CP-3 is atomic and single-commit by construction: the old crate must leave in the same commit that `hypersdk` arrives.
- CP-4 must follow CP-3, because CP-3 temporarily disables the transfer rather than fixing it, and CP-4 is the FR-9 isolation commit for that fix.
- CP-6 must follow CP-4, and it is the only checkpoint that touches production: it deploys the whole migration and canaries one trade.
- A partially migrated tree must never be deployed. Every checkpoint above builds and passes the suite on its own, so no checkpoint needs a deploy.

---

## Risks and Open Questions

### Resolved decisions

**B. Perp→spot is in scope. (Decided 2026-09-20.)** Both directions move to `usd_class_transfer`. FR-4's "behave unchanged" is read as "the user-visible transfer keeps working", not as "preserve the current implementation", because the current implementation does not move funds between classes. This is a deliberate reading of FR-4 and it is a scope increase over the spec's literal wording; it is recorded here so the advisor sees it as a decision rather than a drift.

**C. The alloy pin. (Confirmed 2026-09-20.)** `=2.4.2` stands. The droplet reported `rustc 1.94.1` by direct query, which is exactly alloy 2.2.0's declared `rust-version`, so the pin is load-bearing: a ranged dependency can resolve to a version the droplet cannot build.

**D. Testnet credentials. (Confirmed available 2026-09-20.)** CP-6 can run once CP-4 lands.

**E. The dependency swap is atomic. (Established 2026-09-20, after the original CP-2 failed.)** alloy 0.1 and alloy 2 cannot coexist in one dependency graph, so the old crate leaves in the same commit `hypersdk` arrives. This removes the `alloy2` alias, the bridging signer, and the old CP-6 rename step. It also makes CP-3 the largest single commit in the spec by construction, mitigated by CP-2's boundary. Full evidence in `LEARNINGS.md`.

**F. The transfer is disabled for one commit. (2026-09-20.)** CP-3 makes `transfer_usdc` return a typed unsupported error rather than ship an untested replacement inside a large mechanical commit. This keeps CP-3 behaviourally neutral and is what makes FR-9's isolation of the transfer fix possible. It is a deliberate, temporary regression in a path that already cannot succeed, and it must not survive CP-4.

### Still open

**A. Transfer amount units.** `usd_class_transfer` takes a decimal string, the old path took micro-unit integers, and the current code multiplies by `1_000_000`. Getting this wrong sends a million times the intended amount. The fix is not in doubt - delete the conversion - but it is pinned by assertion in CP-4 rather than assumed, because the failure is silent and expensive.

### Accepted, no action

- **Two `reqwest` versions.** The router is on 0.12, hypersdk on 0.13. No type conflicts, because hypersdk's client is self-contained; the cost is a second TLS stack in the binary.
- **Signing-encoding difference.** Both crates are self-consistent with the JSON they post. Not fixable locally; pinned by CP-3's fixture and proven by CP-6.
- **Licence.** MIT to MPL-2.0, file-level copyleft, no MPL files modified. Recorded in the spec as a decision.
- **`agent_approval.rs`.** Does not sign; MetaMask does. The CP-1 network swap is its only change.

### Carried from the previous handoff, unrelated to this spec

- No liveness alert on `testudo-api`. The spec's third `[CLARIFY]` asks whether to add one first; it would have caught the 49-hour outage in minutes.
- `./uploads/journal` does not resolve relative to the systemd `WorkingDirectory`, so journal image uploads fail and actix-files logs at every startup.
- The canonical `scrub.py` pre-commit hook had both defects: the `set -e` bug, and redaction that rewrote staged files. Both are fixed at the source now. `~/.pi/agent/APPEND_SYSTEM.md` ships a hook that blocks the commit and prints `file:line`, with `@waiver PII` for a false positive and `scrub.py --fix` for deliberate redaction.
- `AGENTS.local.md` does not exist, though the generated `AGENTS.md` points at it. The 134 lines of project guidance it should hold are recoverable from `git show 68067bc1:AGENTS.md`.

---

Plan ready: 7 checkpoints, ~12 hours of focused work plus testnet time. Run `/skill:vox build HL-12-hypersdk-migration` to start CP-1.
