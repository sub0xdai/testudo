# HL-12-hypersdk-migration - LEARNINGS

Per-spec learning record. Written as each checkpoint lands.

---

## Baselines (recorded at CP-1, before any code changed)

All commands run from `testudo-exchange/` with `SQLX_OFFLINE=true`.

| Gate | Command | Baseline |
|---|---|---|
| Tests | `cargo test -p router --bin router` | 938 passed, 22 ignored, 0 failed (960 listed) |
| Lint | `cargo clippy -p router --all-targets` | `router` bin 5 warnings, test bin 6 warnings |
| Lint (workspace) | `cargo clippy --all-targets` | `common_utils` 2, `db-processor` 1 |
| Old crate present | `cargo tree -p router -i hyperliquid-sdk-rs` | resolves to `vendor/hyperliquid-sdk-rs` |
| New crate absent | `cargo tree -p router -i hypersdk` | `did not match any packages` |

Toolchains: this machine `rustc 1.98.0`, droplet (`n0x`) `rustc 1.94.1`. The droplet
version was confirmed by direct query, not taken from the spec. It matters because
alloy 2.2.0 and later declare `rust-version = 1.94.1`, so the droplet sits exactly on
the boundary.

---

## CP-1: Network resolution becomes local (2026-09-20)

**Outcome.** 14 files changed, +73 / -161, plus a new `types/hl_network.rs`. Tests
stayed at 960 listed, clippy stayed at 5 warnings on the router bin. No SDK `Network`
reference remains outside `types/hl_network.rs`, which is now the single boundary where
the type is named.

### The test count did not go up, and that is correct

The plan expected "960 tests plus the new URL test". The count is 960 before and after.
Three URL tests were deleted from `agent_approval.rs` (`test_chain_string`,
`test_exchange_url`, `test_info_url`) and three were added in `types/hl_network.rs`.
Net zero. The behaviour is still asserted; the assertions moved to the single place that
now owns the URLs. A rising count is not the signal here, and pretending it was would
have hidden a redundant duplicate.

### Do not run `cargo fmt -p router` inside a checkpoint

`cargo fmt -p router` reformatted **94 files** in one pass. The router crate is not
rustfmt-clean under this toolchain, so formatting the crate rewrites code this spec has
no business touching. That would have destroyed the reviewable-diff property FR-9 asks
for, and it would collide with every other in-flight branch.

It was reverted in full and the edits re-applied by hand. The clean diff is 14 files.

If a checkpoint needs formatting, format only the lines it added, or run the formatter
on the one new file. Never crate-wide.

### The plan's CP-1 file list was short by two files

`services/hyperliquid/tests/{integration,agent_wallet_integration}.rs` also had to
change, because `AssetUniverse::fetch` and `HyperliquidFillSubscriber::new` changed
their parameter type and the tests call both. The plan listed those two files under
CP-3. Any checkpoint that changes a provider's public signature must carry its call
sites, including test call sites.

### `&'static str` over `String` for the URL helpers

The deleted `agent_approval.rs` helpers returned `String` by allocating in each match
arm. `HlNetwork::info_url` and friends return `&'static str` constants, which is
allocation-free and is what TB-05 wants, but it changes the call sites: `.post(&url)`
becomes `.post(url)`, because `reqwest` implements `IntoUrl` for `&str` and not for
`&&str`. Two call sites needed that change.

### Temporary bridge, and where it dies

`HlNetwork::to_sdk()` is the only place the vendored `Network` type is still named. It
exists because `InfoProvider::new` and `ExchangeProvider::{mainnet,testnet}` still want
the SDK type. It is called from `universe.rs`, `ws_fills.rs`, `exchange_api.rs`,
`import_worker.rs`, `journal_syncer/hyperliquid.rs`, and the two integration test files.
CP-6 deletes it along with the crate. One guard test pins the mapping so a future
edit cannot silently flip mainnet and testnet.

---

## CP-2 blocked: alloy 0.1 and alloy 2 cannot coexist (2026-09-20)

The plan's CP-2 assumed `hypersdk` (which needs `alloy = "2"`) could be added
*alongside* `hyperliquid-sdk-rs` (which needs `alloy = "0.1"`), with alloy 2 arriving
under an `alloy2` alias, both crates in the tree until CP-6 removed the old one.

**That is impossible. Cargo forbids it, for two independent reasons.**

### Blocker 1: duplicate `links` value

