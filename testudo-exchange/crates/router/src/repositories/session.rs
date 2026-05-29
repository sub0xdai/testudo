// @anchor exchange:router:session
// @tags api

use common_utils::auth::AuthError;
use sqlx::PgPool;
use uuid::Uuid;

/// PostgreSQL session repository — server-side refresh token tracking (AUTH-02)
#[derive(Clone)]
pub struct SessionRepository {
    pool: PgPool,
}

/// Domain model for a user session
#[derive(Debug, Clone)]
pub struct UserSession {
    pub id: Uuid,
    pub user_id: Uuid,
    pub refresh_token_hash: String,
    pub ip_address: Option<String>,
    pub user_agent: Option<String>,
    pub is_revoked: bool,
    pub expires_at: chrono::DateTime<chrono::Utc>,
    pub created_at: chrono::DateTime<chrono::Utc>,
    pub last_used_at: chrono::DateTime<chrono::Utc>,
}

/// Input for creating a new session
pub struct NewSession {
    pub user_id: Uuid,
    pub refresh_token_hash: String,
    pub ip_address: Option<String>,
    pub user_agent: Option<String>,
    pub expires_at: chrono::DateTime<chrono::Utc>,
}

/// Internal row type for SQLx query mapping
#[derive(sqlx::FromRow)]
struct SessionRow {
    id: Uuid,
    user_id: Uuid,
    refresh_token_hash: String,
    ip_address: Option<String>,
    user_agent: Option<String>,
    is_revoked: bool,
    expires_at: chrono::DateTime<chrono::Utc>,
    created_at: chrono::DateTime<chrono::Utc>,
    last_used_at: chrono::DateTime<chrono::Utc>,
}

impl SessionRow {
    fn into_session(self) -> UserSession {
        UserSession {
            id: self.id,
            user_id: self.user_id,
            refresh_token_hash: self.refresh_token_hash,
            ip_address: self.ip_address,
            user_agent: self.user_agent,
            is_revoked: self.is_revoked,
            expires_at: self.expires_at,
            created_at: self.created_at,
            last_used_at: self.last_used_at,
        }
    }
}

impl SessionRepository {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    /// Insert a new session row with the hashed refresh token
    pub async fn create_session(&self, session: NewSession) -> Result<UserSession, AuthError> {
        let row: SessionRow = sqlx::query_as::<_, SessionRow>(
            "INSERT INTO user_sessions (user_id, refresh_token_hash, ip_address, user_agent, expires_at) \
             VALUES ($1, $2, $3, $4, $5) \
             RETURNING id, user_id, refresh_token_hash, ip_address, user_agent, is_revoked, expires_at, created_at, last_used_at",
        )
        .bind(session.user_id)
        .bind(&session.refresh_token_hash)
        .bind(&session.ip_address)
        .bind(&session.user_agent)
        .bind(session.expires_at)
        .fetch_one(&self.pool)
        .await
        .map_err(|e| AuthError::RepositoryError(e.to_string()))?;

        Ok(row.into_session())
    }

    /// Look up a session by SHA-256(refresh_token). Returns None if not found.
    pub async fn find_by_token_hash(
        &self,
        token_hash: &str,
    ) -> Result<Option<UserSession>, AuthError> {
        let row: Option<SessionRow> = sqlx::query_as::<_, SessionRow>(
            "SELECT id, user_id, refresh_token_hash, ip_address, user_agent, is_revoked, \
                    expires_at, created_at, last_used_at \
             FROM user_sessions WHERE refresh_token_hash = $1",
        )
        .bind(token_hash)
        .fetch_optional(&self.pool)
        .await
        .map_err(|e| AuthError::RepositoryError(e.to_string()))?;

        Ok(row.map(|r| r.into_session()))
    }

