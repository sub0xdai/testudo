# Specification: Design Polish — Accent Moments, Empty States, Lock Screen

**Spec ID:** UX-05-design-polish
**Date:** 2026-04-09
**Status:** Draft
**Class:** Refactor / UX
**Priority:** P1 — pre-token-launch visual polish; overview and extension are the shareability surfaces
**Depends on:** None
**Series:** UX-05 (standalone)

---

## Problem Statement

The journal frontend has a strong brutalist design system but three areas underperform for the upcoming public launch:

1. **No visual focal point per page.** The monochrome palette is disciplined but relentless — every data point competes equally for attention. A single accent color moment per page would draw the eye to the most important metric and make screenshots more compelling.

2. **Empty states are dead ends.** New users who just connected their wallet see "NO TRADES FOUND" or "No daily p&l data" with zero guidance. These are onboarding moments being wasted — the first screens a pump.fun buyer encounters after signing in.

3. **Lock screen is a wall, not an invitation.** The unauthenticated view shows a centered card with "Connect your wallet" over a blurred background. There's no preview of what the product actually looks like — no reason for a curious visitor to connect. Showing a blurred/dimmed preview of the actual desk would sell the product while asking for auth.

None of these changes affect functionality. They're visual refinements to the existing components.

---

## User Stories

- **As a new user from pump.fun**, I want to see what the trading desk looks like before connecting my wallet, so that I'm motivated to sign in.
- **As a new user with no trades**, I want guidance on what to do next when pages are empty, so that I understand the product isn't broken.
- **As a trader viewing the overview**, I want my net P&L and equity curve to visually stand out, so that the most important data catches my eye first.

---

## Functional Requirements

| ID | Requirement | Priority | Subsystem |
|----|-------------|----------|-----------|
| FR-1 | Overview hero P&L number uses `accent-primary` (rust/copper) color | High | Overview |
| FR-2 | Equity curve chart line uses `accent-primary` color | High | HeroEquityCurve |
| FR-3 | Journal page: trade count or win rate stat uses accent color | Medium | PageSubHeader or Journal |
| FR-4 | Empty state in TradeTable shows actionable guidance instead of bare "NO TRADES FOUND" | High | TradeTable |
| FR-5 | Empty state in ChartContainer shows contextual guidance per chart type | Medium | ChartContainer |
| FR-6 | Empty state in JournalTimeline shows guidance for first journal entry | Medium | JournalTimeline |
| FR-7 | Lock screen shows blurred/dimmed static preview of overview behind the auth card | High | Layout/LockScreen |
| FR-8 | Lock screen preview is a static image or CSS mockup, not live data | High | Layout/LockScreen |

---

## Technical Implementation

### Vertical Checkpoints

| Checkpoint | Scope | Validates |
|------------|-------|-----------|
| CP-1 | Accent color moments: overview hero P&L + equity curve line (FR-1, FR-2) | Visual hierarchy improved; one focal point per view |
| CP-2 | Empty states: TradeTable, ChartContainer, JournalTimeline (FR-4, FR-5, FR-6) | New users see guidance, not dead ends |
| CP-3 | Lock screen preview (FR-7, FR-8) | Unauthenticated visitors see product value |

### CP-1: Accent Color Moments

**Overview hero P&L number** — Currently uses `pnlColor()` which returns green/red based on positive/negative. The accent treatment should be additive, not replacing the signal colors.

Approach: Apply accent-primary as an underline or left-border accent on the hero P&L container, not on the number itself (which must stay green/red for signal meaning). This frames the number as the focal point without overriding its semantic color.

```tsx
// Overview.tsx — hero metrics section
<div class="px-8 py-6 border-b border-container-border/50">
  {/* Accent left border on hero P&L */}
  <div class="border-l-2 border-accent-primary pl-6">
    <div class="flex items-baseline gap-8 mb-1">
      <div>
        <span class={`font-mono text-4xl md:text-5xl font-bold ${pnlColor(stats()!.account.net_pnl)}`}>
          {formatCurrency(stats()!.account.net_pnl)}
        </span>
        ...
```

**Equity curve line color** — Currently likely uses a default chart color. Change to `--accent-primary` CSS variable.