```
error: failed to select a version for `alloy`.
    ... required by package `hyperliquid-sdk-rs v0.1.2 (vendor/hyperliquid-sdk-rs)`
package `alloy` links to the native library `ckzg`, but it conflicts with a previous
package which links to `ckzg` as well:
package `router v0.1.0`
note: only one package in the dependency graph may specify the same links value
to ensure that only one copy of a native library is linked in the final binary
```

alloy 0.1.x pulls `c-kzg 1.0.3`; alloy 2.x pulls `c-kzg 2.1.8`. Both declare
`links = "ckzg"`, and cargo allows only one package per graph to declare a given
`links` value. The old crate reaches c-kzg through its `alloy` `full` feature.

### Blocker 2: the duplicate `alloy` dependency edge itself

Narrowing the vendored fork's `alloy` request to `full` minus `kzg`, `provider-ws`, and
`provider-ipc` removes blocker 1. It does not help, because a second, unrelated failure
appears:

```
versions that meet the requirements `^0.1` are: 0.1.4, 0.1.3, 0.1.2, 0.1.1, 0.1.0
package `router` depends on `alloy` with feature `signer-local` but `alloy` does not
have that feature.
```

This is requested on the `^0.1` edge while alloy 2 is also in the graph. Note that
alloy 0.1.4's index entry declares `signer-local` only in `features2`, not in
`features`. Whatever cargo is doing internally, the practical answer is the same.

### Experiments behind the conclusion

All run against the current lock with `SQLX_OFFLINE=true`.

| Manifest state | Result |
|---|---|
| pristine tree | resolves, builds |
| pristine + `once_cell = "1"` (forces a full re-resolve, no alloy change) | **resolves**, exit 0 |
| pristine + `hypersdk = "0.2.15"` | fails, blocker 1 |
| pristine + `alloy2 = { package = "alloy", version = "=2.4.2" }` only | fails, blocker 1 |
| pristine + vendored fork's alloy narrowed to drop kzg | fails, blocker 2 |
| old crate and `[patch.crates-io]` removed, `hypersdk` + `alloy = "=2.4.2"` added | **resolves**, exit 0 |

The `once_cell` row is the control. It proves the `alloy = { version = "0.1", features =
["signers", "signer-local"] }` line is not latently invalid and that re-resolution in
general works. The failure is specific to having both alloy majors present.

### What this means for the plan

1. **The dependency swap is atomic.** `hypersdk` in and `hyperliquid-sdk-rs` out must
   land in the same commit. There is no intermediate state where both exist, so there is
   no `alloy2` alias to rename later and no bridging signer to write.
2. **The alloy bump cannot be split out either.** The router cannot move to alloy 2
   while the old crate is present, so `siwe.rs`, the `routes/exchanges.rs` test signers,
   and `ExchangeProvider<PrivateKeySigner>` all migrate inside the swap.
3. **A non-buildable commit is not acceptable**, so unless the router's code is first
   decoupled from the old SDK's types, the swap must carry every call site at once.
   That argues for an intermediate boundary before the swap.
4. **The order path already has its boundary.** `crate::services::exchange_api::ExchangeApi`
   is the internal boundary and returns router-owned types, so the SDK leak into other
   modules is only the read paths: `InfoProvider` and `UserFillByTime` used directly by
   `universe.rs`, `ws_fills.rs`, `import_worker.rs`, `journal_syncer/hyperliquid.rs`, and
   `hl_fill_journal.rs`.

The end state is confirmed reachable, so this is a sequencing problem, not a
feasibility one.

### Also worth knowing

With the old crate gone, `alloy = { version = "=2.4.2", features = ["signers",
"signer-local"] }` keeps alloy's default features, which include `essentials`. The
feature set should be chosen deliberately at swap time (`default-features = false`) so
alloy's optional signer backends (AWS KMS, GCP, Turnkey, Ledger) and its kzg stack are
not pulled into a trading router. The router uses only `Address`, `Signature`,
`eip191_hash_message`, `PrivateKeySigner`, and `SignerSync`.

---

## CP-2: Read paths behind a router-owned boundary (2026-09-20)

**Outcome.** New `services/hyperliquid/market_data.rs` (306 lines, 5 tests, 3 TB-01
waivers). Five consumers migrated off `InfoProvider` and `UserFillByTime`:
`universe.rs`, `ws_fills.rs`, `import_worker.rs`, `journal_syncer/hyperliquid.rs`,
`hl_fill_journal.rs`. Tests went 960 to 965 listed, clippy stayed at 5 warnings on the
router bin, `__check.sh` reported zero violations on changed lines.

