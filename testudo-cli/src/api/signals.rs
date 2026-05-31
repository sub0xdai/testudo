// @anchor infra:cli:api:signals
// @tags api

//! POST /api/v1/signals — submit a trade signal.

use crate::api::client::ApiClient;
use crate::api::types::{ApiError, SignalInput, SignalResult};

impl ApiClient {
    /// Submit a trade signal to the backend.
    /// The backend runs it through the DecisionLoop risk engine and returns a
    /// SignalResult with execution details or rejection reason.
    pub async fn submit_signal(&self, input: &SignalInput) -> Result<SignalResult, ApiError> {
        self.post_json("/api/v1/signals", input).await
    }
}
