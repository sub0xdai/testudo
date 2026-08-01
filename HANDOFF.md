# Handoff — Hyperliquid Spot/Perp Transfer + WalletConnect Fixes

**Date:** 2026-08-01 07:35 UTC
**Project:** testudo
**Next focus:** Fix Hyperliquid spot→perp transfer; use Python SDK's `usd_class_transfer` as reference implementation

---

## Summary

Implemented Hyperliquid spot/perp balance display and transfer functionality across backend (Rust router), journal (Solid.js exchange cards), and extension (Solid.js popup). Also fixed WalletConnect/Reown onboarding by wiring AuthContext into WalletConnectFlow and fixing a deploy script env-var override bug. The transfer endpoint is functional for perp→spot (`usd_transfer`), but spot→perp is blocked by a Rust SDK bug: `spot_transfer_to_perp` in hyperliquid-sdk-rs v0.1.2 uses `send_l1_action` which omits `nonce`/`hyperliquidChain`/`signatureChainId` from the action body, and the `ClassTransfer` struct field `usd_size` serializes to `usdSize` instead of `usdc`.

---

## Current State

| What | Status |
|------|--------|
| WalletConnect projectId fix (deploy script) | ✅ Done |
| WalletConnectFlow uses AuthContext | ✅ Done |
| Backend: spot+perp balance query | ✅ Done |
| Backend: transfer endpoint with account validation | ✅ Done |
| Backend: perp→spot transfer (usd_transfer) | ✅ Working |
| Backend: spot→perp transfer | 🔴 Blocked by SDK bug |
| Journal: ExchangeCard transfer UI | ✅ Done (published) |
| Extension: transfer UI | ✅ Done (not yet deployed) |
| SDK patch: ClassTransfer field rename | 🔄 Applied on droplet, not sufficient |

---

## Key Decisions

- **Used `usd_transfer` for perp→spot**: This SDK method works because it goes through `send_user_action` which properly includes chain fields.
- **Did NOT implement raw EIP-712 signing**: Attempted, but it's error-prone. The Python SDK reference shows the correct approach is `usd_class_transfer`.
- **SDK patch approach**: Patched `ClassTransfer.usd_size` → `usdc` on the droplet, but `send_l1_action` still doesn't add `nonce`/chain fields — the patch alone isn't enough.

---

## The Fix (from Python SDK research)

The Python SDK's `usd_class_transfer(amount=50.0, to_perp=True)` works for both directions. The Rust SDK has `usd_class_transfer` at line 954 but it also uses `send_l1_action` — same bug.

**Recommended fix path**: Replace `transfer_usdc` to make a direct HTTP POST to `https://api.hyperliquid.xyz/exchange` with a properly formatted `usdClassTransfer` action that includes `hyperliquidChain`, `signatureChainId`, and `nonce`, signed with the agent's key. The Python SDK's working implementation can be referenced in the hyperliquid-python-sdk repo.

Alternatively: implement `send_asset` with `sourceDex: "spot"`, `destinationDex: ""` (default perp DEX) as the Hyperliquid docs describe.

---

## Artifacts

| Artifact | Path | Description |
|----------|------|-------------|
| Backend routes | `testudo-exchange/crates/router/src/routes/exchanges.rs` | Balance endpoint split spot/perp; transfer_funds handler with account validation |
| HL exchange API | `testudo-exchange/crates/router/src/services/hyperliquid/exchange_api.rs` | `transfer_usdc` method, `transfer_to_perp_raw` (unused) |
| Types | `testudo-exchange/crates/router/src/types/exchanges.rs` | TransferRequest/TransferResponse |
| App state | `testudo-exchange/crates/router/src/types/app.rs` | Added `hl_exchange_api` field |
| Router main | `testudo-exchange/crates/router/src/main.rs` | HL API refactored, transfer route added |
| Journal card | `testudo-journal/src/components/account/ExchangeCard.tsx` | Spot/perp display + transfer UI with direction toggle |
| Journal API | `testudo-journal/src/api/exchange.ts` | `transferFunds` method |
| WalletConnect flow | `testudo-journal/src/components/account/WalletConnectFlow.tsx` | Now uses `useAuth()` context |
| Extension transfer | `testudo-extension/src/popup/components/MainView.tsx` | Transfer UI in Account tab |
| Extension BG | `testudo-extension/src/background/api.ts`, `handlers.ts` | `transferFunds` + handler |
| Deploy script | `scripts/deploy.sh` | Fixed env-var override; SDK patch added |
| Droplet patch | `/root/.cargo/registry/.../hyperliquid-sdk-rs-0.1.2/src/types/actions.rs:246` | `usd_size` → `usdc` |
| Droplet patch | `/root/.cargo/registry/.../hyperliquid-sdk-rs-0.1.2/src/providers/exchange/mod.rs` | Parameter rename in `spot_transfer_to_perp` |
| Previous handoff | `HANDOFF.md` (this file) | Pre-existing CLIs handoff |

---

## Suggested Skills

| Skill | Relevance | Invocation |
|-------|-----------|------------|
| audit | Review the transfer implementation for security/safety | `/skill:audit` |
| graphify | Map the transfer code paths across backend/frontend | `/skill:graphify .` |

Also consider:
- **Read first**: `testudo-exchange/crates/router/src/services/hyperliquid/exchange_api.rs` lines 146-190
- **Read first**: Python SDK reference: https://github.com/hyperliquid-dex/hyperliquid-python-sdk
- **Droplet**: SSH `root@n0x-server`, `cd /opt/testudo && bash scripts/deploy.sh`
- **Git state**: `master`, clean, up to date with remote

---

## Open Questions / Blockers

- **Spot→perp 422**: SDK `send_l1_action` doesn't include chain fields. Need to implement raw HTTP call or fix SDK.
- **Extension not deployed**: Only journal was deployed via `deploy.sh`. Extension needs separate build+deploy.

---

## Redactions

- (None)
