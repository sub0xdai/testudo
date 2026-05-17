# Specification: Redesign Account Page — Smart Card Grid Layout

**Spec ID:** EXT-40-smart-card-grid
**Date:** 2026-03-24
**Status:** Draft
**Class:** Refactor / Frontend UI
**Priority:** P1 — Current stacked list feels like a wireframe; card grid adds visual trust and prevents destructive misclicks
**Depends on:** AUTH-03-frontend-auth (wallet-primary AccountPage exists)
**Series:** EXT-40 (standalone)

---

## Problem Statement

The current `AccountPage.tsx` (532 lines) renders exchange accounts as stacked horizontal rows with flat button hierarchy. TEST, DEL, and REVOKE buttons share identical wireframe styling and proximity, creating misclick risk on a trading platform where accidentally revoking an agent wallet has real financial consequences.

The page also feels "dead" — it displays static configuration data (exchange name, auth mode) without surfacing connection health or portfolio metrics. The `is_active` field from `ExchangeAccount` is available but not visualized.

This is a **UI-only refactor**. All existing functionality (add account, test connection, delete, revoke agent, migrate to agent wallet, WalletConnect EIP-712 flow, extension pairing) remains unchanged. The same React state, the same API calls, the same handlers. Only the JSX layout and component structure changes.

**Framework: React 18 + TypeScript + Tailwind (design tokens)**. NOT Solid.js — that's the extension only.

---

## User Stories

- **As a trader**, I want to see connection health (green/red dot) at a glance, so I know my integrations are alive before executing trades.
- **As a user**, I want destructive actions (DELETE, REVOKE) hidden behind a kebab menu, so I can't accidentally fat-finger a revocation.
- **As a user**, I want adding a new exchange to feel like a natural grid action (ghost card with `+`), not a buried form toggle.
- **As a user**, I want extension pairing demoted to a compact banner, so my primary focus stays on exchange connections.

---

## Functional Requirements

| ID | Requirement | Priority | Subsystem |
|----|-------------|----------|-----------|
| FR-1 | Refactor exchange account list into responsive CSS grid: 1 col mobile, 2 col `md:`, 3 col `lg:` | High | AccountPage |
| FR-2 | Extract each exchange account into an `ExchangeCard` component with: heartbeat dot (`is_active`), exchange name, auth mode badge (CEX/DEX), wallet/API key identifier, balance placeholder | High | ExchangeCard |
| FR-3 | Move TEST/DELETE/REVOKE actions into a kebab menu (`⋮`) dropdown with click-outside-to-close | High | ExchangeCard |
| FR-4 | Destructive actions (DELETE, REVOKE) styled in `signal-red`; benign actions (TEST) in default color | High | ExchangeCard |
| FR-5 | Add dashed ghost card at the end of the grid — clicking opens the existing add-exchange form | Medium | AccountPage |
| FR-6 | Demote `ExtensionPairing` to a compact full-width banner below the grid | Medium | AccountPage |
| FR-7 | Widen container from `max-w-2xl` to `max-w-5xl` to accommodate 3-column grid | Medium | AccountPage |
| FR-8 | Show migration prompt (direct-key → agent wallet) inside the card, not as a separate row | Low | ExchangeCard |

---

## Technical Implementation

### 1. Component Extraction

The 532-line `AccountPage.tsx` has the exchange list rendering inline. Extract an `ExchangeCard` component that receives props — no new state, no new API calls.

```tsx
// testudo-web/src/components/ExchangeCard.tsx
interface ExchangeCardProps {
  account: ExchangeAccount;
  testResult?: TestConnectionResult;
  isTesting: boolean;
  isDeleting: boolean;
  isRevoking: boolean;
  onTest: (id: string) => void;
  onDelete: (id: string) => void;
  onRevoke: (id: string) => void;
  onMigrate: (id: string) => void;
}
```

All handlers stay in `AccountPage` — the card just calls them via props. No state duplication.

### 2. Card Layout

