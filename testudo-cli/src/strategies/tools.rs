// @anchor infra:cli:strategies:tools
// @tags infra

//! Tool constrainer — modifies LLM tool schemas with proof-backed constraints.

use crate::strategies::constraints::ConstraintSet;
use crate::tools::types::ToolDef;

pub struct ToolConstrainer;

impl ToolConstrainer {
    /// Modify the submit_signal tool definition to reflect current constraints.
    pub fn constrain_signal_tool(tool: &mut ToolDef, constraints: &ConstraintSet) {
        // Clamp leverage max
        if let Some(props) = tool.parameters.get_mut("properties") {
            if let Some(leverage) = props.get_mut("leverage") {
                if let Some(max_field) = leverage.get_mut("maximum") {
                    *max_field = serde_json::json!(constraints.max_leverage as u64);
                }
            }
            // Enforce stop_loss in required if constraint says so
            if constraints.stop_loss_required {
                if let Some(required) = tool.parameters.get_mut("required") {
                    if let Some(arr) = required.as_array_mut() {
                        let has_stop_loss = arr.iter().any(|v| v.as_str() == Some("stop_loss"));
                        if !has_stop_loss {
                            arr.push(serde_json::json!("stop_loss"));
                        }
                    }
                }
            }
        }

        // Append proof-backed constraints to description
        tool.description = format!(
            "{}\n\nProof-backed constraints (DO NOT VIOLATE):\n\
             - Max leverage: {}×\n\
             - Max account risk per trade: {:.1}%\n\
             - Max drawdown: {:.1}%\n\
             - Stop loss required: {}",
            tool.description,
            constraints.max_leverage as u64,
            constraints.max_account_risk_pct,
            constraints.max_drawdown_pct,
            if constraints.stop_loss_required { "YES" } else { "no" },
        );
    }
}
