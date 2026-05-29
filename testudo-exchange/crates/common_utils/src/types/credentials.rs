//! Exchange Credentials Types
//!
//! This module provides credential types for exchange adapter authentication.

// @anchor exchange:common_utils:credentials
// @tags infra

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// Exchange account credentials for adapter initialization
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExchangeCredentials {
    pub id: Uuid,
    pub user_id: Uuid,
    pub exchange_name: String,
    pub api_key: String,
    pub api_secret: String,
    pub permissions: serde_json::Value,
    pub is_active: bool,
    pub created_at: DateTime<Utc>,
    pub last_used_at: Option<DateTime<Utc>>,
}
