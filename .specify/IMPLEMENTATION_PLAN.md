# Implementation Plan

> Last updated: 2026-03-24
> Current spec: EXT-40-smart-card-grid
> Phase: BUILD

---

## Active Spec: EXT-40-smart-card-grid

Redesign Account Management UI from stacked rows to responsive card grid with heartbeat indicators, kebab menus for destructive actions, ghost "Add Exchange" card, and compact extension pairing banner.

### Tasks

| ID | Task | Status | Complexity | Depends On |
|----|------|--------|------------|------------|
| T1 | Create ExchangeCard component — heartbeat indicator (pulsing green/red based on `is_active`), exchange name + type badge, truncated identifier, balance placeholder ("---"), kebab menu (⋮) with Test/Delete/Revoke actions, click-outside-to-close dropdown, confirmation step for destructive actions, test result display, migration prompt for direct-key Hyperliquid. | complete | medium | — |
| T2 | Create AddExchangeCard + ExtensionPairingBanner — dashed ghost card with "+" icon and hover effect; compact full-width banner layout for extension pairing (title+button inline, condensed padding). | complete | low | — |
| T3 | Rewrite AccountPage grid layout — replace stacked rows with `grid grid-cols-1 md:grid-cols-2 lg:grid-cols-3 gap-4`, widen container to `max-w-5xl`, wire ExchangeCard/AddExchangeCard/ExtensionPairingBanner, add `fetchBalance` to API client + balance type, async per-card balance fetching on mount, preserve onboarding/setupComplete screens. | pending | medium | T1, T2 |
| T4 | Build validation — `bun run build` for testudo-web, verify all 7 acceptance criteria against code. | pending | low | T3 |

### Key Decisions

- **React, not Solid.js**: Spec code examples use Solid.js (`createSignal`, `Show`) but testudo-web is React 18. All implementations use React hooks/JSX. Spec examples are treated as visual/behavioral reference, not literal code.
- **Heartbeat uses `is_active` field, not live health check**: Calling `POST /accounts/{id}/test` for every account on page load is heavy and slow. Instead, heartbeat reflects the `is_active` boolean from the accounts list (available immediately). The TEST action in the kebab menu provides on-demand health verification. This is KISS.
- **Balance fetch is async per-card, not blocking**: Backend has `GET /exchanges/accounts/{id}/balance` (not yet in frontend client). Cards render immediately with "---", then fill in as balance responses arrive. Errors show "---" silently — no error toast for balance failures.
- **Container widens from `max-w-2xl` to `max-w-5xl`**: A 3-column grid needs horizontal space. The current 672px max is too narrow. 1024px allows comfortable 3-col layout on desktop.
- **Form remains inline below grid, not in a modal**: The existing add-exchange form (selector + API key inputs or WalletConnect) works well inline. Moving to a modal adds complexity (portal, focus trap) without clear UX benefit. Ghost card click reveals the form section below the grid.
- **ExtensionPairingBanner is a layout refactor, not a rewrite**: Same logic as existing `ExtensionPairing.tsx`, just rendered in a compact horizontal layout instead of vertical card. Could be done as a variant prop or new component — new component is cleaner (SoC).
- **No new dependencies**: Spec mentions `lucide-solid` (optional). Using HTML entities/SVG inline for icons keeps the bundle lean. The "+" and "⋮" are text characters, checkmark is inline SVG (already used in setupComplete screen).
- **Confirmation dialogs stay inline**: The existing confirm/cancel pattern (DEL → CONFIRM/NO) is proven and works. Moving to the kebab dropdown, the confirmation renders inside the dropdown menu itself — keeps the interaction contained.

### Discoveries

- **AccountPage.tsx is 532 lines with 3 return paths**: setupComplete (line 290), onboarding (line 322), and normal management (line 347). Only the normal management path needs grid rewrite — other two are separate screens.
- **`ExchangeInfo.type` field exists**: Available from `GET /exchanges` response. Used for CEX/DEX badge on cards. Currently unused in the UI.
- **No `fetchBalance` in frontend client**: Backend route `GET /exchanges/accounts/{id}/balance` returns `{ account_id, exchange_name, balances: [{ asset, total, free, used }], fetched_at }`. Need to add wrapper + type to `api/client.ts` and `types/index.ts`.
- **`Card` component `rounded` prop is dead code**: Passed but ignored (`_rounded`). Cards render without border-radius. New ExchangeCard will not use Card component — it's a standalone styled div to match the spec's hover/transition design.
- **Existing action buttons are flat and co-located**: TEST, REVOKE, and DEL share identical wireframe styling (line 414-470). This is exactly the problem the spec identifies — moving to kebab menu separates benign from destructive actions.

---

## Completed Specs

| Spec | Completion Date |
|------|-----------------|
| EXT-39-pair-ux | 2026-03-24 |
| AUTH-03-frontend-auth | 2026-03-24 |
| AUTH-02-backend-auth | 2026-03-24 |
| AUTH-01-infra-hardening | 2026-03-24 |
| ANL-01-bloomberg-charts (Phase 1) | 2026-03-23 |
| JNL-18-storage-quotas | 2026-03-22 |
| JNL-17-nested-collections | 2026-03-22 |
| JNL-16-database-view | 2026-03-22 |
| JNL-15-export-with-images | 2026-03-22 |
| JNL-14-markdown-hardening | 2026-03-22 |
| UXP-21-light-theme-parity | 2026-03-22 |
| UXP-23-landing-typography | 2026-03-22 |
| UXP-20-strip-glassmorphism | 2026-03-22 |
| UXP-22-signal-color-calibration | 2026-03-22 |
| UXP-19-features-layout | 2026-03-22 |
| EXT-38-background-decomposition | 2026-03-22 |
| EXT-37-message-dispatch-refactor | 2026-03-22 |
| UXP-18-multi-theme | 2026-03-21 |
| HL-11-status-transition-fix | 2026-03-21 |

---

*This file is persistent state. Vox updates it each iteration.*
