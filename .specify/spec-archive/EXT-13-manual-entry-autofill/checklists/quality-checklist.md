# Quality Checklist — EXT-13: Manual Entry with Auto-Fill

> Spec: EXT-13-manual-entry-autofill
> Date: 2026-02-14

## Code Quality

- [ ] No `unwrap()` or unchecked type assertions on user input
- [ ] All number inputs validated (positive, finite, not NaN)
- [ ] No hardcoded platform URLs outside manifest.json
- [ ] Shared TradeForm component used by both modal and popup (DRY)
- [ ] Modal styles remain self-contained in Shadow DOM (no Tailwind leakage)
- [ ] No new npm dependencies added

## Testing

- [ ] Unit tests for TradeForm validation logic
- [ ] Unit tests for reactive R:R recalculation
- [ ] E2E test: auto-fill flow (scraper success → pre-filled form → execute)
- [ ] E2E test: manual entry flow (scraper fail → empty form → type values → execute)
- [ ] Existing background.test.ts passes unchanged

## Security

- [ ] Input sanitization on symbol field (no script injection via symbol name)
- [ ] Number parsing uses strict validation (not just `parseFloat`)
- [ ] No new host_permissions beyond explicitly listed charting platforms

## UX

- [ ] Auto-filled fields visually distinguishable from empty fields
- [ ] Tab order is logical: symbol → side → entry → stop → target
- [ ] Focus management: first empty field gets focus on modal open
- [ ] Error states: invalid fields show visual indicator, not just disabled button
- [ ] Popup Quick Trade is accessible without navigating to a charting site

## Backward Compatibility

- [ ] Scraper strategies 0-5 completely untouched
- [ ] background.ts zero changes
- [ ] TradePayload format unchanged
- [ ] Management preset loading unchanged
- [ ] Auth flow unchanged
