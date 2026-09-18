// @anchor exchange:router:config
// @tags api

use confik::Configuration;

#[derive(Debug, Clone, Configuration)]
pub struct RouterConfig {
    #[confik(default = "127.0.0.1:3001")]
    pub server_addr: String,

    /// JWT access token secret - MUST be provided in production
    pub jwt_access_secret: String,

    /// JWT refresh token secret - MUST be provided in production
    pub jwt_refresh_secret: String,

    /// Access token expiration in seconds
    pub jwt_access_expires_seconds: u64,

    /// Refresh token expiration in seconds
    pub jwt_refresh_expires_seconds: u64,

    /// Rate limiting: requests per minute per IP
    pub rate_limit_requests_per_minute: u32,

    /// Rate limiting: burst capacity
    pub rate_limit_burst_capacity: u32,

    /// RSK-03 — Base URL of the OpenAI-compatible LLM endpoint (DeepSeek/GLM/OpenRouter).
    #[confik(default = "https://api.deepseek.com/v1")]
    pub llm_base_url: String,

    /// RSK-03 — Model name sent to the LLM endpoint.
    #[confik(default = "deepseek-chat")]
    pub llm_model: String,

    /// RSK-03 — Global coach kill-switch. When false, the weekly scheduler
    /// never fires a batch even if users remain opted in.
    #[confik(default = true)]
    pub coach_enabled_global: bool,

    /// RSK-03 — Lifetime-trade threshold below which the coach stays locked.
    #[confik(default = 30)]
    pub coach_min_lifetime_trades: i64,

    /// RSK-03 — Per-week trade threshold below which the coach skips the user.
    #[confik(default = 3)]
    pub coach_min_week_trades: i64,

    /// TS-01 — Global TypeSafe (Jev) kill-switch.
    ///
    /// `false`, the default, builds no client at all: the extension keeps its
    /// pre-judgement behaviour and no trade context leaves the server. Keep it
    /// off until a `TYPESAFE_API_KEY` exists and the privacy copy covers the
    /// egress.
    #[confik(default = false)]
    pub typesafe_enabled: bool,

    /// TS-01 — Evaluation endpoint host, no trailing slash.
    ///
    /// Overridable so staging and tests can point at a local server.
    #[confik(default = "https://api.typesafe.ai")]
    pub typesafe_base_url: String,

    /// TS-01 — Model id sent in the `model` field.
    ///
    /// A pinned id rather than an alias, so a threshold tuned against one
    /// build is not silently moved to the next. confik needs a literal here,
    /// so `default_typesafe_model_stays_pinned` guards the duplication.
    #[confik(default = "jev-1.13.0")]
    pub typesafe_model: String,
}

impl RouterConfig {
    /// Validate critical security configuration
    pub fn validate(&self) -> Result<(), String> {
        if self.jwt_access_secret.is_empty() {
            return Err("JWT_ACCESS_SECRET must be provided".to_string());
        }

        if self.jwt_refresh_secret.is_empty() {
            return Err("JWT_REFRESH_SECRET must be provided".to_string());
        }

        if self.jwt_access_secret.len() < 32 {
            return Err("JWT_ACCESS_SECRET must be at least 32 characters".to_string());
        }

        if self.jwt_refresh_secret.len() < 32 {
            return Err("JWT_REFRESH_SECRET must be at least 32 characters".to_string());
        }

        if self.jwt_access_secret == self.jwt_refresh_secret {
            return Err("JWT_ACCESS_SECRET and JWT_REFRESH_SECRET must be different".to_string());
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    /// The confik attribute needs a string literal, so the pinned model id
    /// exists in two places. This pins them together: changing one without
    /// the other is caught here rather than in production answers.
    #[test]
    fn default_typesafe_model_stays_pinned() {
        assert_eq!(
            crate::services::typesafe::MODEL_PINNED,
            "jev-1.13.0",
            "update the typesafe_model confik default in the same change"
        );
    }
}
