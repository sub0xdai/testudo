# Optimization Target: css-dedup

## Goal
Eliminate duplicated inline styles across extension components. Repeated `style={{...}}` patterns (text-shadow, font-family, gradient functions) should use CSS utility classes or existing `@theme` variables instead.

"Better" means: fewer inline style declarations with zero visual change.

## Target Files
- `testudo-extension/src/popup/popup.css`
- `testudo-extension/src/popup/components/TradeManagement.tsx`
- `testudo-extension/src/popup/components/LoginPreview.tsx`
- `testudo-extension/src/popup/components/ArcGauge.tsx`
- `testudo-extension/src/popup/components/MainView.tsx`
- `testudo-extension/src/modal.tsx`

## Constraints
- Do NOT modify test files
- Do NOT change any visual appearance (colors, fonts, shadows, gradients)
- Do NOT add new CSS dependencies or preprocessors
- Do NOT modify the Shadow DOM isolation in modal.tsx (MODAL_STYLES must remain inline for Shadow DOM)
- Tailwind v4 `@theme` variables are the source of truth — use them

## Strategy Hints
- `text-shadow: 0 0 20px ...` appears in LoginPreview.tsx and MainView.tsx with the same value — extract to a utility class
- `style={{ "font-family": "var(--font-family-mono)" }}` appears in multiple components — should be a Tailwind class
- `riskColorMemo()` inline color used 2x in TradeManagement.tsx — consider CSS variable
- Slider gradient functions (`sliderStyle()`/`riskSliderStyle()`) generate inline backgrounds — these may need to stay inline due to dynamic values, but the base pattern could be shared

## Verification
```bash
cd testudo-extension && bun run build
```
