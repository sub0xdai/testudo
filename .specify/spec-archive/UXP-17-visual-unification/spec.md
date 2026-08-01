# UXP-17: Visual Unification

## Problem
The web landing page is monochrome brutalist but the extension popup uses softer pastels, blue-tinted backgrounds, and larger border radii. Three surfaces feel like three separate apps.

## Design Direction
- Backgrounds: Pure blacks (#050505, #0A0A0A, #111111) — no blue tint
- Borders: Gray (#3F3F46) — not transparent white overlays
- Text: White primary, #888888 secondary, #555555 tertiary
- Buttons: White-bordered outlines by default, invert on hover
- Signal colors: #00FF41 green, #FF003C red — ONLY for trading signals
- Border radius: 4px default (sharp, brutalist)

## Changes

### FR-1: Extension popup CSS token overhaul (popup.css)
- Backgrounds: #050505 / #0A0A0A / #111111
- Borders: #3F3F46 solid
- Signal green: #00FF41 (was #4ade80)
- Signal red: #FF003C (was #ef4444)
- Text secondary: #888888 (was #9ca3af)
- Button radius: 4px (was 10px), input/card: 6px (was 12px)

### FR-2: Extension popup components — decorative color removal
- Buttons using green/red backgrounds → white-bordered outlines
- Only trading data (P&L, position status) keeps signal colors

### FR-3: Login/Register pages — token migration
- bg-zinc-900 → bg-container-bg
- border-zinc-700 → bg-container-border
- text-gray-400 → text-secondary
- text-red-400 → text-signal-red
- Submit button: white-bordered outline style

### FR-4: Modal review — button style alignment
- Verify button styles use white-bordered outlines
- ARM/CONFIRM can keep signal colors (trading action)

## Verification
- `cd testudo-extension && bun run build`
- `cd testudo-web && bun run build`
