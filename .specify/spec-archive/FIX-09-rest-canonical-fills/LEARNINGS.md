# FIX-09 LEARNINGS

## 2026-04-28 (CP-1)

**Production DB users schema differs from migrations.** The migration `20250922164541_users.up.sql` defines `email` + `password_hash` columns, but the live production DB uses `wallet_address VARCHAR(48)` with a regex CHECK constraint (`0x[0-9a-f]{40}` or Base58). DB integration tests must INSERT a valid Ethereum address, not email/password. Use a UUID-derived hex address: `format!("0x{:02x}...{:02x}0000000000000000", uid_bytes[0..15])`.

**RED test confirmed at line 1095.** Failure message: `exit_price = 2419, needs_reconciliation = false` — exactly the SL trigger price from WS. The `upsert_daily_stats` post-commit path runs successfully on exit_price=2419 (non-zero → treated as a win/loss trade). Cleanup order: journal_daily_stats → journal_trades → trade_events → users.

**Pre-existing test failure:** `routes::auth::tests::test_me_returns_user_info` was already failing on master before this spec — unrelated to FIX-09.

**CP-2 ordering is critical.** The shortcut at fill_detector.rs:430,472,532 currently masks the latent FR-3 bug (reconciler returning entry avg as exit price). Once CP-3 removes the shortcut (always sends exit_price=0 → always calls reconciler), CP-2's `CloseCandidates` entry-exclusion must already be in place. CP-2 MUST be committed before CP-3 or the production reconciler will write entry_avg as exit_price for every trade.

## 2026-04-28 (CP-5)

**safe-cex has no `fetchMyTrades`.** The spec pseudocode uses CCXT's `exchange.fetchMyTrades()`, but the sidecar uses safe-cex which doesn't expose it. Implemented by calling Bybit's `/v5/execution/list` directly via `(exchange as any).xhr.get()` — same pattern as `handleFetchOrder`. Non-Bybit exchanges return 501.

**Bybit execution list fields:** `side` is capitalized ("Buy"/"Sell"), `execPrice` is the fill price, `execQty` is quantity filled, `execTime` is ms timestamp as a string, `orderId` is the order ID. Filter by lowercase close_side and `Math.abs(execQty - expected_qty) <= tolerance`.

**Parent repo commit bundles CP-4 submodule bump with CP-5.** The exchange submodule had CP-4 committed but the parent hadn't bumped yet. The CP-5 parent commit covered both (bumped submodule from CP-3 → CP-5 pointer in one go).

## 2026-04-28 (CP-7)

**Sidecar test suite had 5 failures, 3 caused by FIX-09 CP-3.** `ws-fills.test.ts` and `integration.test.ts` asserted on `filled`/`remaining` fields stripped in CP-3. Updated to assert `toBeUndefined()` per new FR-1 contract. 2 remaining failures (`POST /balance`, `POST /position`) are pre-existing from commit `7a32dee` (leverage added to handler response without updating tests) — unrelated to FIX-09.

**Reproducer (FR-10) remains GREEN through CP-7.** `pick_close_leg` entry-exclusion + qty/time gating + REST canonical avg = exit_price matches actual fill, not SL trigger.

**T41 backfill deferred to live session.** No DATABASE_URL available in autonomous build context. Backfill of the 2026-04-27 ETHUSDT trade should be done via: flip `needs_reconciliation = TRUE` for the row and let the reconciler sweep correct it in production.
