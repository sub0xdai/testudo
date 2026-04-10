# Welcome Email Sequence: Time-to-First-Trade

## Sequence Overview
```
Sequence Name: Welcome — The Formation
Trigger: Wallet connected (user created)
Goal: First Alt+X trade within 24 hours
Length: 3 emails
Timing: Immediate → +6 hours → +24 hours
Exit Condition: User executes first trade via Alt+X (activation event)
```

**Important:** This is a wallet-auth product. We don't collect email at signup — we collect a wallet address. These emails would need to be delivered via:
- **In-app notifications** (notification center on the desk)
- **Browser push notifications** (via extension, if permitted)
- **Email** (only if we add optional email collection post-signup)

For the purpose of this document, I'll write them as emails, but they should be designed to work as any notification format.

---

## Email 1: The Formation Holds

```
Send: Immediately after wallet connect
Subject: Your shield wall is ready.
Preview: One keystroke between your chart and your exchange.
```

**Body:**

```
Legionary,

Your wallet is connected. The formation is assembling.

Here's what happens next — it takes about 3 minutes:

━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

STEP 1: ADD YOUR EXCHANGE

Go to Account on the Desk. Click Add Exchange.

• Hyperliquid — connects via wallet signing. No API keys.
  One click. Recommended for your first connection.

• Binance / Bybit / WOO / OKX — API key + secret.
  Trade-only permissions. AES-256-GCM encrypted.

STEP 2: INSTALL THE EXTENSION

Install Testudo for Chrome or Firefox. Then click PAIR
on the Desk to link it to your wallet.

The extension lives on your TradingView tab. It reads
your chart. It talks to your exchange. You press Alt+X.

━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

That's it. Two steps and you're ready to trade.

When you're set up, open TradingView, draw a position
tool (entry + stop + target), and press Alt+X.

Your first risk-managed trade. Under one second.

→ OPEN THE DESK
  https://desk.testudo.vip

— Testudo
   You bring the edge. We enforce the discipline.
```

**CTA:** `OPEN THE DESK` → `https://desk.testudo.vip`

**Design notes:**
- Plain text aesthetic (matches brutalist brand)
- Monospace font throughout
- No images, no HTML chrome — just text with structure
- The "━━━" dividers match the terminal feel
- Short paragraphs, scannable

---

## Email 2: The First Trade

```
Send: 6 hours after signup (ONLY if user hasn't activated)
Segment: Connected wallet + added exchange BUT has not executed first trade
Subject: Alt+X. That's the whole workflow.
Preview: Draw on TradingView. Press one key. Trade sized and placed.
```

**Body:**

```
You've connected your exchange. Good.

Now the part that matters: your first trade.

━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

THE ALT+X WORKFLOW

1. Open TradingView
2. Draw a Long Position or Short Position tool
   (set entry, stop-loss, take-profit on the chart)
3. Press Alt+X
4. Review the confirmation modal — it shows:
   • Position size (calculated from your stop distance)
   • Risk amount (% of account)
   • Entry, SL, TP prices
   • R:R ratio
5. Press Enter twice to confirm
6. Bracket order placed on your exchange

Total time: under 1 second.

━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

WHY DOUBLE-ENTER?

Single Enter arms the button (turns green).
Second Enter confirms.

This is intentional friction. Live trading is real money.
One misclick shouldn't cost you. Two deliberate keystrokes
means you meant it.

━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

NOT READY FOR LIVE?

If your account isn't funded yet, or you want to test
the workflow first:

• Hyperliquid testnet — same Alt+X, zero risk
• Set a very small position size limit in your
  risk settings to trade with minimal exposure

The point isn't the size. The point is feeling
the speed. One trade and you'll understand.

→ OPEN TRADINGVIEW
  https://www.tradingview.com/chart/

— Testudo
```

**CTA:** `OPEN TRADINGVIEW` → TradingView chart URL

**Psychology applied:**
- **Present Bias:** "under 1 second" — immediate gratification
- **Activation Energy:** The exact step-by-step removes ambiguity about what to do
- **Regret Aversion:** "Not ready for live?" section addresses the fear without judgment
- **IKEA Effect:** "Draw YOUR setup" — they built it, they own it
- **Commitment & Consistency:** They already connected a wallet and exchange — this is the next logical step in a chain they started

---

## Email 3: The Journal Is The Mirror

```
Send: 24 hours after signup (ONLY if user hasn't activated)
Segment: Signed up but has NOT executed first trade
Subject: The patterns in your data will tell you truths your ego never will.
Preview: Every trade you execute is logged automatically. No spreadsheets.
```

**Body:**