After this checkpoint the Hyperliquid SDK is named by exactly three production files:
`market_data.rs` (the boundary), `exchange_api.rs` (order path, self-contained behind the
`ExchangeApi` trait), and `types/hl_network.rs` (the `to_sdk` bridge). CP-3's diff is
now concentrated in those three plus the Cargo manifests, instead of scattered across
fourteen.

### `raw_json` is why `HlFill` mirrors all fifteen fields

The obvious boundary design is to carry only the fields the router reads. That would have
been a silent data change. `journal_syncer::hyperliquid::convert_hl_fill` persists
`serde_json::to_value(fill)` into `raw_fills.raw_json`, so the SDK struct's whole wire
shape was already being stored. A narrower `HlFill` would have dropped `crossed`,
`hash`, `start_position`, and `cloid` from every future row while leaving existing rows
with them.

`HlFill` therefore carries all fifteen fields with the SDK's `camelCase` naming, and
`hl_fill_matches_the_sdk_wire_shape` asserts
`to_value(HlFill::from(&sdk_fill)) == to_value(&sdk_fill)`. That test is the guard: it
compares serialized forms rather than field by field, so a renamed, added, or dropped
field fails it.

Worth generalising: before narrowing an ingested type, grep for where the original gets
serialized. The consumer that reads every field may be a database column, not a function.

### Rate-limit classification moved to the boundary, with the same predicate

`journal_syncer` used to sniff the error string for `429` or `rate` inline and map to
`SyncError::RateLimit`. The boundary now owns that predicate and returns a typed
`HlMarketDataError::RateLimited`, so the consumer matches on a variant. The predicate
itself is unchanged and is duplicated nowhere. When `hypersdk` arrives in CP-3 it
exposes a typed status, and this is the one place that has to change.

### The plan's verification line under-listed the files

CP-2's grep criterion named three files; the real answer is five. The two
`services/hyperliquid/tests/` integration files are `#[cfg(test)]`, testnet-only, and
drive the SDK directly, so they stay until CP-3. The plan has been corrected. The
production-code claim in the criterion held exactly.

### TB-01 waivers on pure mapping

Three functions carry `// @waiver TB-01` annotations: the `From<&UserFillByTime> for
HlFill` field map, `HlMarketDataError::classify`, and nothing else. Each is a branch-free
or two-branch map where the only assertion available would restate an assignment, and
each has a test pinning the behaviour instead. The other functions in the module carry
real assertions: `perps` asserts the universe is non-empty, `fills_between` asserts the
conversion neither drops nor duplicates fills and rejects the zero address. Note that
TB-01's density rule is applied nowhere else in this crate; enforcing it here would have
been box-ticking, and the waivers are recorded rather than hidden.

### Both CP-1 and CP-2 are uncommitted

The working tree holds both checkpoints. They interleave in `universe.rs`, `ws_fills.rs`,
and the hyperliquid module files, so splitting them retroactively needs hunk-level
staging. Committing them before CP-3 is worth doing: CP-3 is the largest commit in the
spec and it deserves a clean base.

---

## CP-3 reconnaissance: the signing model is compatible, and the planned fixture was wrong (2026-09-20)

CP-3 is the atomic dependency swap. It was not started in this session: it is the
largest checkpoint in the spec, it cannot be split, and a half-migrated tree does not
build, so an incomplete attempt is worse than no attempt. Instead its two highest
unknowns were measured in a scratch crate outside the repo, and both came back with
answers that change the work.

### Method

`/tmp/alloy2probe` is a throwaway crate with `hypersdk = "0.2.15"` and
`alloy = { version = "=2.4.2", default-features = false, features = ["signers",
"signer-local", "k256"] }`, containing every call-site shape CP-3 has to write: the
SIWE primitives, the test signers, `place`/`cancel`/`cancel_by_cloid`/`modify`,
`clearinghouse_state`, `open_orders`, `usd_class_transfer`, `NonceHandler`, `Cloid`
construction, `Action::prehash`, `Action::sign_sync`, and the `Fill` conversion.

`/tmp/oldprobe` is a second crate depending on the vendored `hyperliquid-sdk-rs` path,
used to compute the *same* signing digest through the production code path. The two
cannot share a graph, which is the whole reason for the swap.

