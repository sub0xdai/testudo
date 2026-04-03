# Testudo — Store Listing Copy

## Firefox Add-ons

### Name
Testudo

### Summary (max 250 chars)
Automated risk management for crypto traders. Position sizing, stop-loss enforcement, and one-click execution from TradingView, DexScreener, and Hyperliquid.

### Description (Markdown supported)
Testudo is a risk management overlay for crypto traders. It connects to TradingView, DexScreener, and Hyperliquid charts and lets you execute trades with automated position sizing, stop-loss enforcement, and daily drawdown limits.

**How it works:**

1. Draw a Long/Short position tool on any TradingView chart
2. Press Alt+X — entry, stop-loss, and take-profit are scraped directly from your drawing
3. Review the pre-filled trade confirmation modal
4. Double-Enter to confirm — your order is placed with full risk management

**Features:**

- Risk-based position sizing — calculates optimal size from your stop distance and account risk percentage
- Break-even automation — moves stop to entry after price moves in your favor
- Trailing stops — locks in profit as price extends
- Partial take-profit — scale out at predefined levels
- Order groups — entry, stop-loss, and take-profit managed as a single atomic unit
- Real-time fill detection via WebSocket
- Position rehydration — survives browser restarts
- Shadow engine for paper trading with real market data

**Supported exchanges:**

- Hyperliquid (native SDK)
- Binance Futures (via CCXT)
- WOO X (via CCXT)
- Any CCXT-compatible exchange

**Privacy:**

Testudo stores your authentication token locally. No tracking, no analytics, no data collection. Your keys never leave your browser. See our privacy policy at https://testudo.vip/privacy.

### Categories
- Web Development (closest fit — no "Finance" or "Trading" category exists)

### Tags
trading, crypto, risk-management, tradingview, position-sizing, hyperliquid, defi

### Email
support@testudo.vip

### Website
https://testudo.vip

### Homepage (Additional Details)
https://testudo.vip

---

## Chrome Web Store

### Name
Testudo

### Short Description (max 132 chars)
Risk management overlay for crypto traders. Position sizing, circuit breakers, and one-click execution from TradingView.

### Detailed Description
(Same as Firefox description above)

### Category
Productivity

### Language
English (US)

---

## Changes to Make on Firefox Developer Hub

1. Update Description field with the text above
2. Change Category from "Shopping" to "Web Development"
3. Uncheck "Experimental" when ready
4. Add Tags: trading, crypto, risk-management, tradingview, position-sizing, hyperliquid, defi
5. Set Homepage to https://testudo.vip
6. Upload new icon (128px shield/crest from assets/brand/)
7. Upload 5 screenshots from store-assets/*.png
8. Note: UUID cannot be changed after creation — testudo-sniper@sub0xdai stays
