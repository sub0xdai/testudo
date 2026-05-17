use common_utils::{
    auth::AuthError,
    models::User,
};
use sqlx::PgPool;

/// PostgreSQL user repository — wallet-primary (AUTH-02)
pub struct PostgresUserRepository {
    pool: PgPool,
}

impl PostgresUserRepository {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    /// Find a user by wallet address
    pub async fn find_by_wallet_address(
        &self,
        wallet_address: &str,
    ) -> Result<Option<User>, AuthError> {
        let row: Option<UserRow> = sqlx::query_as::<_, UserRow>(
            "SELECT id, wallet_address, created_at, updated_at, is_active, \
                    coach_enabled, coach_banner_last_viewed_at \
             FROM users WHERE wallet_address = $1",
        )
        .bind(wallet_address)
        .fetch_optional(&self.pool)
        .await
        .map_err(|e| AuthError::RepositoryError(e.to_string()))?;

        Ok(row.map(|r| r.into_user()))
    }

    /// Find a user by ID
    pub async fn find_by_id(&self, user_id: &uuid::Uuid) -> Result<Option<User>, AuthError> {
        let row: Option<UserRow> = sqlx::query_as::<_, UserRow>(
            "SELECT id, wallet_address, created_at, updated_at, is_active, \
                    coach_enabled, coach_banner_last_viewed_at \
             FROM users WHERE id = $1",
        )
        .bind(user_id)
        .fetch_optional(&self.pool)
        .await
        .map_err(|e| AuthError::RepositoryError(e.to_string()))?;

        Ok(row.map(|r| r.into_user()))
    }

    /// Find existing user by wallet address, or create a new one.
    /// Uses INSERT ... ON CONFLICT to avoid TOCTOU race when two concurrent
    /// SIWE logins arrive for the same wallet address.
    pub async fn find_or_create_by_wallet(
        &self,
        wallet_address: &str,
    ) -> Result<User, AuthError> {
        let user = User::new(wallet_address.to_string());
        let row: UserRow = sqlx::query_as::<_, UserRow>(
            "INSERT INTO users (id, wallet_address, created_at, updated_at, is_active) \
             VALUES ($1, $2, $3, $4, $5) \
             ON CONFLICT (wallet_address) DO UPDATE SET updated_at = NOW() \
             RETURNING id, wallet_address, created_at, updated_at, is_active, \
                       coach_enabled, coach_banner_last_viewed_at",
        )
        .bind(user.id)
        .bind(&user.wallet_address)
        .bind(user.created_at)
        .bind(user.updated_at)
        .bind(user.is_active)
        .fetch_one(&self.pool)
        .await
        .map_err(|e| AuthError::RepositoryError(e.to_string()))?;

        Ok(row.into_user())
    }
}

/// Internal row type for SQLx query mapping
#[derive(sqlx::FromRow)]
struct UserRow {
    id: uuid::Uuid,
    wallet_address: String,
    created_at: chrono::DateTime<chrono::Utc>,
    updated_at: chrono::DateTime<chrono::Utc>,
    is_active: bool,
    coach_enabled: bool,
    coach_banner_last_viewed_at: Option<chrono::DateTime<chrono::Utc>>,
}

impl UserRow {
    fn into_user(self) -> User {
        User {
            id: self.id,
            wallet_address: self.wallet_address,
            created_at: self.created_at,
            updated_at: self.updated_at,
            is_active: self.is_active,
            coach_enabled: self.coach_enabled,
            coach_banner_last_viewed_at: self.coach_banner_last_viewed_at,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_user_row_conversion() {
        let row = UserRow {
            id: uuid::Uuid::new_v4(),
            wallet_address: "0xC285000000000000000000000000000000005b36".to_string(),
            created_at: chrono::Utc::now(),
            updated_at: chrono::Utc::now(),
            is_active: true,
            coach_enabled: true,
            coach_banner_last_viewed_at: None,
        };

        let user = row.into_user();
        assert_eq!(
            user.wallet_address,
            "0xC285000000000000000000000000000000005b36"
        );
        assert!(user.is_active);
    }
}
