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

## Theorems

| Module | Theorem | Used for |
|---|---|---|
| WassersteinMetric | W1 is a metric on R | Regime detection |
| KellyOptimal | f* maximizes geometric growth | Position sizing |
| OUMreversion | Deviation bound after n half-lives | Mean reversion timing |
| MomentumAutocorr | Covariance sign predicts return direction | Trend following |
| FundingArb | Profit iff spread exceeds friction | Funding rate arbitrage |
| DeltaNeutral | Single hedge achieves net-zero delta | Portfolio hedging |
| GamblersRuin | Ruin probability and drawdown bound | When to halt trading |

Full strategy descriptions in `../strat-lean-proofs.md`.

## License

AGPL-3.0.
