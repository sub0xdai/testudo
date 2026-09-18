# Testudo × Tealstreet Integration

How Testudo can execute trades on Tealstreet — and why you'd want to.

## The landscape

| | Testudo | Tealstreet |
|---|---|---|
| **What it is** | Risk-managed trading platform + agent harness | Crypto futures trading terminal |
| **Execution surface** | REST API (`POST /api/v1/signals`), WebSocket, CLI | CLI (`tealstreet buy/sell/close/...`), cross-trade engine |
| **Risk engine** | 8 validation checks, multi-method sizing, shadow mode | None — raw exchange access |
| **Journal** | Every trade, fill, decision recorded | CLI logs + audit log (`~/.tealstreet/audit.log`) |
| **Exchange support** | Binance, Bybit, OKX, Hyperliquid, Bitget, Gate.io, Phemex, BloFin, WOO X | Same CEX set (shares account configs) |
| **Agent-native** | Built for it — SIWE auth, agent wallets, LLM-ready journal summaries | Not agent-native, but CLI is scriptable |

The integration direction is: **Testudo decides, Tealstreet executes**. Testudo's risk engine validates every trade. Tealstreet's CLI provides direct exchange access without building a new adapter.

## Three integration vectors

### 1. CLI bridge (lowest effort, works today)

Testudo's agent loop (or any script) shells out to `tealstreet` for execution.

```
Testudo agent loop
  │
  ├─ observe (market data, journal, sheaf signals)
  ├─ think  (LLM or strategy logic)
  ├─ decide (symbol, side, size, stops)
  │
  └─ act ──► tealstreet exec --account main --symbol BTC_USDT buy ...
              │
              └─► exchange
```

**How it works:**

```bash
# Testudo decides to go long BTC, risk-validated size of 0.05 BTC at $50k with $49k stop
# It shells out:
tealstreet buy \
  --account bybit-main \
  --symbol BTC_USDT \
  0.05 50000 limit \
  --stop-loss 49000 \
  --take-profit 52000
```

The `tealstreet exec` subcommand is the cleanest bridge point — it accepts any trading command as arguments and is designed for scripting:

```bash
tealstreet exec --account main --symbol ETH_USDT buy 1.5 3200 limit
tealstreet exec --account main --symbol ETH_USDT close --all
tealstreet balance --account main
```

**What Testudo adds on top:**
- Risk validation BEFORE the `tealstreet` call (never bypass the risk engine)
- Shadow mode: Testudo simulates the trade, never shells out
- Journal recording: Testudo wraps the tealstreet execution and records the result
- Idempotency: Testudo's idempotency key prevents double-execution

**Implementation sketch (testudo-cli):**

```rust
// testudo-cli/src/execution/tealstreet.rs

pub struct TealstreetBridge {
    binary: PathBuf,  // ~/.local/bin/tealstreet
}

impl TealstreetBridge {
    pub fn execute(&self, decision: &TradeDecision) -> Result<ExecutionReport> {
        // 1. Risk validation already done by DecisionLoop upstream
        // 2. Build the tealstreet command
        let args = match decision.side {
            Side::Long => vec!["buy", "--account", &decision.account, ...],
            Side::Short => vec!["sell", "--account", &decision.account, ...],
        };
        // 3. Shell out, capture stdout/stderr
        let output = Command::new(&self.binary)
            .args(&args)
            .output()?;
        // 4. Parse execution report, record in journal
        // 5. Return structured result
    }
}
```

### 2. Cross-trade engine integration (for multi-account strategies)

Tealstreet v0.10.0 ships a headless cross-trade engine (`tealstreet cross-trade`). Configs live in `~/.tealstreet/cross-trade.json`. Testudo can:

- **Write cross-trade configs** programmatically from strategy logic
- **Read `cross-trade-running.json`** heartbeat to know if the engine is live
- **Read `audit.log`** for cross-trade event tracking

```bash
# Testudo writes a config
cat > ~/.tealstreet/cross-trade.json << 'EOF'
[{
  "id": "testudo-mean-reversion-001",
  "leader": {"account": "bybit-main", "symbol": "BTC_USDT"},
  "followers": [
    {"account": "okx-hedge", "symbol": "BTC_USDT", "mode": "Market", "scale": 0.5}
  ],
  "enabled": true,
  "updatedAt": "2026-07-07T12:00:00Z"
}]
EOF

# Testudo launches the engine
tealstreet cross-trade run

# Testudo monitors
tealstreet cross-trade status
# → {"running": true, "uptime": "2h14m", "activeConfigs": 1}
```