Neither probe touches the repo. `Cargo.lock` and `vendor/` were verified unchanged
afterwards.

### Finding 1: alloy 2 needs almost no code change

The probe compiled with three errors, all of them in the probe's own code rather than
alloy incompatibilities. `eip191_hash_message`, `Address`, `Signature`,
`recover_address_from_prehash`, and `PrivateKeySigner::{random, address, from_str}` are
unchanged, and a `PrivateKeySigner` from alloy 2 satisfies hypersdk's `SignerSync`
directly.

The single real API delta in existing code: alloy 2's
`Signature::from_bytes_and_parity` is **infallible** and returns `Signature`, not
`Result`. In `services/auth/siwe.rs` that removes one `?` and its error mapping. The
plan had budgeted `siwe.rs` and the `routes/exchanges.rs` test signers as migration
work; the truth is one line in one file and nothing at all in the other.

New surface the rewrite needs: `Cloid` is `pub type Cloid = B128` (`Cloid::from_str`,
`Cloid::ZERO`), `OidOrCloid = Either<u64, Cloid>`, `NonceHandler` is public and
`Send + Sync` so the router now generates its own nonces, `BatchCancelCloid` requires a
`fast` field, and the trigger variant is spelled `TpSl`.

### Finding 2: the domain and Agent encoding are byte-identical

The spec's headline risk was a silent signing-model change producing HTTP 422 on every
order. Measured directly:

| Source | Signing digest for `connectionId de6c4037...` |
|---|---|
| vendored `hyperliquid-sdk-rs` (`Agent { source: "a" }`, production path) | `0x6c43321e0f67bbafa52f8ad0cca68ce5e6700cac04fd5f8ccc1df6d4d3e799b0` |
| `hypersdk` (`signing::agent_signing_hash`) | `0x6c43321e0f67bbafa52f8ad0cca68ce5e6700cac04fd5f8ccc1df6d4d3e799b0` |

Identical. The `Exchange` / `"1"` / `1337` / `0x0` domain and the
`Agent(string source,bytes32 connectionId)` struct encode the same way in both crates.
The risk is closed by measurement, not by argument.

### Finding 3: pin the digest, never the signature

The plan's CP-3 task 7 said to pin the Agent wrapper and domain against the vendored
crate's external vector, `connectionId de6c4037...` / signature `fa8a41f6...`. **That
instruction was wrong and has been corrected.**

Signing that identical digest with that identical key produces a *different* signature
under alloy 2:

```
ours r||s : 5928e9a0b25c0d5c96fa998de2ce6dfee78260ed7388416244895292b34b375b5fe8ea602f3adb8b42d73757b2e6f910e97f8e7bdc1682408d0d0b0d32614fab
their r||s: fa8a41f6a3fa728206df80801a83bcbfbab08649cd34d9c0bfba7c7b2f99340ff53a00226604567b98a1492803190d65a201d6805e5831b7044f17fd530aec78
```

Both recover to the same address. The cause is not a hashing difference; alloy 2 derives
the deterministic ECDSA nonce differently. That is harmless, because an ECDSA signature
over a digest is not unique and the exchange verifies by recovering the signer.

It is not harmless in a test. A fixture pinned to `fa8a41f6...` **fails against a
correct implementation**. Whoever hit that failure would either delete the assertion or
start "fixing" the signing path, and the second option damages a money path. The correct
fixture asserts the digest, which is stable, exchange-verified, and identical across
both crates, plus a recovery check that the signature yields the signing key's address.

Generalisation worth carrying: when pinning cryptography across a dependency bump, pin
the input to the signature, not the signature. A deterministic nonce is deterministic
only within one implementation of the algorithm.

### Finding 4: `Fill` conversion needs explicit wire formatting

hypersdk parses fills into `Decimal`, `Side`, and `FillDirection`, so `HlFill` has to be
rebuilt rather than copied. `normalize().to_string()` for the numeric fields, and the
enum matches are exact because `Side` serialises as `"B"`/`"A"` and `FillDirection` as
`"Open Long"` and friends. Hyperliquid sends normalised numeric strings, so this should
be byte-identical, but that is an assumption: CP-6 should compare one live `raw_json`
against the exchange payload rather than trusting it.

### CP-3 execution decision (2026-09-20)

