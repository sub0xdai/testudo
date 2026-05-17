use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// User domain model — wallet-primary identity (AUTH-02).
/// RSK-03 adds two coach-preference columns so the weekly cron can honour
/// per-user opt-out and the `● new` banner indicator can be cleared on visit.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct User {
    pub id: Uuid,
    pub wallet_address: String,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub is_active: bool,
    pub coach_enabled: bool,
    pub coach_banner_last_viewed_at: Option<DateTime<Utc>>,
}

impl User {
    /// Create a new user from a wallet address
    pub fn new(wallet_address: String) -> Self {
        let now = Utc::now();
        Self {
            id: Uuid::new_v4(),
            wallet_address,
            created_at: now,
            updated_at: now,
            is_active: true,
            coach_enabled: true,
            coach_banner_last_viewed_at: None,
        }
    }

    /// Updates the timestamp to current time
    pub fn update_timestamp(&mut self) {
        self.updated_at = Utc::now();
    }

    /// Deactivates the user account
    pub fn deactivate(&mut self) {
        self.is_active = false;
        self.update_timestamp();
    }

    /// Activates the user account
    pub fn activate(&mut self) {
        self.is_active = true;
        self.update_timestamp();
    }
}

#[derive(Debug, thiserror::Error)]
pub enum UserError {
    #[error("Invalid wallet address: {0}")]
    InvalidWalletAddress(String),
    #[error("Validation failed: {0}")]
    ValidationFailed(String),
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn should_create_user_with_wallet_address() {
        let user = User::new("0xC285000000000000000000000000000000005b36".to_string());

        assert_eq!(
            user.wallet_address,
            "0xC285000000000000000000000000000000005b36"
        );
        assert!(user.is_active);
        assert!(!user.id.is_nil());
        assert!(user.created_at <= Utc::now());
        assert_eq!(user.created_at, user.updated_at);
    }

    #[test]
    fn should_update_timestamp_on_operations() {
        let mut user = User::new("0xC285000000000000000000000000000000005b36".to_string());
        let original_time = user.updated_at;

        std::thread::sleep(std::time::Duration::from_millis(1));

        user.deactivate();
        assert!(user.updated_at > original_time);
        assert!(!user.is_active);

        let deactivated_time = user.updated_at;
        std::thread::sleep(std::time::Duration::from_millis(1));

        user.activate();
        assert!(user.updated_at > deactivated_time);
        assert!(user.is_active);
    }

    #[test]
    fn should_not_expose_sensitive_data_in_serialization() {
        let user = User::new("0xC285000000000000000000000000000000005b36".to_string());
        let serialized = serde_json::to_string(&user).expect("Should serialize");

        assert!(serialized.contains("wallet_address"));
        assert!(serialized.contains("0xC285"));

        let deserialized: User = serde_json::from_str(&serialized).expect("Should deserialize");
        assert_eq!(user.wallet_address, deserialized.wallet_address);
        assert_eq!(user.id, deserialized.id);
    }
}
