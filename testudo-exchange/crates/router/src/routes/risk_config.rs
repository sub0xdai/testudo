//! Risk Configuration Routes
//!
//! GET /api/v1/risk-config - Get user's risk configuration
//! PUT /api/v1/risk-config - Update user's risk configuration

// @anchor exchange:router:risk_config
// @tags api

use actix_web::{web, HttpResponse, Result};
use common_utils::{
    risk::{PgRiskConfigStorage, RiskConfig},
    services::pg_cache::PgCacheService,
};
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};

use crate::{middleware::AuthenticatedUser, types::app::AppState};

/// Response for GET /risk-config
#[derive(Debug, Serialize)]
pub struct RiskConfigResponse {
    pub account_risk_percent: Decimal,
    pub max_risk_amount: Option<Decimal>,
    pub max_position_size: Option<Decimal>,
    pub max_leverage: u8,
    pub daily_max_drawdown_percent: Option<Decimal>,
    pub max_open_positions: Option<u32>,
    pub require_stop_loss: bool,
    pub default_stop_atr_multiplier: Option<Decimal>,
    pub min_risk_reward_ratio: Option<Decimal>,
}

impl From<RiskConfig> for RiskConfigResponse {
    fn from(config: RiskConfig) -> Self {
        Self {
            account_risk_percent: config.account_risk_percent,
            max_risk_amount: config.max_risk_amount,
            max_position_size: config.max_position_size,
            max_leverage: config.max_leverage,
            daily_max_drawdown_percent: config.daily_max_drawdown_percent,
            max_open_positions: config.max_open_positions,
            require_stop_loss: config.require_stop_loss,
            default_stop_atr_multiplier: config.default_stop_atr_multiplier,
            min_risk_reward_ratio: config.min_risk_reward_ratio,
        }
    }
}

/// Request for PUT /risk-config
#[derive(Debug, Deserialize)]
pub struct UpdateRiskConfigRequest {
    pub account_risk_percent: Option<Decimal>,
    pub max_risk_amount: Option<Decimal>,
    pub max_position_size: Option<Decimal>,
    pub max_leverage: Option<u8>,
    pub daily_max_drawdown_percent: Option<Decimal>,
    pub max_open_positions: Option<u32>,
    pub require_stop_loss: Option<bool>,
    pub default_stop_atr_multiplier: Option<Decimal>,
    pub min_risk_reward_ratio: Option<Decimal>,
}

/// Error response
#[derive(Debug, Serialize)]
pub struct ErrorResponse {
    pub code: String,
    pub message: String,
}

impl ErrorResponse {
    pub fn new(code: &str, message: &str) -> Self {
        Self {
            code: code.to_string(),
            message: message.to_string(),
        }
    }
}

/// GET /api/v1/risk-config
/// Returns the user's risk configuration (or default if none saved)
pub async fn get_risk_config(
    app_state: web::Data<AppState>,
    user: AuthenticatedUser,
) -> Result<HttpResponse> {
    let cache = PgCacheService::new(app_state.pool.clone());
    let storage = PgRiskConfigStorage::new(cache);

    match storage.load_or_default(user.user_id).await {
        Ok(config) => Ok(HttpResponse::Ok().json(RiskConfigResponse::from(config))),
        Err(e) => {
            tracing::error!(
                "Failed to load risk config for user {}: {}",
                user.user_id,
                e
            );
            Ok(HttpResponse::InternalServerError().json(ErrorResponse::new(
                "storage_error",
                "Failed to load risk configuration",
            )))
        }
    }
}

/// PUT /api/v1/risk-config
/// Updates the user's risk configuration
pub async fn update_risk_config(
    app_state: web::Data<AppState>,
    user: AuthenticatedUser,
    req: web::Json<UpdateRiskConfigRequest>,
) -> Result<HttpResponse> {
    let cache = PgCacheService::new(app_state.pool.clone());
    let storage = PgRiskConfigStorage::new(cache);

    // Load existing config or start with default
    let mut config = match storage.load_or_default(user.user_id).await {
        Ok(c) => c,
        Err(e) => {
            tracing::error!(
                "Failed to load existing config for user {}: {}",
                user.user_id,
                e
            );
            return Ok(HttpResponse::InternalServerError().json(ErrorResponse::new(
                "storage_error",
                "Failed to load existing configuration",
            )));
        }
    };

    // Apply updates from request
    if let Some(v) = req.account_risk_percent {
        config.account_risk_percent = v;
    }
    if let Some(v) = req.max_risk_amount {
        config.max_risk_amount = Some(v);
    }
    if let Some(v) = req.max_position_size {
        config.max_position_size = Some(v);
    }
    if let Some(v) = req.max_leverage {
        config.max_leverage = v;
    }
    if let Some(v) = req.daily_max_drawdown_percent {
        config.daily_max_drawdown_percent = Some(v);
    }
    if let Some(v) = req.max_open_positions {
        config.max_open_positions = Some(v);
    }
    if let Some(v) = req.require_stop_loss {
        config.require_stop_loss = v;
    }
    if let Some(v) = req.default_stop_atr_multiplier {
        config.default_stop_atr_multiplier = Some(v);
    }
    if let Some(v) = req.min_risk_reward_ratio {
        config.min_risk_reward_ratio = Some(v);
    }

    // Validate the updated config
    if let Err(e) = config.validate() {
        return Ok(
            HttpResponse::BadRequest().json(ErrorResponse::new("validation_error", &e.to_string()))
        );
    }

    // Save the updated config
    match storage.save(&config).await {
        Ok(()) => Ok(HttpResponse::Ok().json(RiskConfigResponse::from(config))),
        Err(e) => {
            tracing::error!(
                "Failed to save risk config for user {}: {}",
                user.user_id,
                e
            );
            Ok(HttpResponse::InternalServerError().json(ErrorResponse::new(
                "storage_error",
                "Failed to save risk configuration",
            )))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rust_decimal_macros::dec;

    #[test]
    fn test_risk_config_response_from_config() {
        let config = RiskConfig::default();
        let response = RiskConfigResponse::from(config);

        assert_eq!(response.account_risk_percent, dec!(2));
        assert!(response.require_stop_loss);
    }

    #[test]
    fn test_update_request_partial() {
        let json = r#"{"account_risk_percent": "3.5"}"#;
        let req: UpdateRiskConfigRequest = serde_json::from_str(json).unwrap();

        assert_eq!(req.account_risk_percent, Some(dec!(3.5)));
        assert!(req.max_risk_amount.is_none());
        assert!(req.max_leverage.is_none());
    }
}
