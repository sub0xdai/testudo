// @anchor infra:cli:strategies:template
// @tags infra

//! Strategy TOML template — deserialization target for strategy files.

use serde::Deserialize;
use std::collections::HashMap;

/// A complete strategy definition loaded from a `.toml` file.
#[derive(Debug, Clone, Deserialize)]
pub struct StrategyTemplate {
    pub meta: StrategyMeta,
    #[serde(default, rename = "loop")]
    pub loop_config: Option<LoopConfigSection>,
    pub prompt: StrategyPrompt,
    #[serde(default)]
    pub parameters: Option<HashMap<String, StrategyParam>>,
    #[serde(default)]
    pub constraints: Option<StrategyConstraints>,
    #[serde(default)]
    pub allowed_tools: Option<AllowedToolsSection>,
    /// Proof artifacts this strategy requires.
    #[serde(default)]
    pub required_proofs: Vec<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct StrategyMeta {
    pub name: String,
    pub version: String,
    pub description: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct LoopConfigSection {
    pub interval_secs: Option<u64>,
    pub shadow_only: Option<bool>,
    pub max_signals_per_hour: Option<u32>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct StrategyPrompt {
    pub system: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct StrategyParam {
    #[serde(rename = "type")]
    pub param_type: String,
    pub default: serde_json::Value,
    pub description: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct StrategyConstraints {
    pub max_leverage: Option<u8>,
    pub max_position_notional: Option<f64>,
    pub allowed_symbols: Option<Vec<String>>,
    pub shadow_only: Option<bool>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct AllowedToolsSection {
    pub tools: Vec<String>,
}