```tsx
<div className="border border-container-border bg-container-bg p-5 flex flex-col gap-4">
  {/* Header: heartbeat + name + badge + kebab */}
  <div className="flex justify-between items-start">
    <div className="flex items-center gap-3">
      {/* Heartbeat dot */}
      <span className={`inline-block w-2.5 h-2.5 rounded-full ${
        account.is_active ? 'bg-signal-green animate-pulse' : 'bg-signal-red'
      }`} />
      <h3 className="font-mono text-sm font-bold text-text-primary tracking-wider uppercase">
        {account.exchange_name}
      </h3>
      <span className="text-[10px] text-text-tertiary font-mono bg-main-bg px-2 py-0.5 border border-container-border">
        {account.auth_mode === 'agent_wallet' ? 'DEX' : 'CEX'}
      </span>
    </div>
    <KebabMenu ... />
  </div>

  {/* Identifier */}
  <span className="text-xs text-text-tertiary font-mono truncate">
    {account.wallet_address
      ? `${account.wallet_address.slice(0, 6)}...${account.wallet_address.slice(-4)}`
      : account.account_name}
  </span>

  {/* Balance placeholder — shows test result or "---" */}
  <div className="font-mono text-xl text-text-primary mt-auto">
    {testResult?.status === 'success' ? testResult.message : '---'}
  </div>
</div>
```

### 3. Kebab Menu

Simple local `useState` toggle with click-outside listener via `useEffect` + `useRef`. No portal needed — the card doesn't use `overflow-hidden`.

```tsx
function KebabMenu({ onTest, onDelete, onRevoke, showRevoke, isTesting, isDeleting, isRevoking }) {
  const [open, setOpen] = useState(false);
  const ref = useRef<HTMLDivElement>(null);

  useEffect(() => {
    if (!open) return;
    const handler = (e: MouseEvent) => {
      if (ref.current && !ref.current.contains(e.target as Node)) setOpen(false);
    };
    document.addEventListener('mousedown', handler);
    return () => document.removeEventListener('mousedown', handler);
  }, [open]);

  return (
    <div ref={ref} className="relative">
      <button onClick={() => setOpen(!open)} className="text-text-tertiary hover:text-text-primary px-2 text-lg">⋮</button>
      {open && (
        <div className="absolute right-0 mt-1 w-36 bg-container-bg border border-container-border z-10 flex flex-col">
          <button onClick={() => { onTest(); setOpen(false); }}
            className="text-left px-4 py-2.5 text-xs font-mono text-text-secondary hover:bg-main-bg">
            {isTesting ? 'TESTING...' : 'TEST CONNECTION'}
          </button>
          {showRevoke && (
            <button onClick={() => { onRevoke(); setOpen(false); }}
              className="text-left px-4 py-2.5 text-xs font-mono text-signal-red hover:bg-signal-red/10 border-t border-container-border">
              {isRevoking ? 'REVOKING...' : 'REVOKE AGENT'}
            </button>
          )}
          <button onClick={() => { onDelete(); setOpen(false); }}
            className="text-left px-4 py-2.5 text-xs font-mono text-signal-red hover:bg-signal-red/10 border-t border-container-border">
            {isDeleting ? 'DELETING...' : 'DELETE'}
          </button>
        </div>
      )}
    </div>
  );
}
```

### 4. Ghost Card (Add Exchange)

```tsx
<button
  onClick={() => setShowForm(true)}
  className="border border-dashed border-container-border bg-transparent p-5 flex flex-col items-center justify-center gap-3 min-h-[160px] hover:border-text-tertiary transition-colors group"
>
  <span className="text-2xl text-text-tertiary group-hover:text-text-secondary">+</span>
  <span className="text-xs font-mono text-text-tertiary group-hover:text-text-secondary tracking-wider">
    ADD EXCHANGE
  </span>
</button>
```

### 5. Extension Pairing Banner

The existing `ExtensionPairing.tsx` component is already compact. Wrap it in a full-width container below the grid with reduced visual weight:

```tsx
<div className="mt-8 border-t border-container-border pt-6">
  <ExtensionPairing />
</div>
```

