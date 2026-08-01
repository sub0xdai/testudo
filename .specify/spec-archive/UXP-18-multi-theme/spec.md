# UXP-18: Multi-Theme Support (AMOLED / Soft Dark / Light)

## Problem
The app is locked to a single AMOLED theme (pure blacks). Users in bright environments can't read it, and the pure black can feel harsh on non-OLED screens. Need three switchable themes with persistence.

## Themes

### AMOLED (current default)
```
--bg-core:      #050505
--bg-panel:     #0A0A0A
--bg-elevated:  #111111
--bg-hover:     #1A1A1A
--border:       #3F3F46
--border-active:#52525B
--text-primary: #FFFFFF
--text-secondary:#888888
--text-tertiary: #555555
--signal-green: #00FF41
--signal-red:   #FF003C
```

### Soft Dark
```
--bg-core:      #16161e
--bg-panel:     #1e1e2a
--bg-elevated:  #262636
--bg-hover:     #30304a
--border:       #3b3b52
--border-active:#52526b
--text-primary: #e0ddd8
--text-secondary:#8a8898
--text-tertiary: #5a5868
--signal-green: #00FF41
--signal-red:   #FF003C
```

### Light (Paper)
```
--bg-core:      #f5f0e8
--bg-panel:     #faf7f2
--bg-elevated:  #fffcf7
--bg-hover:     #ebe5db
--border:       #d4cdc2
--border-active:#b0a898
--text-primary: #1a1714
--text-secondary:#6b6458
--text-tertiary: #9a9285
--signal-green: #1a7a2e
--signal-red:   #b8002a
```

Warm cream/parchment feel — not clinical white. Signal colors darken for WCAG AA contrast against light backgrounds. All other tokens are structural — same semantic role, different values.

## Architecture

### Token Switching Mechanism
Use `data-theme` attribute on `<html>` element. CSS selectors:
```css
:root, [data-theme="amoled"] { /* AMOLED tokens */ }
[data-theme="soft-dark"]     { /* Soft Dark tokens */ }
[data-theme="light"]         { /* Light tokens */ }
```

### Persistence
- Key: `testudo-theme` in localStorage
- Values: `"amoled"` | `"soft-dark"` | `"light"`
- Default: `"amoled"` (no attribute = AMOLED)
- Applied before first paint via inline `<script>` in `<head>` to prevent flash

### Anti-flash Script (all HTML entry points)
```html
<script>
  const t = localStorage.getItem('testudo-theme');
  if (t && t !== 'amoled') document.documentElement.setAttribute('data-theme', t);
</script>
```

## Changes Per Surface

### FR-1: Extension Popup (`popup.css`)
**Problem:** Tailwind v4 `@theme` is static — values are resolved at build time and can't switch at runtime.

**Solution:** Keep `@theme` for Tailwind utility generation but override all color utilities via CSS custom properties:
1. Define all color tokens as CSS custom properties in `:root`
2. Add `[data-theme="soft-dark"]` and `[data-theme="light"]` override blocks
3. Change `@theme` references to use `var(--color-*)` instead of hardcoded hex
4. Add anti-flash script to `popup.html`

**Affected classes:** Any Tailwind utility referencing the `@theme` color tokens (`bg-bg-core`, `text-text-primary`, `border-border-subtle`, etc.)

### FR-2: Extension Modal (`modal.tsx`)
**Problem:** `:host` block defines CSS custom properties with hardcoded values.

**Solution:** Read theme from `document.documentElement.dataset.theme` when creating the Shadow DOM, set matching CSS custom properties in `:host`. The modal is short-lived (created/destroyed per trade), so reading once on creation is sufficient.

### FR-3: Web App (`testudo-web`)
**Problem:** Tailwind preset defines tokens statically.

**Solution:**
1. Add CSS custom properties to `src/index.css` or a new `src/styles/theme.css`
2. Override preset colors to reference CSS vars: `'main-bg': 'var(--bg-core)'`
3. Add anti-flash script to `index.html`
4. Add theme picker to Header component (3-way toggle or dropdown)

### FR-4: Journal/Desk App (`testudo-journal`)
**Problem:** `app.css` `:root` block has hardcoded values. `tokens.ts` exports hardcoded JS constants for ECharts.

**Solution:**
1. Add `[data-theme]` blocks to `app.css`
2. Change `tokens.ts` to read from CSS custom properties at runtime:
   ```ts
   export function getSignalGreen(): string {
     return getComputedStyle(document.documentElement)
       .getPropertyValue('--signal-green').trim() || '#00FF41';
   }
   ```
3. Charts must re-render when theme changes (listen to a custom event or mutation observer on `data-theme`)
4. Add anti-flash script to `index.html`
5. Add theme toggle to Layout.tsx header nav

### FR-5: Extension Background/Content Scripts
No changes needed — these don't render UI. The popup reads its own `popup.html` which has the anti-flash script. The modal reads from the host page's `document.documentElement`.

## Theme Picker UI

### Extension Popup (SettingsView.tsx)
Three-button segmented control:
```
[ AMOLED ] [ SOFT ] [ LIGHT ]
```
- White border outline style (matching UXP-17)
- Active state: inverted (white bg, dark text)
- On click: set `localStorage.testudo-theme`, set `document.documentElement.dataset.theme`

### Web App (Header.tsx)
Small icon button or dropdown in the nav bar. Could be a simple cycle button (click cycles AMOLED → Soft → Light → AMOLED).

### Journal/Desk (Layout.tsx)
Same as web — small toggle in the header nav, after the JOURNAL link and before HOME.

## Verification
```bash
cd testudo-extension && bun run build
cd testudo-web && bun run build
cd testudo-journal && bun run build
```
- Visual check: all three themes render correctly on each surface
- No flash of wrong theme on page load
- Theme persists across page refreshes and between web ↔ desk navigation
- ECharts colors update when theme switches
- Signal colors maintain WCAG AA contrast in all themes

## Edge Cases
- **Cross-app sync:** All apps read from the same `localStorage.testudo-theme` key on the same origin (via proxy). Changing theme in settings propagates to desk on next page load.
- **Extension popup:** Separate origin from web — has its own localStorage. Theme setting is independent. Could sync via `chrome.storage.local` if cross-context sync is desired (deferred).
- **System preference:** Could add a 4th option `"system"` that maps to light/dark via `prefers-color-scheme`. Deferred for simplicity.
- **Chart re-render:** ECharts instances cache colors at init. On theme change, call `chart.setOption()` with updated colors or `chart.dispose()` + re-init.

## Non-Goals
- Per-component theming (everything switches together)
- Custom user-defined themes
- System preference auto-detection (deferred)
- Extension ↔ web theme sync (different origins)
