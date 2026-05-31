// @anchor infra:cli:model:agent
// @tags infra

//! Agent state machine — phases, mode, loop configuration.

use crate::llm::types::LlmMessage;

/// The agent's operational state.
#[derive(Debug, Clone)]
pub struct AgentState {
    pub phase: AgentPhase,
    pub mode: AgentMode,
    pub messages: Vec<LlmMessage>,
    pub signal_count_this_hour: u32,
}

impl AgentState {
    pub fn new(mode: AgentMode) -> Self {
        Self {
            phase: AgentPhase::Observing,
            mode,
            messages: Vec::new(),
            signal_count_this_hour: 0,
        }
    }
}

/// Observable phase of the agent loop.
#[derive(Debug, Clone, PartialEq)]
pub enum AgentPhase {
    Observing,
    Thinking,
    Acting,
    Idle,
}

/// Whether the agent runs in shadow (paper) or live mode.
#[derive(Debug, Clone, PartialEq)]
pub enum AgentMode {
    Shadow,
    Live,
}

impl AgentMode {
    /// Returns true if LIVE execution is blocked.
    pub fn is_shadow_only(&self) -> bool {
        matches!(self, AgentMode::Shadow)
    }
}
