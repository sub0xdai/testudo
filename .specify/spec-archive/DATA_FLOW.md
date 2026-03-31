# Logical Data Flow Diagram — Testudo

## ASCII DFD

```
╔══════════════════════════════════════════════════════════════════════════════════════════════╗
║  ZONE A: BROWSER DOMAIN                                                                    ║
╠══════════════════════════════════════════════════════════════════════════════════════════════╣
║                                                                                            ║
║  [TradingView DOM]                                                                         ║
║       │                                                                                    ║
║       │ Alt+X keydown                                                                      ║
║       ▼                                                                                    ║
║  (DOM Scraper)─────shape data────▶(Extension Modal)                                        ║
║   6-strategy                      user edits entry/                                        ║
║   fallback                        stop/target                                              ║
║       │                                │                                                   ║
║       │ scraper health                 │ TradeSetup + mgmt prefs                           ║
║       ▼                                ▼                                                   ║
║  |browser.storage|◄────JWT, mode────(Background Worker)◄─────WS frame──────┐              ║
║   local                             │        │       │                      │              ║
║                                     │        │       │                      │              ║
║         ┌──toast notification───────┤        │       │                      │              ║
║         ▼                           │        │       │                      │              ║
║  [User: Content Page]              │        │       │                      │              ║
║                                     │        │       │                      │              ║
║  (Popup UI)◄─sidecar status────────┘        │       │                      │              ║
║   Solid.js     balance, trades              │       │                      │              ║
║                                              │       │                      │              ║
╠══════════════════════════════════════════════╪═══════╪══════════════════════╪══════════════╣
║  ZONE B: BACKEND SERVICES                    │       │                      │              ║
╠══════════════════════════════════════════════╪═══════╪══════════════════════╪══════════════╣
║                                              │       │                      │              ║
║                              HTTP REST       │       │ WebSocket            │              ║
║                    ┌─────────────────────────┘       │                      │              ║
║                    │                                 │                      │              ║
║                    ▼                                 │                      │              ║
║  ┌─(Router / Actix-Web API)──────────────────────┐  │                      │              ║
║  │  /auth/*        → JWT issue/refresh           │  │                      │              ║
║  │  /trades/*      → trade CRUD                  │  │                      │              ║
║  │  /market-data/* → proxy to Binance            │  │                      │              ║
║  │  /exchanges/*   → account CRUD                │  │                      │              ║
║  │  /paper/*       → shadow balances             │  │                      │              ║
║  │  /health/*      → liveness + sidecar check    │  │                      │              ║
║  └───┬──────────┬──────────┬─────────────────────┘  │                      │              ║
║      │          │          │                         │                      │              ║
║      │ order    │ market   │ credentials             │                      │              ║
║      │ request  │ query    │ (AES-256-GCM)           │                      │              ║
║      ▼          │          ▼                         │                      │              ║
║  (Decision Loop)│    (CCXT Client)                   │                      │              ║
║      │          │          │                         │                      │              ║
║      ▼          │          │ POST /balance,           │                      │              ║
║  (Risk Service) │          │ /order, /position        │                      │              ║
║   validate SL   │          ▼                         │                      │              ║
║   max positions │    ┌──────────┐                    │                      │              ║
║   daily drawdown│    │          │                    │                      │              ║
║      │          │    ▼          ▼                    │                      │              ║
║      ▼          │  (CCXT      [Exchange APIs]        │                      │              ║
║  (Position      │   Sidecar)   Binance/WooX/         │                      │              ║
║   Sizer)        │   Node.js    Bybit                 │                      │              ║
║   MIN(acct%,    │   :3100                            │                      │              ║
║    risk,        │                                    │                      │              ║
║    margin)      │                                    │                      │              ║
║      │          │                                    │                      │              ║
║      │ approved │                                    │                      │              ║
║      │ + size   │                                    │                      │              ║
║      ▼          │                                    │                      │              ║
║  ┌───────────────────────────┐                       │                      │              ║
║  │  EXECUTION FORK           │                       │                      │              ║
║  │                           │                       │                      │              ║
║  │  Shadow (paper)    Live   │                       │                      │              ║
║  │      │               │    │                       │                      │              ║
║  │      ▼               ▼    │                       │                      │              ║
║  │ (Shadow Engine) (CCXT API)│                       │                      │              ║
║  │  in-memory       via      │                       │                      │              ║
║  │  BTreeMap        sidecar  │                       │                      │              ║
║  └──────┬──────────────┬─────┘                       │                      │              ║
║         │              │                             │                      │              ║
║         │    fill/     │   fill/                     │                      │              ║
║         │    cascade   │   result                    │                      │              ║
║         ▼              ▼                             │                      │              ║
║  |Shadow RAM|    |PostgreSQL|                        │                      │              ║
║   orders          users, trades                      │                      │              ║
║   positions       exchange accounts                  │                      │              ║
║   balances        klines, risk config                │                      │              ║
║                        │                             │                      │              ║
║                        │ pg_notify                   │                      │              ║
║                        │ "order.{user_id}"           │                      │              ║
║                        ▼                             │                      │              ║
║                   (WS-Stream Server)─────WS frame────┘                      │              ║
║                    LISTEN/NOTIFY                                            │              ║
║                    fan-out to                                               │              ║
║                    subscribers                                              │              ║
║                        │                                                    │              ║
║                        │ WS frame                                           │              ║
║                        ▼                                                    │              ║
║                   [Web Frontend]                                            │              ║
║                    React dashboard                                          │              ║
║                        │                                                    │              ║
║                        │ also subscribes directly                           │              ║
║                        ▼                                                    │              ║
║                   [Binance WS]──aggTrade, depth, kline──────────────────────┘              ║
║                    fstream.binance.com                                                     ║
║                                                                                            ║
╠══════════════════════════════════════════════════════════════════════════════════════════════╣
║  ZONE C: BACKGROUND LOOPS (in-process, Tokio)                                              ║
╠══════════════════════════════════════════════════════════════════════════════════════════════╣
║                                                                                            ║
║  [Binance REST]──ticker JSON──▶(Price Feed Service)──broadcast::PriceTick──┐               ║
║   fapi.binance.com              polls every 2s                             │               ║
║                                      │                                     │               ║
║                                      │ process_price_update                │               ║
║                                      ▼                                     │               ║
║                                 (Shadow Engine)                            │               ║
║                                  fill matching                             │               ║
║                                  SL/TP cascade                             │               ║
║                                      │                                     │               ║
║                                      │ fill event                          ▼               ║
║                                      │                          (Trade Manager Service)    ║
║                                      │                           shadow + live instances   ║
║                                      │                                │                    ║
║                                      │                                │ break-even,        ║
║                                      │                                │ trailing stop,     ║
║                                      │                                │ partial TP         ║
║                                      │                                ▼                    ║
║                                      │                          ManagementEvent            ║
║                                      │                                │                    ║
║                                      │                                │ mpsc channel       ║
║                                      ▼                                ▼                    ║
║                                 pg_notify("order.{user_id}", payload)                      ║
║                                      │                                                     ║
║                                      ▼                                                     ║
║                                 |PostgreSQL|──NOTIFY──▶(WS-Stream)──▶[Clients]             ║
║                                                                                            ║
╚══════════════════════════════════════════════════════════════════════════════════════════════╝
```

