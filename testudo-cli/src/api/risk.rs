// @anchor infra:cli:api:risk
// @tags api

//! GET/PUT /api/v1/risk-config — risk configuration endpoints.

use crate::api::client::ApiClient;
use crate::api::types::{ApiError, RiskConfigData};

impl ApiClient {
    /// Get the current risk configuration.
    pub async fn get_risk_config(&self) -> Result<RiskConfigData, ApiError> {
        self.get_json("/api/v1/risk-config").await
    }

    /// Update the risk configuration.
    pub async fn update_risk_config(
        &self,
        update: &RiskConfigData,
    ) -> Result<RiskConfigData, ApiError> {
        self.post_json("/api/v1/risk-config", update).await
    }
}
