// @anchor infra:cli:api:onboarding
// @tags api

//! GET /api/v1/onboarding/status — single-call agent readiness check.

use crate::api::client::ApiClient;
use crate::api::types::{ApiError, OnboardingStatus};

impl ApiClient {
    /// Get the user's onboarding status.
    /// Returns readiness state, next step guidance, and missing items.
    pub async fn get_onboarding_status(&self) -> Result<OnboardingStatus, ApiError> {
        self.get_json("/api/v1/onboarding/status").await
    }
}