## Notation

| Symbol | Meaning |
|--------|---------|
| `[ENTITY]` | External entity (user, API, hardware) |
| `(PROCESS)` | Function, service, or class transforming data |
| `\|STORE\|` | Database, cache, or file |
| `-->` / `▶` | Data flow direction (labeled with data type) |

---

## Architectural SWOT Analysis

### Strengths

1. **Single validation gate prevents bypass.** All order flow — paper and live — funnels through `(Decision Loop)` -> `(Risk Service)` -> `(Position Sizer)`. There is no alternative path to order execution, making risk enforcement structurally guaranteed rather than convention-dependent.

2. **Clean execution fork with shared interface.** The `EXECUTION FORK` between `(Shadow Engine)` and `(CCXT API)` uses the same `ExchangeApi` trait, meaning `(Decision Loop)` output is mode-agnostic. Paper and live paths share identical validation but diverge only at execution — Liskov substitution applied at the architecture level.

3. **PostgreSQL as unified event bus.** The `pg_notify("order.{user_id}")` -> `(WS-Stream Server)` -> WebSocket fan-out path eliminates the need for a separate message broker. The same `|PostgreSQL|` store that persists trades also broadcasts events, reducing operational surface area. The `|Redis|` legacy dependency is being phased out, further simplifying this.

4. **DOM scraper resilience.** `(DOM Scraper)` implements 6 fallback strategies against `[TradingView DOM]` mutations. This defense-in-depth approach means a single TradingView UI update doesn't break the ingress pipeline — only a total DOM restructure would require intervention.

5. **Conservative-wins position sizing.** `(Position Sizer)` applies `MIN(account%, fixed_risk, max_size, margin_capacity)`, meaning the tightest constraint always governs. This is a structurally sound risk ceiling — no single parameter misconfiguration can produce oversized positions.

### Weaknesses

