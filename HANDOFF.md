# Handoff

**Date:** 2026-09-18
**Project:** testudo
**HEAD:** `0c342402` (pushed; droplet at `434e912a`, one commit behind)
**Next focus:** HL-12, the migration off `hyperliquid-sdk-rs`. Spec written, awaiting `/vox plan`.

---

## Summary

Three strands landed today, in this order.

1. **TS-01 Phase 1 + UC-1 + UC-3 + the backlog sweeper.** A server-side TypeSafe (Jev) client, setup-tag resolution on the pre-trade path, journal note attribution on the post-close path, an audit log, and a recovery sweeper. Off by default; `TYPESAFE_ENABLED=false`.
2. **Production recovery.** `testudo-api` had been down since 2026-09-16 10:09 UTC with `target/release/router` missing. Two independent failures were stacked: an incomplete registry patch (`E0560`) and a migration checksum divergence that would have caused a second outage on restart.
3. **Dependency hygiene.** The registry `sed` is gone; the SDK is vendored with a one-line fix and a regression test. A spec exists for leaving that crate entirely.

---

## Current State

| What | Status |
|------|--------|
| TS-01 transport, UC-1, UC-3, migration, sweeper | Done, 938 tests pass |
| `TYPESAFE_ENABLED` | `false` in production. Client absent, sweeper not spawned |
| `judgment_*` columns on `journal_trades` | Applied. 169 rows at `not_attempted` |
| `testudo-api` / `testudo-ws` / `testudo-cex` | active, health 200 |
| `hyperliquid-sdk-rs` | Vendored at `vendor/`, one-field patch, `[patch.crates-io]` |
| Spot-to-perp transfer | **Still broken.** Wrong action; see below |
| HL-12 hypersdk migration | Spec written, not planned or built |

---

## The 2026-09-16 Outage

`target/release/router` and `ws-stream` did not exist, so systemd returned `203/EXEC` every 5 seconds. The restart counter reached **36,315** over roughly 49 hours before anyone looked.

Two causes, both now fixed:

**1. Incomplete registry patch.** `deploy.sh` renamed the `ClassTransfer` struct field but not its field-init call site, so `ClassTransfer { usd_size, to_perp }` referenced a field that no longer existed. `E0560` on every build.

**2. A second outage waiting behind the first.** The production database had been migrated with a droplet-only, uncommitted edit to `20260530000001_agent_key_audit_trail.up.sql` (guarding `ALTER TABLE trade_groups`, which does not exist in prod). sqlx records a SHA-384 of the up-migration, and `main.rs` calls `std::process::exit(1)` on a migration error with `Restart=always` behind it. The recorded checksum was `8ae16236...`, matching the droplet file and not the committed `58acfec3...`, so any binary built from the repo would have aborted on startup. Committed the deployed version.

Diagnostic method worth reusing: for an unmodified neighbouring migration, worktree checksum == committed checksum == database checksum. That control validates the method before you trust a mismatch.

---

## Correction to the Previous Handoff

The previous version of this file recorded that the Rust SDK's `send_l1_action` "omits `nonce`/`hyperliquidChain`/`signatureChainId` from the action body". **That is wrong about `nonce`.** Both `send_l1_action` and `send_user_action` end in `self.post(action_value, signature, nonce)`, which places `nonce` at the top level of the payload, where Hyperliquid expects it.

The real defect is narrower and still stands: the *action body* for `spotUser`/`classTransfer` lacks `hyperliquidChain` and `signatureChainId`. `ClassTransfer` carries only an amount and a direction. Corroborated by `hypersdk`, whose `UsdClassTransferAction` includes `signature_chain_id`, `hyperliquid_chain`, and `nonce`.

This matters because the previous note sent us toward the wrong fix. Do not patch `ClassTransfer` and call the transfer solved.

---

## Key Decisions

- **`Option<Arc<dyn SystemOneClient>>` on `AppState` is the feature flag.** No boolean to forget, and every judgement degrades to a null payload rather than an error.
- **Retry splits by path.** Pre-trade never retries: a 500 ms budget cannot absorb a `retry-after`. Post-trade retries with jittered backoff.
- **A transient failure leaves the row queued** (`not_attempted`), not `failed`. That is what makes the sweeper a retry loop instead of a one-hop loss.
- **Vendoring over registry patching.** Production must build the same dependency tree as every other environment.
- **The vendored fix is behaviour-preserving.** The `usdc` serde rename matches what prod already ran. Migrating to `hypersdk` is the behavioural fix, and it is a separate spec.

---

## Artifacts

| Artifact | Path |
|----------|------|
| TypeSafe client, wire types | `testudo-exchange/crates/router/src/services/typesafe/{client,types}.rs` |
| UC-1 setup tag resolution | `.../services/typesafe/service.rs` |
| UC-3 note attribution | `.../services/typesafe/attribution.rs` |
| Backlog sweeper | `.../services/typesafe/sweep.rs` |
| Judgement routes | `.../routes/judgment.rs` |
| UC-3 migration | `.../sqlx_postgres/migrations/20260605000000_judgment_attribution.{up,down}.sql` |
| Vendored SDK | `testudo-exchange/vendor/hyperliquid-sdk-rs/` |
| Wire-shape regression test | `.../services/hyperliquid/exchange_api.rs` (`class_transfer_serializes_*`) |
| Deploy script | `scripts/deploy.sh` |
| HL-12 spec | `.specify/specs/HL-12-hypersdk-migration/spec.md` |
| Design doc for TS-01 | `docs/plans/typesafe-jev-sniper-integration.md` |

---

## Open Items

**HL-12 (next).** Spec at `.specify/specs/HL-12-hypersdk-migration/spec.md`. Per `.specify/WORKFLOW.md` the next step is `/vox plan` then the advisor gate. Three `[CLARIFY]` items are unresolved, and one of them gates the whole spec: whether a funded Hyperliquid testnet account with an approved agent wallet exists. Without it the transfer fix cannot be verified, which is the main reason to migrate.

**Known-broken UI.** The journal and extension still offer spot-to-perp transfer, which cannot succeed. Either hide it until HL-12 lands or accept that it is a live control that always fails.

**No liveness alert.** The API was down for 49 hours and nothing told anyone. A health check on `testudo-api` that pages would have caught it in minutes. Worth doing before the next migration, not after.

**Pre-existing, unrelated.** `./uploads/journal` does not exist relative to `WorkingDirectory=/opt/testudo/testudo-exchange`, so actix-files logs an error at every startup and journal image uploads will fail. Disk on the droplet is 85% full (12 GB free).

**Scrub hook.** Fixed in this clone: `vendor/` excluded, and `set -e` removed so the hook's own `case $?` can handle scrub.py's exit code. The canonical version of that hook has the same bug and no path exclusion, so other projects scaffolded from it will silently rewrite files. Worth fixing at the source.

---

## Redactions

None.

---

## Environment Notes

- Build the router locally with `SQLX_OFFLINE=true`. `db-processor` uses `sqlx::query!` and there is no local Postgres on `localhost:5000`.
- The droplet builds without it by finding `/opt/testudo/.env` through dotenvy's parent-directory walk.
- Deploy: `ssh n0x 'cd /opt/testudo && bash scripts/deploy.sh'`. Full compiler output now goes to `/tmp/testudo-build.log` and the last 40 lines print on failure.
- Do not use `pkill -f "cargo build"` over SSH. It matches the SSH command's own argv and kills the session.
