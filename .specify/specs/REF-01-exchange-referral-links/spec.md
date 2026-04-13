# Specification: Exchange Referral Links in Add Exchange Flow

**Spec ID:** REF-01-exchange-referral-links
**Date:** 2026-04-13
**Status:** Draft
**Class:** Feature / Growth
**Priority:** P2 — revenue generation via exchange affiliate programs
**Depends on:** None
**Series:** REF-01 (standalone)

---

## Problem Statement

When users add an exchange on the desk account page, there's no path for users who don't yet have an exchange account. The form assumes they already have API keys. Users who need to create an account navigate away to find the exchange themselves — losing the affiliate opportunity.

The onboarding flow links to docs (`testudo.vip/docs/07-exchanges`) but not to exchange signup pages directly.

This spec adds hardcoded personal referral links into the Add Exchange form, shown conditionally after exchange selection. One click opens the exchange signup with the ref code. Zero backend changes — purely frontend.

---

## User Stories

- **As a new user**, I want a direct link to create an exchange account when I don't have one, so that I can start trading without hunting for signup pages.
- **As the platform owner**, I want my referral links embedded in the exchange setup flow, so that I earn affiliate revenue from users who create accounts through Testudo.

---

## Functional Requirements

| ID | Requirement | Priority | Subsystem |
|----|-------------|----------|-----------|
| FR-1 | After selecting an exchange in AddExchangeForm, show a "Don't have an account?" link that opens the exchange's signup page with ref code in a new tab | High | testudo-journal |
| FR-2 | Ref links are stored in a single constant map (`exchange_id → referral_url`) | High | testudo-journal |
| FR-3 | Link only appears after exchange selection, not in the default "Select exchange..." state | Medium | testudo-journal |
| FR-4 | Same referral link shown in OnboardingFlow (first-time users) since it renders AddExchangeForm | Medium | testudo-journal |
| FR-5 | Hyperliquid selection shows a ref link too (before the WalletConnectFlow) | Medium | testudo-journal |

---

## Technical Implementation

### Vertical Checkpoints

| Checkpoint | Scope | Validates |
|------------|-------|-----------|
| CP-1 | Add referral URL map + render link in AddExchangeForm | Link appears after exchange selection, opens correct URL |
| CP-2 | Verify in OnboardingFlow context + Hyperliquid path | Works for first-time users and DEX selection |

### Referral URL Map

Single source of truth — a constant in `AddExchangeForm.tsx`:

```typescript
const EXCHANGE_REFERRAL_URLS: Record<string, string> = {
  binance:     'https://accounts.binance.com/register?ref=XXXXX',
  bybit:       'https://www.bybit.com/register?affiliate_id=XXXXX',
  okx:         'https://www.okx.com/join/XXXXX',
  woo:         'https://x.woo.org/register?ref=XXXXX',
  hyperliquid: 'https://app.hyperliquid.xyz/join/XXXXX',
}
```

> Replace `XXXXX` with actual affiliate codes before deploying.

### UI Injection Point

In `AddExchangeForm.tsx`, between the exchange `<select>` (line 89) and the conditional Hyperliquid/CEX blocks (line 91):

```tsx
{/* Referral link — shown after exchange selection */}
<Show when={selectedExchange() && EXCHANGE_REFERRAL_URLS[selectedExchange()]}>
  <a
    href={EXCHANGE_REFERRAL_URLS[selectedExchange()]}
    target="_blank"
    rel="noopener noreferrer"
    class="block font-mono text-[10px] text-text-tertiary hover:text-accent-steel transition-colors"
  >
    Don't have an account? Sign up on {
      props.exchanges.find(e => e.id === selectedExchange())?.name ?? selectedExchange()
    } &rarr;
  </a>
</Show>
```

Renders for all 5 exchanges including Hyperliquid, positioned between dropdown and the API key form / WalletConnectFlow.

### Paved Roads

- SolidJS `Show` / `For` conditional rendering — used throughout `AddExchangeForm.tsx`
- `ExchangeInfo.name` provides display names ("Binance", "WOO X", etc.) — from `GET /api/v1/exchanges`
- External link pattern with `target="_blank"` — already used in `OnboardingFlow.tsx:80-87`

### Files

- `testudo-journal/src/components/account/AddExchangeForm.tsx` — add referral URL map + conditional `<a>` tag after exchange dropdown

**One file, ~15 lines added.**

### Dependencies Added

None.

---

## Acceptance Criteria

- [ ] Selecting any exchange shows a referral link below the dropdown
- [ ] Link text includes the exchange display name (e.g., "Sign up on Bybit")
- [ ] Link opens correct referral URL in new tab
- [ ] No link shown when "Select exchange..." placeholder is active
- [ ] Hyperliquid selection shows ref link above WalletConnectFlow
- [ ] OnboardingFlow inherits the behavior (uses same AddExchangeForm component)
- [ ] `cd testudo-journal && bun run build` passes

---

## Risks

1. **Ref codes change** — Exchange affiliate programs may change URL formats or codes. Mitigation: all URLs in one constant map, easy to update.
2. **Exchange rejects ref format** — Some exchanges have specific URL parameter requirements. Mitigation: test each link manually before committing real codes.

---

## Completion Signal

This spec is complete when:
1. Referral links appear in the Add Exchange form for all 5 exchanges
2. All acceptance criteria met
3. `bun run build` passes in testudo-journal
4. Code committed to master