    /// Mark a single session as revoked
    pub async fn revoke_session(&self, session_id: Uuid) -> Result<(), AuthError> {
        sqlx::query("UPDATE user_sessions SET is_revoked = TRUE WHERE id = $1")
            .bind(session_id)
            .execute(&self.pool)
            .await
            .map_err(|e| AuthError::RepositoryError(e.to_string()))?;

        Ok(())
    }

    /// Revoke all sessions for a user. Returns the number of sessions revoked.
    pub async fn revoke_all_for_user(&self, user_id: Uuid) -> Result<u64, AuthError> {
        let result = sqlx::query(
            "UPDATE user_sessions SET is_revoked = TRUE \
             WHERE user_id = $1 AND is_revoked = FALSE",
        )
        .bind(user_id)
        .execute(&self.pool)
        .await
        .map_err(|e| AuthError::RepositoryError(e.to_string()))?;

        Ok(result.rows_affected())
    }

    /// Touch the last_used_at timestamp for an active session
    pub async fn update_last_used(&self, session_id: Uuid) -> Result<(), AuthError> {
        sqlx::query("UPDATE user_sessions SET last_used_at = NOW() WHERE id = $1")
            .bind(session_id)
            .execute(&self.pool)
            .await
            .map_err(|e| AuthError::RepositoryError(e.to_string()))?;

        Ok(())
    }

    /// Delete expired or revoked sessions. Returns the number of rows removed.
    pub async fn cleanup_expired(&self) -> Result<u64, AuthError> {
        let result = sqlx::query(
            "DELETE FROM user_sessions WHERE expires_at < NOW() OR is_revoked = TRUE",
        )
        .execute(&self.pool)
        .await
        .map_err(|e| AuthError::RepositoryError(e.to_string()))?;

        Ok(result.rows_affected())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_session_row_conversion() {
        let now = chrono::Utc::now();
        let row = SessionRow {
            id: Uuid::new_v4(),
            user_id: Uuid::new_v4(),
            refresh_token_hash: "abc123hash".to_string(),
            ip_address: Some("192.168.1.1".to_string()),
            user_agent: Some("Mozilla/5.0".to_string()),
            is_revoked: false,
            expires_at: now + chrono::Duration::days(7),
            created_at: now,
            last_used_at: now,
        };

        let session = row.into_session();
        assert_eq!(session.refresh_token_hash, "abc123hash");
        assert_eq!(session.ip_address.as_deref(), Some("192.168.1.1"));
        assert!(!session.is_revoked);
    }

    #[test]
    fn test_session_row_conversion_null_optionals() {
        let now = chrono::Utc::now();
        let row = SessionRow {
            id: Uuid::new_v4(),
            user_id: Uuid::new_v4(),
            refresh_token_hash: "def456hash".to_string(),
            ip_address: None,
            user_agent: None,
            is_revoked: true,
            expires_at: now,
            created_at: now,
            last_used_at: now,
        };

        let session = row.into_session();
        assert!(session.ip_address.is_none());
        assert!(session.user_agent.is_none());
        assert!(session.is_revoked);
    }

    #[test]
    fn test_new_session_construction() {
        let user_id = Uuid::new_v4();
        let expires = chrono::Utc::now() + chrono::Duration::days(7);
        let new_session = NewSession {
            user_id,
            refresh_token_hash: "sha256hex".to_string(),
            ip_address: Some("10.0.0.1".to_string()),
            user_agent: Some("TestAgent/1.0".to_string()),
            expires_at: expires,
        };

        assert_eq!(new_session.user_id, user_id);
        assert_eq!(new_session.refresh_token_hash, "sha256hex");
    }

    #[test]
    fn test_session_repository_construction() {
        // Verify SessionRepository can be constructed with a pool
        // (actual DB tests require a running PostgreSQL instance)
        use sqlx::postgres::PgPoolOptions;

        // Just verify the type compiles and Clone works
        // Cannot create a real pool without a database URL
        let _: fn(PgPool) -> SessionRepository = SessionRepository::new;
    }
}
