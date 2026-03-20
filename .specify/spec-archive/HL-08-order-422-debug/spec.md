# HL-08: Debug Hyperliquid Order HTTP 422 Deserialization Error

**Status:** In Progress
**Priority:** P0
**Created:** 2026-03-19
**Depends on:** HL-07 (completed)

## Context

After fixing the agent address parameter (HL-07 fix #8) and enabling Hyperliquid-only mode (no CEX sidecar required), orders now reach Hyperliquid but are rejected with HTTP 422: "Failed to deserialize the JSON body into the target type."

Debug logging was added to dump the order params before the SDK call. Restart the backend and attempt a trade — the logs will show exactly what's being sent.

## What We Know

### Confirmed Working
- Balance fetch (clearinghouseState API): OK
- Agent wallet approval (EIP-712): OK
- Auth mode: Agent wallet with correct user_address passed to SDK
- `build_exchange()` now passes `*user_address` (not `auth.address`) to `ExchangeProvider::mainnet_agent()`
- All 959 tests pass

### The Error
```
HTTP error: status 422, body: Failed to deserialize the JSON body into the target type
```
This is from Hyperliquid's exchange endpoint (POST https://api.hyperliquid.xyz/exchange).

### SDK Request Flow (hyperliquid-sdk-rs v0.1.2)
1. `place_order(&order)` → wraps in `BulkOrder { orders: [order], grouping: "na" }`
2. `send_l1_action("order", &bulk_order)` constructs:
   - `connection_id` = keccak256(msgpack(order) + nonce_bytes + vault_byte)
   - Agent L1 action with `source: "a"` (mainnet), `connection_id`
   - EIP-712 signature over Agent action with **chain_id=1337** (HL L1 domain, NOT Arbitrum)
3. Wraps in agent action:
   ```json
   {
     "type": "agent",
     "agentAddress": "0x{user_wallet_address}",
     "agentAction": { "type": "order", "orders": [...], "grouping": "na" },
     "source": "a"
   }
   ```
4. Final POST body:
   ```json
   {
     "action": <agent_action>,
     "signature": { "r": "0x...", "s": "0x...", "v": 27 },
     "nonce": <timestamp_ms>,
     "vaultAddress": null
   }
   ```

### Suspected Root Causes (in order of likelihood)

1. **`vaultAddress: null` vs omitted**: The SDK serializes `self.vault_address` (which is `None`) as JSON `null`. Hyperliquid may reject `"vaultAddress": null` but accept a missing field. Check if `skip_serializing_if = "Option::is_none"` is applied.

2. **`source` field type mismatch**: Agent action has `"source": "a"` as a string. Hyperliquid may expect it as a different type or key name. Compare against Python SDK's `approve_agent` source format.

3. **CLOID format**: Our orders include a CLOID (client order ID) formatted as `"{:032x}"` (32-hex UUID). Hyperliquid requires CLOIDs to be exactly 32 hex characters starting with `0x`. Check if the SDK's `with_cloid` format matches expectations.

4. **Order field serialization**: The `OrderRequest` uses `#[serde(rename_all = "camelCase")]` at the struct level but also individual `#[serde(rename = "a")]` etc. If `rename_all` interferes with the manual renames, field names could be wrong.

5. **`agentAddress` format**: SDK uses `format!("{:#x}", agent_address)` which produces `0xabcd...` (lowercase). Hyperliquid may expect checksummed address or different format.

## Investigation Steps

### Step 1: Read the debug logs
Restart backend and place a trade. The new logging will show:
```
HyperliquidExchangeApi: placing order coin=BTC asset_index=0 is_buy=true limit_px=70447 sz=0.00536 ...
```
Verify the values make sense.

### Step 2: Test SDK directly
Create a minimal test that bypasses our code and calls the SDK directly:
```rust
let signer = "agent_private_key".parse::<PrivateKeySigner>()?;
let exchange = ExchangeProvider::mainnet_agent(signer, user_wallet_address);
let order = OrderRequest::limit(0, true, "70000", "0.001", "Gtc");
let result = exchange.place_order(&order).await;
```
If this fails too, the issue is in the SDK. If it works, the issue is in how we construct the order.

### Step 3: Patch SDK to log request body
Temporarily modify `~/.cargo/registry/src/.../hyperliquid-sdk-rs-0.1.2/src/providers/exchange/mod.rs` at the `post()` function (line ~1720) to print the full JSON payload before sending:
```rust
eprintln!("HL REQUEST: {}", serde_json::to_string_pretty(&payload).unwrap());
```

### Step 4: Compare against Python SDK
Run the Python SDK example with the same agent wallet to verify the request format:
```python
agent_exchange = Exchange(agent_wallet, MAINNET_API_URL, account_address=user_wallet)
result = agent_exchange.order("BTC", True, 0.001, 70000.0, {"limit": {"tif": "Gtc"}})
```

### Step 5: Check vaultAddress serialization
In SDK `post()` function (line 1719):
```rust
"vaultAddress": self.vault_address,  // Option<Address> → null when None
```
If this is the issue, the fix is either:
- Add `#[serde(skip_serializing_if = "Option::is_none")]` to the payload construction
- Or serialize manually without the field when None

## Key Files
- **Our exchange API**: `testudo-exchange/crates/router/src/services/hyperliquid/exchange_api.rs`
- **Our auth**: `testudo-exchange/crates/router/src/services/hyperliquid/auth.rs`
- **SDK exchange provider**: `~/.cargo/registry/src/.../hyperliquid-sdk-rs-0.1.2/src/providers/exchange/mod.rs`
- **SDK order types**: `~/.cargo/registry/src/.../hyperliquid-sdk-rs-0.1.2/src/types/requests.rs`
- **SDK actions**: `~/.cargo/registry/src/.../hyperliquid-sdk-rs-0.1.2/src/types/actions.rs`

## Environment
- **Network:** Mainnet
- **Auth mode:** Agent wallet
- **Wallet:** 0xC285F922116959Db9eAF9f07729faBB7370A5b36
- **Balance:** ~$85 USDC
- **SDK version:** hyperliquid-sdk-rs 0.1.2
- **Order attempted:** BTC_USDT limit buy, qty ~0.005, price ~$70,447

## Session Progress (Mar 19)

### Completed This Session
1. **build_exchange agent address fix** — passed `*user_address` instead of `auth.address`
2. **Cancel ghost positions** — expanded fallback to `!status.is_terminal()`
3. **Skip HL in reconciliation sweep** — avoids CEX sidecar errors
4. **Hyperliquid-only mode** — `live_exchange_api` no longer requires CEX sidecar
5. **Audit: 4 route handler fixes** — add_account validation, positions, close-position, supported-exchanges all have HL-native paths now
6. **Debug logging added** — order params logged before SDK call