### 6. Grid Container

```tsx
{/* Replace max-w-2xl with max-w-5xl */}
<div className="max-w-5xl mx-auto px-6 pt-24 pb-16">
  {/* Header with wallet address + logout */}
  ...

  {/* Card Grid */}
  <div className="grid grid-cols-1 md:grid-cols-2 lg:grid-cols-3 gap-4 mt-8">
    {accounts.map(account => (
      <ExchangeCard
        key={account.id}
        account={account}
        testResult={testResults[account.id]}
        isTesting={testingId === account.id}
        isDeleting={deletingId === account.id}
        isRevoking={revokingId === account.id}
        onTest={() => handleTest(account.id)}
        onDelete={() => handleDelete(account.id)}
        onRevoke={() => handleRevoke(account.id)}
        onMigrate={() => handleMigrate(account.id)}
      />
    ))}
    <AddExchangeGhostCard onClick={() => setShowForm(true)} />
  </div>

  {/* Extension Pairing — demoted below grid */}
  <div className="mt-8 border-t border-container-border pt-6">
    <ExtensionPairing />
  </div>
</div>
```

### 7. Migration Prompt (Inside Card)

For Hyperliquid accounts with `auth_mode === 'direct'` (not agent wallet), show a subtle prompt inside the card body:

```tsx
{account.exchange_name === 'hyperliquid' && account.auth_mode !== 'agent_wallet' && (
  <button onClick={() => onMigrate(account.id)}
    className="text-[10px] font-mono text-signal-amber hover:underline">
    Migrate to agent wallet →
  </button>
)}
```

### Files

- `testudo-web/src/components/ExchangeCard.tsx` — **new** — card component with heartbeat, kebab menu, balance, migration prompt
- `testudo-web/src/pages/AccountPage.tsx` — **modified** — replace inline list with grid + ExchangeCard components, widen container
- `testudo-web/src/components/ExtensionPairing.tsx` — **unchanged** — just repositioned in layout

### Dependencies Added

- None

---

## Acceptance Criteria

- [ ] Account page renders a responsive grid (`grid-cols-1` / `md:2` / `lg:3`)
- [ ] Each exchange account renders as a card with heartbeat dot, name, badge, identifier
- [ ] `is_active === true` shows pulsing green dot; `false` shows static red dot
- [ ] Kebab menu (`⋮`) opens dropdown with TEST, DELETE, and conditionally REVOKE
- [ ] Clicking outside the kebab dropdown closes it
- [ ] DELETE and REVOKE buttons styled in `text-signal-red`
- [ ] Ghost "ADD EXCHANGE" card with dashed border renders at end of grid
- [ ] Clicking ghost card opens the existing add-exchange form
- [ ] Extension pairing renders as compact banner below the grid
- [ ] Container uses `max-w-5xl` (was `max-w-2xl`)
- [ ] All existing functionality works: add, test, delete, revoke, migrate, WalletConnect flow
- [ ] Page uses design tokens (`container-border`, `text-primary`, `signal-green`, etc.) — no raw Tailwind colors
- [ ] `cd testudo-web && bun run build` passes

---

## Risks

1. **Kebab dropdown clipped by grid** — Grid cells may clip overflow. Mitigation: Cards don't use `overflow-hidden`; dropdown uses `z-10` absolute positioning.
2. **532-line monolith refactor** — Extracting ExchangeCard from inline JSX risks breaking handler wiring. Mitigation: All handlers stay in AccountPage, passed as props. No state moves.
3. **Balance display without API** — There's no dedicated "fetch balance" endpoint in the current API. `testConnection` returns status + latency but not balance. Mitigation: Show `---` by default; show test result message after TEST is clicked. Balance fetch can be added in a future spec.

---

## Completion Signal

This spec is complete when:
1. Exchange accounts render as a responsive card grid with heartbeat indicators
2. Destructive actions are behind kebab menus with red styling
3. Ghost card and extension pairing banner are positioned correctly
4. All existing functionality works unchanged
5. `bun run build` passes
6. Code committed to master
