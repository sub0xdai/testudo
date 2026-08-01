# Next Session: Polish 011-popup-redesign UI

## Context for the Next Engineer

You are continuing work on the Testudo browser extension popup UI redesign. The structural refactor is DONE (3-tab layout, HeaderBar, TabBar, PositionCard, range sliders, toggle cards). All tests pass. But the visual polish needs work — it looks utilitarian/flat compared to the reference trading app UIs.

## What Exists Now

The extension popup (`testudo-extension/`) is built with **Solid.js + Tailwind CSS v4** (460px wide, dark theme, zero border-radius).

### Current File Structure
```
src/popup/
├── popup.css                          # Tailwind v4 @theme tokens + global styles
├── popup.html                         # Entry point
├── index.tsx                          # Mount point
├── App.tsx                            # View router (auth → main → settings)
├── context/AuthContext.tsx             # Auth state provider
├── components/
│   ├── HeaderBar.tsx                  # Logo + balance + compact mode toggle + WS dot + gear
│   ├── TabBar.tsx                     # Trade / Positions / Account tabs
│   ├── MainView.tsx                   # Tab controller + footer
│   ├── TradeManagement.tsx            # Range sliders + toggle cards
│   ├── ActiveOrders.tsx               # Position card list
│   ├── PositionCard.tsx               # Rich trade card with accent border
│   ├── ModeToggle.tsx                 # PAPER/LIVE toggle (compact + full variants)
│   ├── StatusBar.tsx                  # WS connection dot + label
│   ├── AuthSection.tsx                # Login form
│   └── SettingsView.tsx               # Backend/WS URL config
```

### Current Design Tokens (popup.css @theme)
```
bg-core: #0A0A0A       bg-panel: #121212      bg-elevated: #1A1A1A
bg-hover: #242424      border-grid: #444444    border-active: #FFFFFF
signal-green: #4E9F76  signal-red: #A64B4B     signal-orange: #B87333
text-primary: #FFFFFF   text-secondary: #B0B0B0 text-dim: #777777
Fonts: Cinzel (display), Space Mono (mono)
```

## What's Wrong (Current vs Target)

### 1. Tab Bar looks like plain buttons
**Current:** White text on flat bg-panel, hard white underline on active tab.
**Target:** Pill-shaped segmented control with subtle highlight on active tab (like the reference bottom nav with "Trade | Positions | Orders"). Consider a contained pill with bg-elevated for active state instead of underline.

### 2. Range sliders are nearly invisible
**Current:** 4px green track with a tiny 12px green-bordered square thumb. The fill gradient works but the thumb is too small to see.
**Target:** Make the thumb larger (16-20px), add a glow/shadow effect, or use a more prominent visual. The slider fill should be more visible — consider making the track thicker (6-8px).

### 3. Header feels cramped
**Current:** Status dot + TESTUDO + PAPER pill + gear all jammed on one line, balance below.
**Target:** Give the header more breathing room. Consider moving the WS status text (currently just a dot) more prominent, or adding subtle spacing. The balance (10,000.00 USDT) should feel more like a hero number — larger, maybe 24-28px.

### 4. No visual depth
**Current:** Everything is flat colored boxes. No gradients, no shadows, no layering.
**Target (from reference):** Subtle dark gradients on cards/panels, maybe a very faint border glow on active elements, subtle shadow depth on the tab bar. The reference has a feeling of floating elements over a dark background.

### 5. Toggle cards (Trailing Stop, Partial TP) look boxy
**Current:** Plain border + orange label + OFF button. No visual hierarchy.
**Target:** Add subtle bg-panel fill even when collapsed, smoother transition feel. The OFF/ON button could be more styled (pill shape even with 0 radius, or inverted colors).

### 6. Input fields for slider values are clunky
**Current:** Standard number inputs with bottom-border styling next to range sliders.
**Target:** These should feel more integrated — same line as the slider, perhaps with a subtle bg-elevated box around the number.

### 7. Footer is too plain
**Current:** "PAPER ONLY" text at bottom with minimal styling.
**Target:** Could be removed entirely (mode info already in header) or styled as a very subtle status strip.

## Reference UI Characteristics to Match

The reference screenshot (trading app with leverage gauge) has these qualities:
- **Depth**: Dark gradient background, cards float above
- **Contained inputs**: Price and Size are in bordered boxes, not bare underline inputs
- **Info grid**: Mark Price / 24h Change / Liquidation / Margin Req arranged in a clean 2x2 grid
- **Tab bar**: Bottom pill-shaped segmented control with contained highlight
- **Typography contrast**: Large numbers are bold and prominent, labels are dim and small
- **Spacing**: Generous padding, nothing feels cramped
- **The circular gauge**: We don't need this, but the SENSE of a focal visual element is important

## What NOT to Change
- DO NOT change any `data-testid` attributes
- DO NOT change component structure (keep all existing components)
- DO NOT change functionality or message passing logic
- DO NOT change the build system or manifest
- Keep all tests passing: `bun run build && bun run typecheck && bun run test`
- E2E tests: `npx playwright test tests/e2e/popup.spec.ts` (10 tests must pass)

## Suggested Approach

1. **Start with popup.css** — adjust design tokens, add subtle gradients/shadows, increase slider track/thumb sizes
2. **Polish HeaderBar.tsx** — more spacing, larger balance, better visual hierarchy
3. **Restyle TabBar.tsx** — contained pill/segment style instead of underline
4. **Enhance TradeManagement.tsx** — larger slider thumbs, value inputs in contained boxes
5. **Add depth to PositionCard.tsx** — subtle panel shadow or gradient bg
6. **Verify** — `bun run build && bun run typecheck && bun run test && npx playwright test tests/e2e/popup.spec.ts`

## Verification Commands
```bash
cd testudo-extension
bun run build        # Chrome + Firefox
bun run typecheck    # Zero TS errors
bun run test         # 63 unit tests
npx playwright test tests/e2e/popup.spec.ts  # 10 E2E tests
```
