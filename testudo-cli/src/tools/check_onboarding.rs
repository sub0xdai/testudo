// @anchor infra:cli:tools:check_onboarding
// @tags api

//! check_onboarding tool — check onboarding/readiness status.

use crate::tools::types::ToolDef;

pub fn tool_def() -> ToolDef {
    ToolDef {
        name: "check_onboarding".into(),
        description:
            "Check your onboarding status. Returns whether you're ready to trade, \
             what steps are missing (exchange connection, agent wallet, risk config), \
             and available exchanges. Call this first to verify readiness.".into(),
        parameters: serde_json::json!({
            "type": "object",
            "properties": {},
            "required": []
        }),
    }
}