1. **Shadow Engine is volatile.** `|Shadow RAM|` (orders, positions, balances) exists only in process memory behind `Arc<RwLock<ShadowEngine>>`. A router process crash loses all paper trading state. There is no persistence or snapshot mechanism for `|Shadow RAM|` — the flow from `(Shadow Engine)` to `|Shadow RAM|` is a dead end with no durability guarantee.

2. **Dual broadcast path creates coupling.** `(Price Feed Service)` publishes via `broadcast::PriceTick` to both `(Shadow Engine)` (for fill matching) and `(Trade Manager Service)` (for management rules). If `(Price Feed Service)` stalls — e.g., Binance REST rate limit at the `[Binance REST]` -> `(Price Feed Service)` edge — both paper fills and management automation halt simultaneously. There is no independent price source for either consumer.

3. **CCXT Sidecar is an unmonitored SPOF for live trades.** The `(CCXT Client)` -> `(CCXT Sidecar)` -> `[Exchange APIs]` path is the *only* live execution route. The sidecar health check (`/health/sidecar`) is a simple poll with a 30s interval. Between polls, a sidecar crash means live `(Trade Manager Service)` management actions (trailing stop amendments, partial TPs) silently fail until the next health poll surfaces the issue.

4. **Redundant market data proxying.** `[Binance REST]` is queried in two distinct paths: `(Router/API)` serves `/market-data/*` endpoints for the `(Popup UI)` and `[Web Frontend]`, while `(Price Feed Service)` independently polls the same Binance ticker endpoint every 2s. These are separate HTTP clients hitting the same upstream, doubling rate limit consumption without shared caching.

### Opportunities

1. **Snapshot `|Shadow RAM|` to `|PostgreSQL|`.** Adding a periodic or event-driven flush from `(Shadow Engine)` -> `|PostgreSQL|` would make paper state recoverable across restarts. The `pg_notify` infrastructure already exists — shadow state could ride the same `|PostgreSQL|` event path that live trades use, unifying the persistence model.

2. **Consolidate Binance market data into a shared cache.** The `[Binance REST]` -> `(Price Feed Service)` and `[Binance REST]` -> `(Router/API)` paths could be unified through a single `(Market Data Cache)` process that polls once and serves both consumers. This halves Binance API calls and creates a single point to implement rate limit backoff.

3. **CCXT Sidecar circuit breaker.** The `(CCXT Client)` -> `(CCXT Sidecar)` edge could implement a circuit breaker pattern. When consecutive failures exceed a threshold, the `(Trade Manager Service)` live instance could pause management actions and escalate via `pg_notify` to the `(Background Worker)` -> `(Popup UI)` path, giving users immediate feedback rather than waiting for the 30s health poll.

4. **WebSocket bidirectional commands.** Currently `(WS-Stream Server)` -> clients is unidirectional (NOTIFY fan-out). The existing WebSocket connection from `(Background Worker)` could accept upstream commands (cancel order, amend SL) — eliminating the round-trip through HTTP REST for time-sensitive operations.

### Threats

1. **Single point of failure at `(Price Feed Service)`.** This Tokio task is the sole price source for `(Shadow Engine)` fill matching and `(Trade Manager Service)` automation. Its failure silently stops paper trading fills and all management actions (trailing stops, break-even, partial TP). The `broadcast::channel` would simply stop producing — downstream consumers would block indefinitely on `recv()` with no timeout or deadman switch.

2. **TradingView DOM dependency.** The entire ingress pipeline — `[TradingView DOM]` -> `(DOM Scraper)` -> `(Extension Modal)` — depends on a third-party DOM structure that Testudo does not control. Despite 6 fallback strategies, a major TradingView rewrite (e.g., canvas-based rendering replacing DOM elements) would sever the primary data ingress path entirely. The `(Extension Modal)` manual-entry mode (`EXT-13`) mitigates but doesn't eliminate this.

3. **AES-256-GCM key management centralization.** Exchange API credentials flow through `(Router/API)` -> `|PostgreSQL|` encrypted, but the decryption key must be available to `(CCXT Client)` at runtime. Compromise of the router process memory exposes all stored exchange credentials simultaneously — the `|PostgreSQL|` -> `(CCXT Client)` -> `(CCXT Sidecar)` -> `[Exchange APIs]` path becomes an attack surface amplifier.

4. **`|browser.storage|` token exposure.** JWT tokens stored in `|browser.storage|` are accessible to any extension with sufficient permissions. The `(Background Worker)` -> `(Router/API)` authentication path relies entirely on the browser storage boundary — no additional token binding (e.g., to extension ID or device fingerprint) exists to prevent token theft from a compromised browser profile.