CP-3 stays atomic. The alternative considered was splitting it into CP-3a (swap plus
read paths, with order placement returning a typed unavailable error) and CP-3b (the
order path), which would have made each half tractable but would land a commit where
trading is disabled. Rejected: nothing should ship in a state where the core function
does not work, even if that commit is never deployed.

Consequence: CP-3 needs an uninterrupted session. It is a writing task against the
verified mapping above, not a discovery task, but it cannot be interleaved with other
work.

Two last measurements close the order-encoding question:

- `decimal_normalized` serialises a `Decimal` as a string with trailing zeros stripped.
  So hypersdk's JSON body and the bytes it hashes agree, which is what Hyperliquid
  requires and what the old crate's `limit_px: String` produced. Consistent by
  construction rather than by argument.
- `normalize_status`, `extract_order_id`, `extract_avg_price`, and `build_order_request`
  are referenced only inside `exchange_api.rs`. No other module sees the SDK's order
  types, so that file can be migrated on its own.

---

## CP-3: Atomic swap to hypersdk (2026-09-20)

**Outcome.** `hyperliquid-sdk-rs` and `[patch.crates-io]` are gone, `vendor/` is deleted
(28 files), and `hypersdk = "0.2.15"` + `alloy = "=2.4.2"` are in. Router tests went 943
passed to 954 passed, 0 failed, 22 ignored. Clippy is unchanged at 5 warnings on the
router bin and 6 on the test bin. `__check.sh` reports zero TB-09 or TB-10 violations on
any added line.

Gates, all run from `testudo-exchange/` with `SQLX_OFFLINE=true`:

| Gate | Result |
|---|---|
| `cargo tree -p router -i hyperliquid-sdk-rs` | `did not match any packages` |
| `cargo tree -p router -i hypersdk` | `hypersdk v0.2.15` |
| `cargo tree -p router -i alloy` | `alloy v2.4.2` |
| `cargo clippy -p router --all-targets` | bin 5, test bin 6 (baseline) |
| `cargo clippy --all-targets` (workspace) | exit 0 |
| `cargo test -p router --bin router` | 954 passed / 0 failed / 22 ignored |
| `cargo test` (workspace) | exit 0, 0 failed |
| `__check.sh` on added lines | no violations |

Changed outside the three adapter files: `Cargo.toml` (×2), `Cargo.lock`,
`services/auth/siwe.rs` (one line plus comment), `services/exchange_api.rs` (one variant),
`routes/trade_management.rs` (two match arms), `types/hl_network.rs`, `market_data.rs`,
and the two `tests/` files.

### The plan's file list was short by two files

`services/exchange_api.rs` and `routes/trade_management.rs` had to move. The typed "not
implemented" transfer error of task 3 is a new `ExchangeApiError` variant, and
`routes/trade_management.rs` matches that enum exhaustively in `format_exchange_error` and
`error_code_for`, so both need an arm. Four lines of real change. Same class of omission
as CP-1's: **any checkpoint that changes a shared enum's shape must carry its match
sites**, and the enum lives outside the directory the plan was watching.

`signal.rs` and the second `trade_management.rs` match have a `_ => false` arm, so they
needed nothing. Worth checking for a catch-all before assuming an enum change is
contained: it is what kept this from being four files.

### The plan's task-7 fixture described a value that cannot exist

Task 7 asked for `Action::Order(fixture).prehash(nonce, None, None, Chain::Mainnet)` to
equal `0x6c43321e0f67bbafa52f8ad0cca68ce5e6700cac04fd5f8ccc1df6d4d3e799b0`, "for the
order fixture described in the verified mapping below". Two things were wrong.

First, the verified mapping never described an order fixture. Second, `0x6c43321e...` is
not a `prehash` of any order: it is the *agent* hash
`agent_signing_hash(Chain::Mainnet, 0xde6c4037...)`, measured in the pre-swap probe to
prove the domain and `Agent` encoding match the vendored crate. `prehash` for an order is
`agent_signing_hash(chain, keccak(msgpack(action) ‖ nonce ‖ 0 ‖ 0))`, so it equals that
value only if the fixture's msgpack hash happens to be `0xde6c4037...`.

Written literally, the assertion would have failed against a correct implementation. That
is the same failure mode as pinning the signature: the fix that makes it pass would be to
change the encoding. It was corrected rather than written in.

What is asserted instead, in `exchange_api.rs::tests`:

1. `agent_signing_digest_matches_the_external_vector` - `agent_signing_hash` equals
   `0x6c43321e...` for connection id `0xde6c4037...`. This is the only value with an
   external reference, and it is the one that closed the spec's 422 risk.
2. `order_signing_input_is_stable` - the fixture order's msgpack connection id
   (`0x80a7fa73...`) and its `prehash` (`0x5262c55d...`) against measured goldens, plus
   `prehash == agent_signing_hash(chain, connection_id)`.
3. `signed_order_recovers_to_the_signing_key` - the property the exchange actually checks.

Together 1 and 2 pin the whole chain without either being circular: 1 proves the outer
`Agent`/domain layer against an independent reference, 2 locks the inner msgpack layer.
There is no independent reference for the inner layer, because the old and new crates
encode the order body differently by design. CP-6 is the authority for that, not a test.

### `Cloid` and `OidOrCloid` are re-exported one level up

`hypersdk::hypercore::types` has `use crate::hypercore::{Chain, Cloid, OidOrCloid, ...}`
as a private import, so `types::Cloid` is a privacy error even though `types::OrderRequest`
is public. Both aliases must be imported from `hypersdk::hypercore`. Three compile errors
came from this, which is the cheap kind.

### The `0x` prefix repair is gone, and so is string comparison

The old crate's `with_cloid(Uuid)` produced unprefixed hex, so `place_order` and
`place_trigger_order` each carried a `if !s.starts_with("0x")` repair, and
`find_oid_by_cloid` compared three string spellings of the same cloid for compatibility.
`Cloid` is `B128`, so `format!("{c:#x}")` is the only encoding serde can produce and
`find_oid_by_cloid` is now `order.cloid == Some(wanted)`. Both repairs and the
three-way comparison are deleted.

`cloid_from_uuid` is the single conversion, and `cloid_uuid_is_the_big_endian_hex_hyperliquid_expects`
pins it against the `cloid:{:032x}` order-id format this module still emits, because those
two representations have to agree or a cancel by cloid never finds its order.

### `format_sz` became `round_sz` and returns a `Decimal`

`hypersdk` serialises `sz` and `limit_px` with `decimal_normalized`, which strips trailing
zeros, so the wire form of a rounded quantity is just the `Decimal`. Returning a `String`
and re-parsing it would have been a round trip through the same value. `round_sz` returns
the `Decimal`; the wire bytes are identical to the old path.

Tick rounding is unchanged, as FR-4 requires: `(limit_px / tick).round_dp(0) * tick` with
`tick_size_from_mid`'s existing bands. The one edit to that function is that it now takes
a `Decimal` instead of a `&str`, so the `f64` parse and its `unwrap_or(0.0)` are gone.
`PerkMarket::round_price` was not adopted.

### CP-2's prediction of a typed rate-limit status was wrong

CP-2's notes said "When `hypersdk` arrives in CP-3 it exposes a typed status, and this is
the one place that has to change." It does not. `hypersdk`'s transport error is
`ApiError(pub String)`; the status codes are not modelled. `HlMarketDataError::classify`
keeps its substring predicate unchanged, and the comment now says why.

### Known gap carried forward: `Fill` has two fields `HlFill` does not

`hypersdk`'s `Fill` has `builder_fee` and `liquidation`, which the vendored crate's
`UserFillByTime` did not. `HlFill` still has fifteen fields, so both are dropped before
`raw_json` is written. That is not a regression - the old path dropped them too - and FR-4
says behaviour stays put, so nothing changed here.

But the wire-shape test only proves the drop is harmless when both are `None`, and
`liquidation` is not always `None`: a liquidated position produces fills that carry it.
The router has no builder, so `builder_fee` should stay absent; `liquidation` is the real
one. Either `HlFill` grows two fields or the column is knowingly lossy. It is recorded
here rather than silently left, and CP-6 should check one liquidation fill if it can. This
is a new spec, not CP-6 scope.

### Assertions added, and where density was deliberately not chased

`transfer_usdc` gained one `debug_assert!` on the resolved query address, `place_order`
one on the single-order batch yielding at most one status, and `place_trigger_order` one
on surfacing a per-status error as a warning. The crate does not enforce TB-01 density
anywhere else and CP-2 already recorded why box-ticking it is worse than not: several of
these functions are branch-free field maps whose only available assertion restates an
assignment. Every new assert here names an invariant that a caller could actually break.