```typescript
// HeroEquityCurve.tsx — ECharts line series config
series: [{
  type: 'line',
  data: ...,
  lineStyle: { color: `rgb(${getComputedStyle(document.documentElement).getPropertyValue('--accent-primary').trim().replace(/ /g, ',')})` },
  areaStyle: { color: { type: 'linear', ... } },  // gradient fade below line
}]
```

Use the existing `getTokenColor()` helper from `lib/tokens.ts` if available, or read the CSS variable directly. The line should be the rust/copper accent; the area fill should be a subtle 10% opacity gradient of the same color fading to transparent.

**Journal page accent** — Add accent-primary left border on the trade count stat in PageSubHeader, matching the Overview treatment. Subtle, consistent.

### CP-2: Empty States

**TradeTable empty state** — Replace bare "NO TRADES FOUND" with contextual guidance:

```tsx
// Current (TradeTable.tsx)
<td colspan={...} class="px-3 py-12 text-center text-text-tertiary font-mono text-sm">
  NO TRADES FOUND
</td>

// New
<td colspan={...} class="px-3 py-16 text-center">
  <p class="font-mono text-sm text-text-secondary mb-2">NO TRADES YET</p>
  <p class="font-mono text-xs text-text-tertiary mb-1">
    Trades appear automatically after your first fill on a connected exchange.
  </p>
  <p class="font-mono text-xs text-text-tertiary">
    Or import history from the <a href="/account" class="text-accent-steel hover:text-text-primary transition-colors underline">Account</a> page.
  </p>
</td>
```

If filters are active and produce zero results, show a different message:

```tsx
<p class="font-mono text-sm text-text-secondary mb-2">NO MATCHING TRADES</p>
<p class="font-mono text-xs text-text-tertiary">
  Try adjusting your filters or time range.
</p>
```

**ChartContainer empty state** — Already has basic empty handling. Enhance with chart-specific hints:

```tsx
// ChartContainer.tsx — enhance empty state
<p class="font-mono text-xs text-text-tertiary mb-1">
  No {props.title.toLowerCase()} data
</p>
<Show when={!props.hasActiveFilters}>
  <p class="font-mono text-[10px] text-text-tertiary">
    Data populates as you close trades.
  </p>
</Show>
```

**JournalTimeline empty state** — Show guidance for first journal entry:

```tsx
<p class="font-mono text-sm text-text-secondary mb-2">NO JOURNAL ENTRIES</p>
<p class="font-mono text-xs text-text-tertiary">
  Click on a trade to open the detail panel, then write your thesis in the notes section.
</p>
```

### CP-3: Lock Screen Preview

The lock screen currently shows a centered card over a blurred Hadrian's Wall background. The goal is to show a **static preview** of what the overview looks like with data, dimmed and blurred behind the auth card.

Approach: Use a **static screenshot/mockup** rendered as a background image, not live data (which would require API calls and break without auth). This keeps it lightweight and avoids any data leakage.

```tsx
// Layout.tsx — LockScreen component
function LockScreen() {
  const auth = useAuth()
  return (
    <div class="relative z-10 min-h-[calc(100vh-var(--header-h))]">
      {/* Dimmed, blurred preview of the desk */}
      <div class="absolute inset-0 overflow-hidden opacity-20 blur-sm pointer-events-none select-none" aria-hidden="true">
        {/* Static mockup of overview layout */}
        <div class="max-w-[1400px] mx-auto px-8 pt-16">
          <div class="flex gap-0">
            {/* Fake sidebar */}
            <div class="w-56 shrink-0 border-r border-container-border/50 hidden md:block">
              <div class="px-6 py-3 border-b border-container-border/50">
                <div class="font-display text-xs tracking-section text-text-tertiary uppercase">ACCOUNT</div>
                <div class="mt-3 space-y-3">
                  <div class="flex justify-between">
                    <span class="font-mono text-[10px] text-text-tertiary">Total P&L</span>
                    <span class="font-mono text-xs text-signal-green">$12,847.32</span>
                  </div>
                  <div class="flex justify-between">
                    <span class="font-mono text-[10px] text-text-tertiary">Win Rate</span>
                    <span class="font-mono text-xs text-text-primary">68.2%</span>
                  </div>
                  <div class="flex justify-between">
                    <span class="font-mono text-[10px] text-text-tertiary">Trades</span>
                    <span class="font-mono text-xs text-text-primary">147</span>
                  </div>
                </div>
              </div>
            </div>
            {/* Fake hero area */}
            <div class="flex-1 min-w-0 px-8 py-6">
              <div class="border-l-2 border-accent-primary pl-6 mb-8">
                <span class="font-mono text-5xl font-bold text-signal-green">$12,847.32</span>
                <span class="font-mono text-sm text-text-secondary ml-3">net P&L</span>
              </div>
              {/* Fake chart placeholder */}
              <div class="h-64 border border-container-border/30 relative overflow-hidden">
                <svg class="w-full h-full" viewBox="0 0 400 150" preserveAspectRatio="none">
                  <polyline fill="none" stroke="rgb(var(--accent-primary))" stroke-width="2" points="0,120 40,110 80,95 120,100 160,80 200,70 240,55 280,60 320,40 360,30 400,20" />
                </svg>
              </div>
            </div>
          </div>
        </div>
      </div>

      {/* Auth card (on top) */}
      <div class="relative z-10 flex flex-col items-center justify-center min-h-[calc(100vh-var(--header-h))] px-6">
        <div class="border border-container-border bg-main-bg/75 backdrop-blur-md p-10 md:p-14 max-w-lg w-full text-center">
          {/* ... existing lock screen content unchanged ... */}
        </div>
      </div>
    </div>
  )
}
```

