# Next Session: Fix EXT-31 Bracket Order Rejection

## Problem
After implementing EXT-31 (bracket order safety), live trade placement fails with:
**"Order rejected by exchange — check server logs for details"**

The exchange (WOO X) is rejecting the new bracket order format where `stopLoss` and `takeProfit` are attached to the entry `createOrder` call.

## What Was Changed (commit 1e2670d)
EXT-31 replaced three sequential exchange calls (entry, SL, TP) with a single CCXT `createOrder` call using attached `stopLoss`/`takeProfit` parameters:

```javascript
exchange.createOrder('BTC/USDT:USDT', 'limit', 'sell', qty, price, {
  stopLoss: { triggerPrice: slPrice },
  takeProfit: { triggerPrice: tpPrice },
})
```

## Investigation Steps
1. **Check backend logs** — `cargo run --bin router` output will show the actual exchange error message from CCXT
2. **Check sidecar logs** — the CCXT sidecar on port 3100 may log the raw exchange rejection
3. **Possible causes:**
   - WOO X may not support attached SL/TP on limit orders (only on market orders?)
   - The `triggerPrice` format may need to be a number, not a string
   - WOO X may require additional params like `stopLoss.type` or `takeProfit.type`
   - The CCXT version (4.5.39) may handle WOO bracket orders differently than documented
4. **Test on WOO testnet first** if possible

## Files to Check
- `testudo-ccxt/src/handlers.js` — sidecar forwards bracket params to CCXT
- `testudo-exchange/crates/router/src/services/ccxt_client.rs` — builds request body
- `testudo-exchange/crates/router/src/routes/trade_management.rs` — bracket call at ~line 823

## Fallback Plan
If WOO X doesn't support attached bracket orders, revert to **deferred placement** strategy:
1. Place only entry order on submission
2. Wait for entry fill via WebSocket (FillDetectorService already handles this)
3. Place SL/TP only after entry fill confirmation
4. This uses existing infrastructure (fill detector, OCO logic) with minimal new code

## Spec
`.specify/specs/EXT-31-bracket-order-safety/spec.md`
