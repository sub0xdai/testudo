# testudo-proofs

Lean 4 verification layer for Testudo autonomous trading strategies.

Machine-checked proofs of the theorems that underpin every strategy primitive.
The agent references proven formulas instead of hallucinating its own math.

## Build

```bash
cd testudo-proofs
lake build
```

Requires Lean 4 (v4.30.0-rc2) and Mathlib4.

## Verify Artifacts

Each proof ships with a strategy artifact (`.toml`) that bridges theorem →
constraint → LLM prompt. Run the cross-reference checker:

```bash
python3 verify-artifacts.py
```

This verifies every `.lean` file has a matching `.toml` artifact, theorem names
match, and constraint values are within proven bounds.

## Theorems + Artifacts

| Module | Theorem | Artifact | Constraints | Used for |
|---|---|---|---|---|
| WassersteinMetric | W₁ is a metric | `WassersteinMetric.toml` | `min_samples=50`, regime thresholds | Regime detection |
| KellyOptimal | f* maximizes growth | `KellyOptimal.toml` | `max_leverage=5`, `max_account_risk_pct=2.0` | Position sizing |
| OUMreversion | Deviation bound | `OUMreversion.toml` | `reversion_half_life_candles=6` | Mean reversion timing |
| MomentumAutocorr | Cov → return direction | `MomentumAutocorr.toml` | `min_autocorr_threshold=0.10` | Trend following |
| FundingArb | No-arbitrage bound | `FundingArb.toml` | `min_funding_rate_bps=1.5` | Funding rate arbitrage |
| DeltaNeutral | Hedge → Δ=0 | `DeltaNeutral.toml` | `max_net_delta=0.01` | Portfolio hedging |
| GamblersRuin | Drawdown bound | `GamblersRuin.toml` | `max_drawdown_pct=20` | When to halt trading |

Full strategy descriptions: `../strat-lean-proofs.md`

## Consumption Path

Artifacts are loaded by the trading harness bridge (**AGENT-09-strategy-system**).
The bridge merges constraints (most conservative wins), intersects with user
risk config, and bakes proof-derived bounds into LLM tool definitions.

See `.specify/specs/AGENT-09-strategy-system/spec.md` for the bridge design.

## License

AGPL-3.0.