Key decisions:
- **Static mockup, not live data** — no API calls, no auth required, no data leakage
- **`opacity-20 blur-sm`** — visible enough to suggest the product, not readable
- **`aria-hidden="true" pointer-events-none select-none`** — invisible to screen readers, not interactive
- **Fake data uses realistic but generic numbers** — $12,847.32 P&L, 68.2% win rate, upward equity curve
- **SVG polyline for chart** — lightweight, no ECharts needed, just suggests a line going up
- **Hidden on mobile** (`hidden md:block` on sidebar) — preview too small to be useful on phones

### Paved Roads

- `pnlColor()` and `rColor()` in `lib/formatters.ts` — existing signal color helpers
- `getTokenColor()` or CSS variable reading pattern in `lib/tokens.ts` — for chart accent color
- `ChartContainer` already has empty/loading/error states — just enhancing the empty copy
- `glass-panel` and `backdrop-blur` patterns already used throughout

### Files

All in `testudo-journal/src/`:

- `components/Overview.tsx` — **modified** — accent border on hero P&L section
- `components/HeroEquityCurve.tsx` — **modified** — line color to accent-primary
- `components/charts/ChartContainer.tsx` — **modified** — enhanced empty state copy
- `components/trades/TradeTable.tsx` — **modified** — actionable empty state with filter awareness
- `components/journal/JournalTimeline.tsx` — **modified** — journal empty state guidance
- `components/Layout.tsx` — **modified** — lock screen preview mockup behind auth card

### Dependencies Added

None.

---

## Acceptance Criteria

- [ ] Overview hero P&L has accent-primary left border — visually distinct focal point
- [ ] Equity curve line renders in accent-primary (rust/copper), not default blue/white
- [ ] TradeTable empty state says "NO TRADES YET" with guidance text and Account link
- [ ] TradeTable with active filters shows "NO MATCHING TRADES" with filter hint
- [ ] ChartContainer empty state includes "Data populates as you close trades"
- [ ] JournalTimeline empty state guides users to open a trade's detail panel
- [ ] Lock screen shows dimmed/blurred static preview of overview with fake data
- [ ] Lock screen preview is `aria-hidden`, not interactive, no API calls
- [ ] No functional regression — auth, navigation, data loading all unchanged
- [ ] `cd testudo-journal && bun run build` passes

---

## Risks

1. **Accent color on hero P&L may clash with green/red signal colors** — Using a left-border accent instead of text color avoids this. The number stays green/red; the container is framed by rust/copper. Verify visually in both themes.
2. **Lock screen mockup maintenance** — Static fake data can drift from actual UI. Mitigation: keep it minimal (just layout shapes and one number), so it doesn't need updating when real components change.
3. **Empty state copy length** — Too much text in an empty state feels heavy. Keep it to 2 lines max: what's missing + what to do about it.

---

## Completion Signal

This spec is complete when:
1. Overview has a clear visual focal point via accent color
2. Empty states guide users instead of dead-ending
3. Lock screen previews the product experience
4. `bun run build` passes
5. Code committed to master