---

## CP-4: Spot and perp transfer on usd_class_transfer (2026-09-20)

**Outcome.** `transfer_usdc` now calls `Client::usd_class_transfer` for both directions,
with a new `parse_transfer_amount` helper that returns a decimal USDC value and rejects
zero, negatives, and unparseable input. `ExchangeApiError::TransferNotImplemented` and
both of its match arms in `routes/trade_management.rs` are gone. Three tests were added.
Tests went 954 to 957 passed, clippy stayed at 5 warnings on the router bin, and
`__check.sh` reported zero violations on added lines.

### The transfer was already dead code, and the spec is wrong about it

`transfer_usdc` has no caller anywhere in the repository. Not in Rust, not in the
journal, not in the extension:

```
$ grep -rn "transfer_usdc" --include=*.rs --include=*.ts --include=*.tsx .
./testudo-exchange/crates/router/src/services/hyperliquid/exchange_api.rs:149
```

That single hit is the definition. There is no transfer route, and the `ExchangeApi`
trait has no transfer method (`get_balance`, `place_order`, `amend_order`,
`cancel_order`, `cancel_all_orders`, and nothing else). Neither `testudo-journal/src`
nor `testudo-extension/src` contains the word "transfer" at all.

This is not a CP-3 regression. At revision `434e912a`, before the swap,
`git grep -l transfer_usdc 434e912a` also returns only the file that defines it.

So the spec's problem statement, "the journal and extension still offer spot-to-perp
transfer, which cannot succeed", is not supported by the code. There is no UI control
and no reachable backend path. The spec's second `[CLARIFY]`, about hiding a
known-broken control, is moot for the same reason. `FR-3` is still satisfied literally:
the transfer is implemented against `usd_class_transfer`, and the testnet criteria in
CP-6 can exercise it once something calls it.

### `signatureChainId` is hex, and the first test expectation was wrong

The red step caught this because the test asserted the value instead of assuming it:

```
assertion `left == right` failed
  left: String("0xa4b1")
 right: "42161"
```

`Chain::arbitrum_id()` displays as `0xa4b1`, which is 42161 in hex. That is
Hyperliquid's documented form for `signatureChainId`, and it matches what this repo's
own `agent_approval.rs` builds for MetaMask-signed approvals
(`format!("0x{:x}", signature_chain_id)`) and what the spec quotes from `hypersdk`'s
source. The vendored crate's `UsdSend` used a decimal `signature_chain_id: u64`, so
this is a real difference between the old and new wire forms for the same field. There
is no working baseline to preserve either way: the spot to perp direction was rejected
outright, and the perp to spot direction sent a different action entirely. CP-6 should
confirm the exchange accepts the hex form on testnet.

### The red step was worth running for the units alone

`parse_transfer_amount` was written first with the old micro-unit conversion in place,
so the test failed against it rather than against a missing function:

```
assertion `left == right` failed
  left: 12500000
 right: 12.5
```

That is the highest-severity hazard in the migration demonstrated rather than asserted
in prose. A compile error would not have proven the guard bites; this does.

### The plan's CP-4 verification grep was over-broad

The criterion said `grep -rn "ClassTransfer\|spot_transfer_to_perp\|1_000_000"` would
produce nothing. It produces eight hits, all benign:

- `hl_fill_journal.rs:107`, `((ms % 1000) * 1_000_000)`, milliseconds to nanoseconds.
- `cex_history.rs:1140`, `trade.transact_time_ns / 1_000_000`, nanoseconds to
  milliseconds.
- This file's own comments explaining the hazard, and the identifiers
  `UsdClassTransfer`/`usdClassTransfer`, which are hypersdk's names and required.

The intent was "no micro-unit conversion of a transfer amount". The corrected check is
`grep -n "\* *Decimal::from(1_000_000\|trunc()" ...`, which is empty, and `\bClassTransfer\b`,
which matches only a doc comment (the word boundary does not fall inside
`UsdClassTransfer`). Recorded in the plan so the next checkpoint does not write a
criterion that cannot pass.

### Two TB-01 waivers, recorded rather than chased

`parse_transfer_amount` and `transfer_usdc` carry `// @waiver TB-01`. The first is a
parse plus a range check with no branch that can fail without returning an error; the
second already asserts a non-zero query address and a positive amount, and a third
assertion would restate one of those. The unit that matters is pinned by a test.