Cross-device sync works via the listener bridge — Testudo can write configs while the web app is open, and changes propagate both ways with last-write-wins semantics.

### 3. Account credential sharing

Testudo already stores exchange API keys (via `POST /api/v1/exchanges/accounts`). Tealstreet stores them at `~/.tealstreet/accounts.json`. These can be kept in sync:

```
Testudo (master) ──► ~/.tealstreet/accounts.json
  │                        │
  │  stores encrypted      │  tealstreet reads
  │  API keys in DB        │  for direct execution
  │                        │
  └── on account add ──────┘  export to tealstreet format
```

```bash
# Testudo exports its exchange credentials to tealstreet format
testudo exchange export --format tealstreet > /tmp/tealstreet-accounts.json
tealstreet account import /tmp/tealstreet-accounts.json
```

Or vice versa — if the user already has Tealstreet configured, import into Testudo:

```bash
tealstreet account export > /tmp/tealstreet-accounts.json
testudo exchange import --from tealstreet /tmp/tealstreet-accounts.json
```

## Architecture decision: shell-out vs. direct API

| Approach | Effort | Risk | When |
|---|---|---|---|
| **Shell out to `tealstreet` CLI** | Hours | Low — tealstreet handles exchange auth | Today. Works now. |
| **Tealstreet as Testudo exchange adapter** | Days | Medium — new adapter surface | When shell latency matters or you need streaming fill confirmations |
| **Direct exchange API (bypass Tealstreet)** | Weeks | Already done — Testudo has CEX adapters | When you want pure Testudo without Tealstreet dependency |

**Recommendation:** Start with the CLI bridge. It's one afternoon of work, zero new exchange adapter code, and tealstreet already handles auth, order construction, and exchange-specific quirks. Graduate to a native adapter only if shell-out latency becomes a bottleneck (unlikely for mid-frequency strategies).

## What the CLI bridge doesn't cover

- **Real-time fill confirmations.** Shell-out returns after the order is placed, not filled. For streaming execution reports, you'd need the Tealstreet WebSocket or Testudo's own exchange adapters.
- **Paper trading.** Tealstreet has no shadow mode. Use Testudo's shadow engine for paper testing, then switch to live tealstreet execution.
- **Chase execution mode.** Tealstreet's chase copier is web-only in v0.10.0. The CLI warns and skips chase configs.

## Quick start: a Testudo strategy that trades via Tealstreet

```bash
# 1. Ensure tealstreet is installed and configured
tealstreet --version          # v0.10.0+
tealstreet account add        # Add your exchange account

# 2. Test manual execution
tealstreet balance --account main
tealstreet exec --account main --symbol BTC_USDT buy 0.01 50000 limit

# 3. Wire into Testudo
#    In testudo-cli, add a TealstreetExecutionProvider that shells out.
#    The existing agent loop (observe → think → act) stays the same;
#    only the "act" step changes — from POST /api/v1/signals
#    to spawning `tealstreet exec ...`.

# 4. Shadow-first, as always
#    Testudo's risk engine validates in shadow mode first.
#    Only graduates to live tealstreet execution after thresholds are met.
```

## Dual-runtime warning

If you run Testudo's agent AND the Tealstreet web app simultaneously with the same exchange accounts, both can place orders. Tealstreet's cross-trade daemon prints a throttled warning when it detects this. Testudo should add its own guard: check `~/.tealstreet/cross-trade-running.json` before executing, and warn if the tealstreet daemon is live with enabled configs.

## Related docs

- [AGENT_TRADING.md](../AGENT_TRADING.md) — Testudo agent trading guide
- [agent-integration-blueprint.md](../agent-integration-blueprint.md) — Signal endpoint + WebSocket + journal memory
- [testudo-cli/README.md](../testudo-cli/README.md) — testudo CLI harness
- [Tealstreet CLI releases](https://github.com/Tealstreet/cli/releases)