```
You signed up 24 hours ago. You haven't traded yet.

That's fine. But here's what you're missing:

━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

WHAT HAPPENS AFTER YOUR FIRST TRADE

Every trade through Testudo is logged automatically:
• Entry price, exit price, fees
• Position size and risk amount
• R-multiple (profit relative to risk)
• Hold duration
• P&L in dollars and percentage

No CSV imports. No spreadsheet formulas.
No "I'll journal it later" (you won't).

After 10 trades, you'll see:
• Your P&L calendar — green days, red days, weekly totals
• Equity curve — your account trajectory over time
• Win rate, profit factor, expectancy
• Dignitas score — your composite performance rating

After 50 trades, you'll know:
• Which setups actually work (not which ones feel good)
• What time of day you trade best
• Whether your edge is real or imagined

━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

THE FORMATION

"Most traders don't lose because they can't read
a chart. They lose because they can't manage
themselves."

The formation doesn't start with perfect entries.
It starts with one properly-sized trade. Then another.
Then a pattern emerges from the data.

Your chart is open. Your exchange is connected.
Your extension is paired.

One keystroke.

→ EXECUTE YOUR FIRST TRADE
  https://desk.testudo.vip

— Testudo
   Adapt. Outlast. Don't break.
```

**CTA:** `EXECUTE YOUR FIRST TRADE` → Desk URL

**Psychology applied:**
- **Loss Aversion:** "Here's what you're missing" — framed as a loss, not a gain
- **Zeigarnik Effect:** "You signed up but haven't traded" — the open loop creates tension
- **Goal-Gradient Effect:** "After 10 trades... After 50 trades..." — shows the compounding value of starting
- **Commitment & Consistency:** They already took 3 steps (wallet, exchange, extension). The 4th is the payoff.
- **Social proof via data:** "The patterns in your data will tell you truths your ego never will" — quotes from the manifesto they already believe in (they signed up because of it)

---

## Sequence Behavior Rules

### Exit conditions:
- User executes first Alt+X trade → exit sequence, trigger "Activation Celebration" event
- User unsubscribes → exit
- After email 3, sequence ends regardless

### Branch logic:
```
Email 1 (immediate)
  ↓
  Wait 6 hours
  ↓
  Check: has_first_trade?
  ├── YES → Exit sequence. Send "Formation Active" celebration.
  └── NO  → Send Email 2
             ↓
             Wait 18 hours (total 24h from signup)
             ↓
             Check: has_first_trade?
             ├── YES → Exit. Send celebration.
             └── NO  → Send Email 3. End sequence.
```

### Post-activation celebration (sent when first trade completes):

```
Subject: Formation active. ⬡
Preview: Your first trade is logged. The data starts now.

━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

FIRST TRADE EXECUTED

Symbol:    [SYMBOL]
Side:      [LONG/SHORT]
Size:      [QUANTITY]
Risk:      [RISK_AMOUNT] ([RISK_PCT]%)
R:R:       [RISK_REWARD_RATIO]

Your trade is live. SL and TP are set.
When it closes, the journal logs everything.

→ VIEW YOUR TRADE ON THE DESK

━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

WHAT NEXT?

• Trade more. Each trade adds data.
• After your first close, check the Overview
  for your initial stats.
• Write a thesis in the Journal —
  why did you take this trade?

The formation holds.

— Testudo
```

---

## Metrics Plan

| Metric | Benchmark | Target |
|--------|-----------|--------|
| Email 1 open rate | 60-80% (welcome emails) | 70% |
| Email 2 open rate | 40-50% | 45% |
| Email 3 open rate | 30-40% | 35% |
| Email 1 → first trade (same day) | 15-20% | 20% |
| Email 2 → first trade (within 6h) | 10-15% | 12% |
| Email 3 → first trade (within 24h) | 5-10% | 8% |
| Overall sequence → activation | 25-35% | 30% |

---

## Implementation Notes

### Email delivery challenge:
Testudo uses wallet-based auth — no email collected. Options:
1. **Optional email field** post-signup: "Get setup help by email (optional)"
2. **Browser push via extension**: Extension can send notifications
3. **In-app notification center**: Build a simple inbox on the desk
4. **Telegram/Discord bot**: Common in DeFi — wallet-linked bot sends DMs

**Recommendation:** Start with **in-app notifications** (lowest friction, no email required) + **optional email capture** on the Account page. DeFi users resist email but will accept in-app notifications and Discord.

### Tone calibration:
- Monospace, plain-text aesthetic — no HTML templates with headers/footers
- Roman/military metaphors woven naturally, not forced
- Technical but not condescending — these are traders, not beginners
- Direct. No "Hey there!" or "Hope you're doing well!"
- The manifesto's voice ("The formation holds") is the email's voice
