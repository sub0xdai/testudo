# EXT-11: Popup UX Rework — Auth Gate + Roman Stoic Design System

> Priority: P0 | Depends on: EXT-08 | Status: Complete
> Created: 2026-02-11

## Overview

**Current:** The extension popup is a 320px vertical stack of four sections (Settings, Trade Management, Auth, Status Bar) with no access control. The design uses navy-blue backgrounds (#1a1a2e), emerald-green accents, and rounded corners — completely mismatched with testudo-web's "Roman Stoic Trading Terminal" aesthetic. Auth is buried at the bottom; users can trade live without authenticating. No session persistence across popup re-opens.

**Target:** Rework the popup into a 400px, two-view architecture following browser extension UX best practices (1Password, Todoist patterns). Auth gate on first open with persistent sessions. Design language aligned with testudo-web: hard 90-degree angles, Cinzel/Space Mono typography, jade/terracotta signal colors, #0A0A0A backgrounds. Single scroll layout with gear icon for settings.

## User Stories

- [ ] As a trader, I want to authenticate once and stay logged in across browser sessions so that opening the extension is instant.
- [ ] As a new user, I want to see a clean login screen when I first open the extension so the UX feels professional and secure.
- [ ] As a paper trader, I want to bypass login via "continue without account" so I can test without creating credentials.
- [ ] As a trader, I want the extension to visually match the testudo-web trading interface so the brand feels cohesive.
- [ ] As a trader, I want trade management config as the primary view so I can quickly adjust settings before pressing Alt+X.
- [ ] As a trader, I want settings (URLs, account) behind a gear icon so they don't clutter my primary workflow.

## Functional Requirements

| ID | Requirement | Priority |
|----|-------------|----------|
| FR-1 | **Auth Gate**: On popup open, check `browser.storage.local` for valid tokens. If valid, skip to main view. If expired/missing, show login screen. | High |
| FR-2 | **Persistent Session**: After successful login, tokens persist in `browser.storage.local`. Token refresh handled by `background.ts` (already implemented). Popup never shows login again until explicit logout. | High |
| FR-3 | **Paper Mode Bypass**: Login screen includes "continue without account" link. Entering paper mode sets `paperOnly=true` in storage, hides LIVE toggle in main view. | High |
| FR-4 | **Two-View Router**: `App.tsx` uses a Solid.js signal (`"auth" | "main" | "settings"`) to switch views. No router library needed for 3 states. | High |
| FR-5 | **Main View**: Header ("TESTUDO" in Cinzel + gear icon button), TradeManagement body, PAPER/LIVE mode toggle, footer with StatusBar + logged-in email. | High |
| FR-6 | **Settings View**: Gear icon opens settings. Contains: back button, Backend URL input, WebSocket URL input, Account section (email + LOGOUT button). Inputs save on change with "saved" feedback. | High |
| FR-7 | **Design System — Colors**: Background `#0A0A0A`, panels `#121212`, borders `#333333` (2px), text `#FFFFFF`/`#888888`/`#555555`, buy/positive `#4E9F76`, sell/negative `#A64B4B`, warning `#B87333`. | High |
| FR-8 | **Design System — Typography**: Cinzel for headers, labels, buttons (uppercase). Space Mono for inputs, data values. Fonts bundled as WOFF2 in extension (no external CDN — Manifest V3 CSP). | High |
| FR-9 | **Design System — Geometry**: Zero border-radius everywhere (`* { border-radius: 0 !important; }`). Hard 90-degree angles. Square status indicators (8x8px, not circles). | High |
| FR-10 | **Design System — Inputs**: Transparent background, bottom-border only (3px solid #333), focus state turns jade green (#4E9F76). Space Mono font. No number spinners. | Medium |
| FR-11 | **Design System — Buttons**: Transparent bg, 2px border, uppercase Cinzel text. Fill on hover. Industrial style matching testudo-web. | Medium |
| FR-12 | **Design System — Transitions**: Instant (0s). No fade animations. Blink animation for connecting status only. | Medium |
| FR-13 | **Mode Toggle**: PAPER button bordered jade green, LIVE bordered terracotta red. Active state fills with color. If `paperOnly`, LIVE button hidden. Persists to `browser.storage.local`. | High |
| FR-14 | **Auth Context**: Shared Solid.js context (`createContext`/`useContext`) holding `authenticated`, `email`, `paperOnly` signals. Consumed by AuthSection, MainView, SettingsView. | Medium |
| FR-15 | **Popup Width**: 400px (up from 320px). Single scroll layout. | Medium |
| FR-16 | **Background Worker Unchanged**: `background.ts` is not modified. All existing message types (LOGIN, LOGOUT, AUTH_STATUS, EXECUTE_TRADE, WS_STATUS, WS_STATE_CHANGED) remain as-is. | High |

## Technical Notes

### Files to Create

| File | Purpose |
|------|---------|
| `src/popup/context/AuthContext.tsx` | Shared auth state context (authenticated, email, paperOnly signals) |
| `src/popup/components/MainView.tsx` | Post-auth primary view: header + TradeManagement + ModeToggle + footer |
| `src/popup/components/ModeToggle.tsx` | Extracted PAPER/LIVE toggle with paperOnly awareness |
| `src/popup/components/SettingsView.tsx` | Settings panel: URL inputs + account section + back navigation |
| `src/fonts/cinzel-variable.woff2` | Bundled Cinzel font (display/headers) |
| `src/fonts/space-mono-regular.woff2` | Bundled Space Mono font (data/inputs, regular weight) |
| `src/fonts/space-mono-bold.woff2` | Bundled Space Mono font (data/inputs, bold weight) |

### Files to Modify

| File | Change |
|------|--------|
| `src/popup/App.tsx` | Rewrite: 3-state view router wrapping AuthProvider |
| `src/popup/popup.css` | Rewrite: @font-face declarations, @theme tokens, global resets (border-radius: 0), input/button base styles |
| `src/popup/popup.html` | Set body background #0A0A0A as fallback |
| `src/popup/components/AuthSection.tsx` | Rewrite: full-page login gate with onAuthenticated/onContinueWithoutAccount callbacks |
| `src/popup/components/TradeManagement.tsx` | Restyle: swap navy/emerald palette to core/jade, Cinzel labels, Space Mono inputs |
| `src/popup/components/StatusBar.tsx` | Restyle: square dot (remove rounded-full), new color palette, blink animation |
| `build.ts` | Add font file copying to dist output |

### Files to Delete

| File | Reason |
|------|--------|
| `src/popup/components/Settings.tsx` | Replaced by SettingsView.tsx |

### Architecture

```
popup.html
  └── App.tsx (AuthProvider wrapper)
        ├── [view === "auth"]     → AuthSection (full-page login gate)
        │                            ├── Login form (email + password)
        │                            ├── LOGIN button
        │                            └── "continue without account" link
        │
        ├── [view === "main"]     → MainView
        │                            ├── Header: "TESTUDO" + gear icon
        │                            ├── TradeManagement (body)
        │                            ├── ModeToggle (PAPER | LIVE)
        │                            └── Footer: StatusBar + email
        │
        └── [view === "settings"] → SettingsView
                                     ├── Back button
                                     ├── Backend URL input
                                     ├── WebSocket URL input
                                     └── Account: email + LOGOUT
```

### Font Bundling Strategy

Chrome Manifest V3 extensions block external stylesheets via CSP. Fonts must be bundled:

1. Download WOFF2 files from Google Fonts
2. Place in `src/fonts/`
3. Declare `@font-face` in `popup.css`
4. Copy to `dist/popup/fonts/` via `build.ts`
5. Reference via relative URL in CSS: `url('./fonts/cinzel-variable.woff2')`

### Design Token Reference (Tailwind v4 @theme)

```css
@theme {
  --color-bg-core: #0A0A0A;
  --color-bg-panel: #121212;
  --color-bg-elevated: #1A1A1A;
  --color-bg-hover: #242424;
  --color-border-grid: #333333;
  --color-border-active: #FFFFFF;
  --color-signal-green: #4E9F76;
  --color-signal-red: #A64B4B;
  --color-signal-orange: #B87333;
  --color-text-primary: #FFFFFF;
  --color-text-secondary: #888888;
  --color-text-dim: #555555;
  --font-family-display: 'Cinzel', 'Times New Roman', serif;
  --font-family-mono: 'Space Mono', monospace;
}
```

### Existing Code to Reuse

- `background.ts` message handlers: LOGIN, LOGOUT, AUTH_STATUS, EXECUTE_TRADE, WS_STATUS, WS_STATE_CHANGED — all unchanged
- `browser.storage.local` keys: `accessToken`, `refreshToken`, `tokenExpiry`, `backendUrl`, `wsUrl`, `executionMode`, `managementPreset` — all unchanged
- `TradeManagement.tsx` field logic: signal-based preset loading, updateField pattern, min/max validation — logic preserved, only styles change
- `StatusBar.tsx` WS state listener pattern — preserved, only styles change
- `AuthSection.tsx` login flow (sendMessage → background → store tokens) — logic preserved in new auth context

### Dependencies

- Solid.js (existing)
- Tailwind CSS v4 (existing)
- No new npm dependencies

### Assumptions

- Google Fonts WOFF2 files are freely redistributable (they are, under SIL Open Font License)
- Tailwind v4 `@theme` directive works in the extension's esbuild/Tailwind CLI pipeline
- `browser.storage.local` has sufficient space for font-unrelated data (it does — 10MB limit)

---

## Acceptance Criteria

- [ ] Fresh install: popup opens to login screen, not trade management
- [ ] Login persists: close popup, reopen — goes directly to main view (no re-login)
- [ ] "continue without account" enters paper-only mode (LIVE toggle hidden)
- [ ] Gear icon opens settings view, back button returns to main view
- [ ] All text uses Cinzel (headers/labels/buttons) or Space Mono (inputs/data)
- [ ] Zero border-radius on all elements (inspect via DevTools)
- [ ] Background is #0A0A0A, panels #121212, borders #333333
- [ ] Buy/positive elements use #4E9F76, sell/negative use #A64B4B
- [ ] Popup width is 400px
- [ ] Status indicator is square (8x8px), not circular
- [ ] Mode toggle: PAPER=jade, LIVE=terracotta, active state fills with color
- [ ] `bun run build` succeeds for both Chrome and Firefox targets
- [ ] `bun run typecheck` passes with no errors
- [ ] `bun run test` — all existing unit tests pass
- [ ] Settings (URLs, management preset) persist across popup close/reopen
- [ ] Logout from settings view returns to auth gate
- [ ] `background.ts` is unmodified (zero changes)

---

## Completion Signal

### Implementation Checklist
- [ ] Fonts downloaded and bundled in `src/fonts/`
- [ ] `popup.css` rewritten with @font-face, @theme, global resets
- [ ] `AuthContext.tsx` created with shared auth state
- [ ] `App.tsx` rewritten with 3-state view router
- [ ] `AuthSection.tsx` rewritten as full-page login gate
- [ ] `MainView.tsx` created (header + trade management + mode toggle + footer)
- [ ] `ModeToggle.tsx` created with paperOnly awareness
- [ ] `SettingsView.tsx` created (URLs + account + back nav)
- [ ] `TradeManagement.tsx` restyled to Roman aesthetic
- [ ] `StatusBar.tsx` restyled (square dots, new palette)
- [ ] `build.ts` updated to copy font files
- [ ] `Settings.tsx` deleted (replaced by SettingsView.tsx)

### Testing Requirements
- [ ] `bun run build` exits 0
- [ ] `bun run typecheck` exits 0
- [ ] `bun run test` exits 0
- [ ] Manual: load in Chrome, verify auth gate flow
- [ ] Manual: verify persistent session across popup close/reopen
- [ ] Manual: verify paper mode bypass hides LIVE toggle
- [ ] Manual: verify settings view navigation (gear icon + back)
- [ ] Manual: verify design matches testudo-web aesthetic

### Done Signal
When ALL above criteria are satisfied, output:
```
<promise>DONE</promise>
```

---

*Template version: 1.0*
