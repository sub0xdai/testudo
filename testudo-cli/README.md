# testudo — Terminal-First Trading Harness

`testudo` is a terminal-native CLI for the [Testudo](https://testudo.vip) cryptocurrency exchange platform. It provides autonomous LLM-powered trading, a live TUI dashboard, strategy management backed by Lean 4 mathematical proofs, and daemon mode for 24/7 operation.

## Quick Start

**Linux & macOS:**
```bash
curl -fsSL https://api.testudo.vip/install.sh | bash
```

**Windows (PowerShell):**
```powershell
powershell -c "irm https://api.testudo.vip/install.ps1 | iex"
```

**Windows (Git Bash / WSL):**
```bash
curl -fsSL https://api.testudo.vip/install.sh | bash
```

After install:
```bash
testudo init                    # Complete setup wizard (6 steps)
testudo strategy list           # Browse strategies
testudo agent start --strategy mean-reversion   # Start trading
testudo journal                 # Check results
testudo dashboard               # Open live TUI
```

## Commands

### Trading

| Command | Description |
|---------|-------------|
| `testudo agent start` | Start the autonomous trading loop |
| `testudo agent start --strategy <name>` | Start with a specific strategy |
| `testudo agent start --daemon` | Run as a background daemon |
| `testudo agent stop` | Stop the agent (not yet implemented) |
| `testudo attach` | Connect to a running daemon |
| `testudo journal` | Print 30-day trading summary |
| `testudo listen` | Stream WebSocket events as JSON Lines |

### Dashboard

| Command | Description |
|---------|-------------|
| `testudo dashboard` | Open the live TUI dashboard |

The dashboard displays 6 panes: Positions, Agent Reasoning, P&L Chart (sparkline), Signal Log, Journal Summary, and Risk (with drawdown gauge). Press `q` or `Esc` to quit.

### Strategies

| Command | Description |
|---------|-------------|
| `testudo strategy list` | List all available strategies |
| `testudo strategy show <name>` | View strategy details and prompt |
| `testudo strategy add <name> --from <path>` | Install a custom strategy |
| `testudo strategy remove <name>` | Remove a custom strategy |
| `testudo strategy validate <name>` | Validate against proof artifacts |

#### Built-in Strategies

- **mean-reversion** — Bollinger Bands (20,2σ) + RSI(14), max 3× leverage
- **momentum-breakout** — Volume-confirmed channel breakout, max 5× leverage
- **funding-arb** — Funding rate differential arbitrage, max 2× leverage

Custom strategies are loaded from `~/.config/testudo/strategies/` and override builtins with the same name. Strategy TOML files have `[meta]`, `[prompt]`, `[constraints]`, and `[allowed_tools]` sections.

### Setup

| Command | Description |
|---------|-------------|
| `testudo init` | Guided 5-step onboarding wizard |

## Architecture

```
testudo
├── CLI (clap)         — 7 subcommands with typed arguments
├── TUI (ratatui)      — 6-pane dashboard with TEA event loop
├── API Client         — typed REST client for 7 backend endpoints
├── WebSocket Client   — real-time events with exponential backoff
├── LLM Provider       — Anthropic Messages API (tool_use blocks)
├── Agent Loop         — observe → think → act → journal → sleep
├── Strategy Registry  — TOML-based strategies with user overrides
├── Risk Pre-check     — client-side leverage/position/symbol validation
├── Proof Bridge       — STRAT-01 Lean 4 proof artifact loader
└── Daemon             — Unix socket JSON-RPC, file logging
```

## Daemon Mode

```bash
# Start daemon
testudo agent start --daemon --strategy momentum-breakout

# Check status
testudo attach

# Control via socket
echo '{"method":"status"}' | nc -U ~/.config/testudo/testudo.sock
echo '{"method":"stop"}'   | nc -U ~/.config/testudo/testudo.sock
```

Logs are written to `~/.config/testudo/logs/testudo.log` with daily rotation (JSON format).

## Proof-Backed Strategies

Strategies can reference Lean 4 proof artifacts from the `testudo-proofs` crate. When a strategy specifies `required_proofs = ["kelly", "ou-reversion"]`, the harness:

1. Loads the proof's `.toml` artifact (constraints + prompt)
2. Merges constraints (most conservative bound always wins)
3. Intersects with your risk config (you can only tighten, never loosen)
4. Modifies the LLM's tool schemas to enforce proven bounds
5. Validates the strategy doesn't violate any proof constraint

```bash
# See which proofs back your strategy
testudo strategy validate mean-reversion
```

## Building

```bash
cd testudo-cli
cargo build --release
```

### Dependencies

- Rust 1.80+
- `ratatui` + `crossterm` — TUI
- `tokio` — async runtime
- `reqwest` — HTTP client
- `clap` — CLI parsing
- `common_utils` — shared types with Testudo backend

## Testing

```bash
cargo test                    # 165 tests
cargo clippy --all-targets    # zero warnings
```

## Environment Variables

| Variable | Default | Description |
|----------|---------|-------------|
| `RUST_LOG` | `info` | Tracing verbosity (error, warn, info, debug, trace) |

## License

MIT — see the monorepo root for details.
