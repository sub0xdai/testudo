/// Integration example showing how User and ExchangeAccount models work together
///
/// # Example Integration
///
/// ```rust
/// use common_utils::models::{User, ExchangeAccount, StandardExchangeAccountFactory};
/// use serde_json::json;
///
/// // Create a user with wallet address (AUTH-02: wallet-primary identity)
/// let user = User::new("0xC285000000000000000000000000000000005b36".to_string());
///
/// // Mock encrypted API credentials (in real usage, these would be encrypted)
/// let mock_encrypted_key = b"encrypted_binance_api_key".to_vec();
/// let mock_encrypted_secret = b"encrypted_binance_secret".to_vec();
///
/// // Define permissions for the exchange account
/// let permissions = json!({
///     "spot_trading": true,
///     "futures_trading": false,
///     "withdrawals": false
/// });
///
/// // Create an exchange account linked to the user
/// let exchange_factory = StandardExchangeAccountFactory::default();
/// let exchange_account = exchange_factory.create_exchange_account(
///     user.id,
///     "binance",
///     mock_encrypted_key,
///     mock_encrypted_secret,
///     permissions,
/// ).expect("Failed to create exchange account");
///
/// assert!(exchange_account.has_permission("spot_trading"));
/// ```
#[cfg(test)]
mod integration_tests {
    use super::super::{StandardExchangeAccountFactory, User};
    use serde_json::json;
    use uuid::Uuid;

    #[test]
    fn should_integrate_user_and_exchange_account_models() {
        let user = User::new("0xC285000000000000000000000000000000005b36".to_string());

        let mock_encrypted_key = b"encrypted_binance_api_key".to_vec();
        let mock_encrypted_secret = b"encrypted_binance_secret".to_vec();

        let permissions = json!({
            "spot_trading": true,
            "futures_trading": false,
            "withdrawals": false
        });

        let exchange_factory = StandardExchangeAccountFactory::default();
        let exchange_account = exchange_factory
            .create_exchange_account(
                user.id,
                "binance",
                mock_encrypted_key,
                mock_encrypted_secret,
                permissions,
            )
            .expect("Failed to create exchange account");

        assert_eq!(exchange_account.user_id, user.id);
        assert_eq!(exchange_account.exchange_name, "binance");
        assert!(exchange_account.has_permission("spot_trading"));
        assert!(!exchange_account.has_permission("futures_trading"));

        let serialized = serde_json::to_string(&exchange_account).expect("Should serialize");
        assert!(!serialized.contains("encrypted_binance_api_key"));
        assert!(!serialized.contains("encrypted_binance_secret"));
    }

    #[test]
    fn should_support_multiple_exchanges_per_user() {
        let user = User::new("0xC285000000000000000000000000000000005b36".to_string());

        let exchange_factory = StandardExchangeAccountFactory::default();

        let exchanges = vec!["binance", "coinbase", "kraken"];
        let mut accounts = Vec::new();

        for exchange in exchanges {
            let account = exchange_factory
                .create_exchange_account(
                    user.id,
                    exchange,
                    b"encrypted_key".to_vec(),
                    b"encrypted_secret".to_vec(),
                    json!({"spot_trading": true}),
                )
                .expect("Should create account");

            accounts.push(account);
        }

        for account in &accounts {
            assert_eq!(account.user_id, user.id);
            assert!(account.has_permission("spot_trading"));
        }

        let exchange_names: Vec<&str> = accounts.iter().map(|a| a.exchange_name.as_str()).collect();
        assert!(exchange_names.contains(&"binance"));
        assert!(exchange_names.contains(&"coinbase"));
        assert!(exchange_names.contains(&"kraken"));
    }

    #[test]
    fn should_enforce_user_id_foreign_key_relationship() {
        let exchange_factory = StandardExchangeAccountFactory::default();

        let result = exchange_factory.create_exchange_account(
            Uuid::nil(),
            "binance",
            b"encrypted_key".to_vec(),
            b"encrypted_secret".to_vec(),
            json!({"spot_trading": true}),
        );

        assert!(result.is_err());
    }
}
