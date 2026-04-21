# QNT-01c Learnings

### 2026-04-21 — T1 (TS types)

- **`bun run build` IS the type gate for testudo-journal** — no separate `typecheck` script exists in `package.json`. Build exit 0 is the acceptance criterion for type correctness (contrast with `testudo-extension` which uses `bun run typecheck`).
- **`JournalTrade` interface is in `src/api/client.ts:192-235`** (post-T1 line offsets shift by 18 for the new `KellyInputs` type block). Consumers (`TradeWithTags`, `TradeDetail`) extend it via `extends JournalTrade` — `kelly_inputs` propagates to them automatically without additional edits.
- **`KellyInputs` fields are `number`, not `string`** — unlike `JournalTrade.realized_pnl: string` (Decimal-as-string convention), the kelly_inputs JSONB blob uses raw JSON numbers. The QNT-01a T7 Rust serializer uses `serde_json::json!({...})` with `Decimal` values coerced to `f64` in the JSON blob. Type accordingly.

### 2026-04-21 — T2 (backend passthrough + row badge)

- **FR-2 (backend passthrough) was already satisfied by QNT-01a**: `list_trades` uses `SELECT jt.*` and `get_trade` uses `SELECT *` — both pick up `kelly_inputs` from the DB column automatically. The Rust `JournalTrade` struct has `pub kelly_inputs: Option<serde_json::Value>` since QNT-01a T7. No Rust changes needed.
- **Badge placed before the notes icon in the TAGS column cell**: keeps visual order Kelly badge → notes icon → tag pills. Uses `e.stopPropagation()` so clicking doesn't fire the row's `onClick` (TradeDetail). The `onKellyBadgeClick?: () => void` optional prop is a no-op in T2, wired to modal in T3.
- **`border-signal-green/40` / `text-signal-green/80` with hover to full**: matches signal-green palette from RSK-01 (ExchangeCard heartbeat dots). Opacity variants keep badge visually subordinate to P&L data.
