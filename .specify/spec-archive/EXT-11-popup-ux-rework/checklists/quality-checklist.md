# Quality Checklist: EXT-11-popup-ux-rework

> Spec ID: EXT-11-popup-ux-rework
> Date: 2026-02-11

## Code Quality
- [ ] No TypeScript errors (`bun run typecheck`)
- [ ] Build succeeds (`bun run build`)
- [ ] Unit tests pass (`bun run test`)
- [ ] No unused imports or dead code
- [ ] All new components have `data-testid` attributes for E2E testing

## Design System Compliance
- [ ] Zero border-radius on all elements
- [ ] Cinzel font loads and renders for headers/labels/buttons
- [ ] Space Mono font loads and renders for inputs/data
- [ ] Color palette matches spec: #0A0A0A, #121212, #333333, #4E9F76, #A64B4B
- [ ] All transitions are instant (0s)
- [ ] Status indicators are square, not circular
- [ ] Popup width is 400px

## Functional Completeness
- [ ] Auth gate blocks access on fresh install
- [ ] Login → tokens stored → persistent session works
- [ ] "continue without account" → paper-only mode (LIVE hidden)
- [ ] Gear icon → settings view → back button → main view
- [ ] Settings (URLs) persist across popup close/reopen
- [ ] Trade management preset persists across popup close/reopen
- [ ] Mode toggle persists and correctly shows PAPER/LIVE state
- [ ] Logout returns to auth gate
- [ ] background.ts is completely unmodified

## Cross-Browser
- [ ] Chrome build loads and renders correctly
- [ ] Firefox build loads and renders correctly
- [ ] Fonts render in both browsers
