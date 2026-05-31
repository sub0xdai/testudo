// @anchor infra:cli:api:journal
// @tags api

//! Journal endpoints — GET /journal/agent/summary, /insights, POST /compare.

use crate::api::client::ApiClient;
use crate::api::types::{AgentInsight, AgentSummary, ApiError, CompareRequest, CompareResult};

impl ApiClient {
    /// Get the agent journal summary.
    /// When `format` is "llm", the server returns markdown — use `get_summary_text()`.
    pub async fn get_summary(
        &self,
        timeframe: &str,
        format: &str,
    ) -> Result<AgentSummary, ApiError> {
        let path = format!(
            "/api/v1/journal/agent/summary?timeframe={}&format={}",
            timeframe, format
        );
        self.get_json(&path).await
    }

    /// Get the agent journal summary as raw text (markdown when format=llm).
    pub async fn get_summary_text(
        &self,
        timeframe: &str,
        format: &str,
    ) -> Result<String, ApiError> {
        let path = format!(
            "/api/v1/journal/agent/summary?timeframe={}&format={}",
            timeframe, format
        );
        self.get_text(&path).await
    }

    /// Get detected patterns from the coach pipeline.
    pub async fn get_insights(&self) -> Result<Vec<AgentInsight>, ApiError> {
        self.get_json("/api/v1/journal/agent/insights").await
    }

    /// Compare two time periods.
    pub async fn post_compare(&self, req: &CompareRequest) -> Result<CompareResult, ApiError> {
        self.post_json("/api/v1/journal/agent/compare", req).await
    }
}
