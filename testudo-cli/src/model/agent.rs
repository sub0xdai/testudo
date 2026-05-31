// @anchor infra:cli:model:agent
// @tags infra

//! Agent state machine — phases, mode, loop configuration,
//! idempotency tracking, and signal rate limiting.

use crate::llm::types::LlmMessage;

/// The agent's operational state.
#[derive(Debug, Clone)]
pub struct AgentState {
    pub phase: AgentPhase,
    pub mode: AgentMode,
    pub messages: Vec<LlmMessage>,
    pub idempotency: idempotency::IdempotencyTracker,
    pub rate_limiter: rate_limit::SignalRateLimiter,
}

impl AgentState {
    pub fn new(mode: AgentMode) -> Self {
        Self {
            phase: AgentPhase::Observing,
            mode,
            messages: Vec::new(),
            idempotency: idempotency::IdempotencyTracker::default(),
            rate_limiter: rate_limit::SignalRateLimiter::new(5),
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

// ── Idempotency tracking ────────────────────────────────────────────

pub mod idempotency {
    use uuid::Uuid;

    /// Tracks idempotency keys for signal submissions.
    /// Each new signal gets a fresh UUIDv4. Retries reuse the same key.
    #[derive(Debug, Clone)]
    pub struct IdempotencyTracker {
        current: Uuid,
        max_retries: u32,
        attempt: u32,
    }

    impl IdempotencyTracker {
        pub fn new(max_retries: u32) -> Self {
            Self {
                current: Uuid::new_v4(),
                max_retries,
                attempt: 0,
            }
        }

        /// Generate a new key for the next signal.
        pub fn next_key(&mut self) -> Uuid {
            self.current = Uuid::new_v4();
            self.attempt = 0;
            self.current
        }

        /// Get the current key (for retries).
        pub fn current_key(&self) -> Uuid {
            self.current
        }

        /// Record a retry attempt. Returns false if max retries exceeded.
        pub fn record_retry(&mut self) -> bool {
            self.attempt += 1;
            self.attempt <= self.max_retries
        }

        pub fn max_retries(&self) -> u32 {
            self.max_retries
        }

        pub fn attempt_count(&self) -> u32 {
            self.attempt
        }
    }

    impl Default for IdempotencyTracker {
        fn default() -> Self {
            Self::new(3)
        }
    }
}

// ── Rate limiting ───────────────────────────────────────────────────

pub mod rate_limit {
    /// Limits signal submissions per time window.
    #[derive(Debug, Clone)]
    pub struct SignalRateLimiter {
        max_signals: u32,
        count: u32,
    }

    impl SignalRateLimiter {
        pub fn new(max_signals: u32) -> Self {
            Self {
                max_signals,
                count: 0,
            }
        }

        /// Attempt to submit a signal. Returns false if rate limit exceeded.
        pub fn try_signal(&mut self) -> bool {
            if self.count >= self.max_signals {
                return false;
            }
            self.count += 1;
            true
        }

        /// Reset the counter (e.g., at the start of a new window).
        pub fn reset(&mut self) {
            self.count = 0;
        }

        pub fn remaining(&self) -> u32 {
            self.max_signals.saturating_sub(self.count)
        }
    }
}
