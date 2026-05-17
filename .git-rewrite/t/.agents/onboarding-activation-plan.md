# Onboarding Activation Plan: Time-to-First-Trade

*Goal: Get every new user from wallet connect to first Alt+X trade in under 5 minutes.*

## North Star Metric
**Activation rate:** % of wallet-connected users who execute their first trade via Alt+X within 24 hours.

## Redesigned Onboarding Steps

### Step 1: Connect Wallet (5 seconds)
**Current:** Works well. SIWE/SIWS one-click.
**Keep as-is.** This is already low friction.

### Step 2: Add Exchange (60 seconds)
**Current:** Works. Exchange selection + API key entry.
**Improvement:** Default to Hyperliquid (wallet-based, no API keys = zero friction for DeFi users). Show it first in the dropdown. For CEX users, keep the API key flow.

**Copy change for empty state:**
```
Before: "Connect an exchange to enable trading."
After:  "Add your exchange. Hyperliquid connects in one click.
         CEX users: API key with trade-only permissions."
```

### Step 3: Install & Pair Extension (30 seconds)
**Current:** Two separate mental steps (install from store, then pair with code).
**Improvement:** Merge into one step. Show direct install link. After install, extension auto-detects the desk session and pairs via the pairing code flow that's already built.

**Copy:**
```
"Install the Testudo extension for Chrome or Firefox.
 Once installed, click PAIR above to link it to this wallet."
```

**Key:** Show the Chrome Web Store link as a prominent button, not a text link. After clicking, show a "waiting for extension..." state that auto-resolves when pairing completes.

### Step 4: Execute Your First Trade (THE ACTIVATION EVENT)
**This step doesn't exist today. Add it.**

**Two paths:**

**Path A — Live trade (for funded accounts):**
```
"Open TradingView. Draw a Long or Short Position tool.
 Set your entry, stop-loss, and take-profit.
 Press Alt+X. Confirm with double-Enter.
 Your first risk-managed trade is live."
```

**Path B — Paper trade / demo (for unfunded or cautious users):**
```
"Not ready for live? Try a test trade on Hyperliquid testnet.
 Same workflow, zero risk. Switch to mainnet when ready."
```

**Celebration on completion:**
```
"FIRST TRADE EXECUTED
 Your formation is active. Every trade from here is sized,
 bracketed, and journaled automatically.

 [VIEW ON DESK]  [TRADE AGAIN]"
```

## Stepper UI Changes

Replace the current 4-step stepper with:

```
[1] CONNECT  →  [2] EXCHANGE  →  [3] EXTENSION  →  [4] FIRST TRADE
     wallet        add one         install+pair       Alt+X
```

- Remove "Import History" as a visible step (it's automatic, runs in background)
- Add "First Trade" as the final activation step
- Show a progress bar: "3 of 4 complete — one trade away from full activation"
- After step 4 completes, collapse stepper permanently with a "Formation active" badge

## Empty State Improvements

### Overview page (no trades yet):
```
Before: "No trades this month"
After:  "YOUR FORMATION AWAITS

         No trades recorded yet. Open TradingView, draw your
         setup, and press Alt+X to execute your first
         risk-managed trade.

         [OPEN TRADINGVIEW]  [HOW IT WORKS →]"
```

### Calendar (no trades):
```
Before: "No trades this month. Closed trades will appear here
         automatically. Try navigating to a month with activity."
After:  "YOUR P&L CALENDAR

         Each day you trade will appear here with your daily P&L.
         Green days. Red days. Weekly totals. The data builds
         automatically — one Alt+X at a time.

         [EXECUTE FIRST TRADE →]"
```

### Journal (empty):
```
Before: (generic empty)
After:  "THE JOURNAL IS THE MIRROR

         Every trade you execute through Testudo is logged here
         with full analytics — R-multiple, duration, P&L.
         Write your thesis before. Review the outcome after.

         No manual entry required. Just trade.

         [YOUR FIRST TRADE STARTS HERE →]"
```

## Friction Reduction Checklist

- [ ] Default exchange selection to Hyperliquid (lowest friction — wallet auth, no API keys)
- [ ] Chrome Web Store link opens in new tab, desk polls for extension connection
- [ ] Remove "Import History" as visible step (background task)
- [ ] Add "First Trade" activation step with guided instructions
- [ ] Celebration modal on first Alt+X execution
- [ ] Empty states across all pages point toward first trade
- [ ] Progress indicator visible until activation completes
- [ ] "How Alt+X works" expandable section on step 4 (30-second visual)

## Metrics Plan

| Metric | Target | Current (est) |
|--------|--------|---------------|
| Wallet → Exchange added | 80% | ~70% |
| Exchange → Extension paired | 70% | ~50% |
| Extension → First trade | 60% | ~25% |
| Overall activation (wallet → first trade) | 35% | ~10% |
| Time to first trade | < 5 min | Unknown |
| Day 1 retention (return after first trade) | 60% | Unknown |
