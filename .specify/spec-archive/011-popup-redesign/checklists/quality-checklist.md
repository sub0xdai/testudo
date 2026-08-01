# Quality Checklist: 011-popup-redesign

> Spec: 011-popup-redesign | Date: 2026-02-12

## Pre-Implementation
- [ ] Spec reviewed and understood
- [ ] Dependencies (EXT-08, EXT-12) confirmed complete
- [ ] Current baseline passes: `bun run build && bun run typecheck && bun run test`

## Implementation
- [ ] FR-1: HeaderBar component created
- [ ] FR-2: Balance display in header
- [ ] FR-3: TabBar component created
- [ ] FR-4: MainView refactored to tab controller
- [ ] FR-5: Range sliders replace number inputs
- [ ] FR-6: Toggle cards for trailing stop / partial TP
- [ ] FR-7: PositionCard component created
- [ ] FR-8: ActiveOrders uses PositionCards
- [ ] FR-9: Account tab content complete
- [ ] FR-10: Empty positions state
- [ ] FR-11: Footer simplified
- [ ] FR-12: CSS additions complete
- [ ] FR-13: E2E tests updated
- [ ] FR-14: All data-testid attributes preserved
- [ ] FR-15: Build succeeds Chrome + Firefox

## Verification
- [ ] `bun run build` passes
- [ ] `bun run typecheck` zero errors
- [ ] `bun run test` all unit tests pass
- [ ] `bun run test:e2e` all E2E tests pass
- [ ] Manual verification: 3 tabs render correctly
- [ ] Manual verification: balance visible in header
- [ ] Manual verification: range sliders interactive
