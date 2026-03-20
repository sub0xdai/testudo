# Quality Checklist — EXT-31 Bracket Order Safety (Rev 2)

**Spec ID:** EXT-31-bracket-order-safety
**Date:** 2026-03-14

## Implementation

- [ ] `create_trade` passes `stop_loss_trigger: None` and `take_profit_trigger: None`
- [ ] FillDetectorService `FillAction` struct has deferred placement fields
- [ ] Entry fill branch populates `stop_loss_price`, `take_profit_price`, `entry_fill_price`, `entry_quantity`
- [ ] `FillKind::Entry` handler places SL order (stop-market, reduce-only)
- [ ] `FillKind::Entry` handler places TP order (limit, not reduce-only)
- [ ] Close side inferred correctly from SL/TP price vs entry price
- [ ] `clientOrderId` stamped on deferred SL/TP orders
- [ ] Exchange order IDs registered via `register_exchange_order_id`

## Error Handling

- [ ] SL failure logs CRITICAL with group ID
- [ ] TP failure logs warning (non-critical)
- [ ] Shadow trades unaffected (no exchange API calls)
- [ ] Entry cancellation before fill → no SL/TP placed

## Testing

- [ ] New test: entry fill triggers deferred SL/TP placement
- [ ] New test: no SL/TP placed when prices absent
- [ ] All existing fill_detector tests pass
- [ ] `cargo clippy --all-targets` clean
- [ ] `cargo test` all pass

## Verification

- [ ] `cargo clippy --all-targets && cargo test` passes
- [ ] Extension build unaffected
