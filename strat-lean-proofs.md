# Architecture Specification: Testudo Autonomous Execution System

## 1. System Components
* **Hermes (Meta-Strategist)**: Large Language Model. Evaluates deterministic market states, queries historical action journals, and routes capital to verified quantitative sub-strategies.
* **Testudo (Execution Engine)**: Performant Rust runtime. Ingests websocket streams, computes optimal transport metrics, and executes deterministic primitives on target exchanges (Hyperliquid).
* **Lean 4 (Verification Layer)**: Static environment housing formal proofs for the $p$-Wasserstein metric and Kantorovich duality. Eliminates LLM mathematical hallucination.

## 2. State-Context Pipeline
1.  **Verification (Offline)**: Hermes synthesizes strategy module $\rightarrow$ Evaluates against `optimal_transport.lean` $\rightarrow$ Outputs formally verified logic.
2.  **Observation (Online)**: Testudo ingests OHLCV stream $\rightarrow$ Computes 1-Wasserstein distance against historical $k$-means centroids $\rightarrow$ Writes compressed geometric state vector to DB.
3.  **Evaluation (Online)**: Hermes queries DB state (Regime ID, Distance) and Journal (last $N$ operational hypotheses).
4.  **Command (Online)**: Hermes synthesizes state and memory, outputting strict JSON directive.
5.  **Execution (Online)**: Testudo parses `AgentDirective`, enforces regime-specific leverage limits, and executes.

## 3. Core Directives Schema
```rust
#[derive(Deserialize, Serialize, Debug)]
struct AgentDirective {
    thesis_summary: String,
    strategy_module: StrategyType,
    exchange: ExchangeConfig,
    max_leverage: u8,
    margin_type: MarginMode,
    invalidation_criteria: String,
}

#[derive(Deserialize, Serialize, Debug)]
#[serde(rename_all = "snake_case")]
enum StrategyType {
    MeanReversion,
    MomentumBreakout,
    FundingArbitrage,
    DeltaNeutralHedge,
    HaltExecution,
}
