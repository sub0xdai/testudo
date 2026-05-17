//! Position Repository
//!
//! PostgreSQL persistence layer for managed positions (EXT-09 FR-9).
//! Enables restart recovery by persisting position state.

use chrono::Utc;
use rust_decimal::Decimal;
use sqlx::PgPool;
use uuid::Uuid;

use super::types::*;

/// Repository for persisting managed positions to PostgreSQL.
#[derive(Clone)]
pub struct PositionRepository {
    pool: PgPool,
}

impl PositionRepository {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    /// Create the managed_positions table if it doesn't exist.
    /// Called at startup, follows existing pattern from sqlx_postgres/lib.rs.
    pub async fn create_table(&self) -> Result<(), sqlx::Error> {
        sqlx::query(
            r#"
            CREATE TABLE IF NOT EXISTS managed_positions (
                id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
                user_id UUID NOT NULL,
                symbol VARCHAR(32) NOT NULL,
                side VARCHAR(5) NOT NULL,
                entry_price DECIMAL NOT NULL,
                stop_price DECIMAL NOT NULL,
                target_price DECIMAL NOT NULL,
                quantity DECIMAL NOT NULL,
                risk_percent DECIMAL NOT NULL,
                break_even_at INTEGER NOT NULL,
                trailing_enabled BOOLEAN NOT NULL DEFAULT false,
                trailing_distance INTEGER DEFAULT 0,
                partial_tp_enabled BOOLEAN NOT NULL DEFAULT false,
                partial_tp_percent INTEGER DEFAULT 0,
                state VARCHAR(16) NOT NULL DEFAULT 'pending',
                be_triggered BOOLEAN NOT NULL DEFAULT false,
                partial_tp_fired BOOLEAN NOT NULL DEFAULT false,
                current_stop DECIMAL,
                remaining_quantity DECIMAL,
                exchange_order_ids JSONB,
                exchange_account_id UUID,
                leverage SMALLINT NOT NULL DEFAULT 1,
                setup_tag TEXT,
                created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
                updated_at TIMESTAMPTZ NOT NULL DEFAULT now()
            )
            "#,
        )
        .execute(&self.pool)
        .await?;

        // Idempotent migration for existing tables
        sqlx::query(
            r#"
            ALTER TABLE managed_positions
                ADD COLUMN IF NOT EXISTS exchange_account_id UUID,
                ADD COLUMN IF NOT EXISTS leverage SMALLINT NOT NULL DEFAULT 1,
                ADD COLUMN IF NOT EXISTS setup_tag TEXT
            "#,
        )
        .execute(&self.pool)
        .await?;

        Ok(())
    }

    /// Insert a new managed position.
    pub async fn insert(&self, position: &ManagedPosition) -> Result<(), sqlx::Error> {
        let side_str = match position.side {
            PositionSide::Long => "long",
            PositionSide::Short => "short",
        };
        let state_str = state_to_str(&position.state);
        let (trailing_enabled, trailing_distance) = match &position.rules.trailing_stop {
            Some(t) => (t.enabled, t.distance_percent as i32),
            None => (false, 0),
        };
        let (partial_tp_enabled, partial_tp_percent) = match &position.rules.partial_tp {
            Some(p) => (p.enabled, p.close_percent as i32),
            None => (false, 0),
        };
        let order_ids_json =
            serde_json::to_value(&position.exchange_order_ids).unwrap_or(serde_json::Value::Null);

        sqlx::query(
            r#"
            INSERT INTO managed_positions (
                id, user_id, symbol, side, entry_price, stop_price, target_price,
                quantity, risk_percent, break_even_at, trailing_enabled, trailing_distance,
                partial_tp_enabled, partial_tp_percent, state, be_triggered, partial_tp_fired,
                current_stop, remaining_quantity, exchange_order_ids, exchange_account_id,
                leverage, setup_tag, created_at
            ) VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13, $14, $15, $16, $17, $18, $19, $20, $21, $22, $23, $24)
            "#,
        )
        .bind(position.id)
        .bind(position.user_id)
        .bind(&position.symbol)
        .bind(side_str)
        .bind(position.entry_price)
        .bind(position.stop_price)
        .bind(position.target_price)
        .bind(position.quantity)
        .bind(position.rules.risk_percent)
        .bind(position.rules.break_even_at as i32)
        .bind(trailing_enabled)
        .bind(trailing_distance)
        .bind(partial_tp_enabled)
        .bind(partial_tp_percent)
        .bind(state_str)
        .bind(position.be_triggered)
        .bind(position.partial_tp_fired)
        .bind(position.current_stop)
        .bind(position.remaining_qty)
        .bind(order_ids_json)
        .bind(position.exchange_account_id)
        .bind(position.rules.leverage as i16)
        .bind(position.setup_tag.as_deref())
        .bind(position.created_at)
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    /// Load all non-closed positions (state IN ('pending', 'filled', 'managing')).
    pub async fn load_active(&self) -> Result<Vec<ManagedPosition>, sqlx::Error> {
        let rows = sqlx::query_as::<_, PositionRow>(
            r#"
            SELECT id, user_id, symbol, side, entry_price, stop_price, target_price,
                   quantity, risk_percent, break_even_at, trailing_enabled, trailing_distance,
                   partial_tp_enabled, partial_tp_percent, state, be_triggered, partial_tp_fired,
                   current_stop, remaining_quantity, exchange_order_ids, exchange_account_id,
                   leverage, setup_tag, created_at
            FROM managed_positions
            WHERE state IN ('pending', 'filled', 'managing')
            "#,
        )
        .fetch_all(&self.pool)
        .await?;

        Ok(rows.into_iter().map(|r| r.into_position()).collect())
    }

    /// Update mutable state fields for a position.
    pub async fn update_state(
        &self,
        id: Uuid,
        state: &PositionState,
        be_triggered: bool,
        partial_tp_fired: bool,
        current_stop: Decimal,
        remaining_qty: Decimal,
    ) -> Result<(), sqlx::Error> {
        sqlx::query(
            r#"
            UPDATE managed_positions
            SET state = $2, be_triggered = $3, partial_tp_fired = $4,
                current_stop = $5, remaining_quantity = $6, updated_at = $7
            WHERE id = $1
            "#,
        )
        .bind(id)
        .bind(state_to_str(state))
        .bind(be_triggered)
        .bind(partial_tp_fired)
        .bind(current_stop)
        .bind(remaining_qty)
        .bind(Utc::now())
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    /// Mark a position as closed.
    pub async fn mark_closed(&self, id: Uuid) -> Result<(), sqlx::Error> {
        sqlx::query(
            r#"
            UPDATE managed_positions SET state = 'closed', updated_at = $2 WHERE id = $1
            "#,
        )
        .bind(id)
        .bind(Utc::now())
        .execute(&self.pool)
        .await?;
        Ok(())
    }
}

fn state_to_str(state: &PositionState) -> &'static str {
    match state {
        PositionState::Pending => "pending",
        PositionState::Filled => "filled",
        PositionState::Managing => "managing",
        PositionState::Closed => "closed",
    }
}

fn str_to_state(s: &str) -> PositionState {
    match s {
        "filled" => PositionState::Filled,
        "managing" => PositionState::Managing,
        "closed" => PositionState::Closed,
        _ => PositionState::Pending,
    }
}

fn str_to_side(s: &str) -> PositionSide {
    match s {
        "short" => PositionSide::Short,
        _ => PositionSide::Long,
    }
}

/// Internal row type for sqlx deserialization.
#[derive(sqlx::FromRow)]
struct PositionRow {
    id: Uuid,
    user_id: Uuid,
    symbol: String,
    side: String,
    entry_price: Decimal,
    stop_price: Decimal,
    target_price: Decimal,
    quantity: Decimal,
    risk_percent: Decimal,
    break_even_at: i32,
    trailing_enabled: bool,
    trailing_distance: Option<i32>,
    partial_tp_enabled: bool,
    partial_tp_percent: Option<i32>,
    state: String,
    be_triggered: bool,
    partial_tp_fired: bool,
    current_stop: Option<Decimal>,
    remaining_quantity: Option<Decimal>,
    exchange_order_ids: Option<serde_json::Value>,
    exchange_account_id: Option<Uuid>,
    leverage: i16,
    setup_tag: Option<String>,
    created_at: chrono::DateTime<Utc>,
}

impl PositionRow {
    fn into_position(self) -> ManagedPosition {
        let trailing_stop = if self.trailing_enabled {
            Some(TrailingStopRule {
                enabled: true,
                distance_percent: self.trailing_distance.unwrap_or(0) as u32,
            })
        } else {
            None
        };

        let partial_tp = if self.partial_tp_enabled {
            Some(PartialTpRule {
                enabled: true,
                close_percent: self.partial_tp_percent.unwrap_or(0) as u32,
            })
        } else {
            None
        };

        let exchange_order_ids = self
            .exchange_order_ids
            .and_then(|v| serde_json::from_value(v).ok())
            .unwrap_or_default();

        ManagedPosition {
            id: self.id,
            user_id: self.user_id,
            symbol: self.symbol,
            side: str_to_side(&self.side),
            entry_price: self.entry_price,
            stop_price: self.stop_price,
            target_price: self.target_price,
            quantity: self.quantity,
            rules: ManagementRules {
                risk_percent: self.risk_percent,
                break_even_at: self.break_even_at as u32,
                leverage: self.leverage as u8,
                trailing_stop,
                partial_tp,
            },
            state: str_to_state(&self.state),
            be_triggered: self.be_triggered,
            partial_tp_fired: self.partial_tp_fired,
            current_stop: self.current_stop.unwrap_or(self.stop_price),
            remaining_qty: self.remaining_quantity.unwrap_or(self.quantity),
            exchange_order_ids,
            created_at: self.created_at,
            exchange_account_id: self.exchange_account_id,
            setup_tag: self.setup_tag,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rust_decimal_macros::dec;

    #[test]
    fn test_state_roundtrip() {
        let states = vec![
            PositionState::Pending,
            PositionState::Filled,
            PositionState::Managing,
            PositionState::Closed,
        ];
        for state in states {
            let s = state_to_str(&state);
            let roundtripped = str_to_state(s);
            assert_eq!(state, roundtripped);
        }
    }

    #[test]
    fn test_side_roundtrip() {
        assert_eq!(str_to_side("long"), PositionSide::Long);
        assert_eq!(str_to_side("short"), PositionSide::Short);
        assert_eq!(str_to_side("unknown"), PositionSide::Long); // default
    }

    #[test]
    fn test_position_row_conversion() {
        let account_id = Uuid::new_v4();
        let row = PositionRow {
            id: Uuid::new_v4(),
            user_id: Uuid::new_v4(),
            symbol: "BTC_USDT".to_string(),
            side: "long".to_string(),
            entry_price: dec!(50000),
            stop_price: dec!(49000),
            target_price: dec!(52000),
            quantity: dec!(0.2),
            risk_percent: dec!(2),
            break_even_at: 50,
            trailing_enabled: true,
            trailing_distance: Some(20),
            partial_tp_enabled: true,
            partial_tp_percent: Some(50),
            state: "filled".to_string(),
            be_triggered: false,
            partial_tp_fired: false,
            current_stop: Some(dec!(49000)),
            remaining_quantity: Some(dec!(0.2)),
            exchange_order_ids: None,
            exchange_account_id: Some(account_id),
            leverage: 10,
            setup_tag: None,
            created_at: Utc::now(),
        };

        let pos = row.into_position();
        assert_eq!(pos.side, PositionSide::Long);
        assert_eq!(pos.state, PositionState::Filled);
        assert_eq!(pos.entry_price, dec!(50000));
        assert!(pos.rules.trailing_stop.is_some());
        assert_eq!(pos.rules.trailing_stop.unwrap().distance_percent, 20);
        assert!(pos.rules.partial_tp.is_some());
        assert_eq!(pos.rules.partial_tp.unwrap().close_percent, 50);
        assert_eq!(pos.rules.leverage, 10);
        assert_eq!(pos.exchange_account_id, Some(account_id));
    }

    #[test]
    fn test_position_row_defaults_when_none() {
        let row = PositionRow {
            id: Uuid::new_v4(),
            user_id: Uuid::new_v4(),
            symbol: "ETH_USDT".to_string(),
            side: "short".to_string(),
            entry_price: dec!(3000),
            stop_price: dec!(3100),
            target_price: dec!(2800),
            quantity: dec!(1),
            risk_percent: dec!(1),
            break_even_at: 30,
            trailing_enabled: false,
            trailing_distance: None,
            partial_tp_enabled: false,
            partial_tp_percent: None,
            state: "managing".to_string(),
            be_triggered: true,
            partial_tp_fired: false,
            current_stop: None,
            remaining_quantity: None,
            exchange_order_ids: None,
            exchange_account_id: None,
            leverage: 1,
            setup_tag: Some("breakout".to_string()),
            created_at: Utc::now(),
        };

        let pos = row.into_position();
        assert_eq!(pos.side, PositionSide::Short);
        assert_eq!(pos.state, PositionState::Managing);
        assert!(pos.rules.trailing_stop.is_none());
        assert!(pos.rules.partial_tp.is_none());
        // Defaults to original values when None
        assert_eq!(pos.current_stop, dec!(3100));
        assert_eq!(pos.remaining_qty, dec!(1));
        assert_eq!(pos.rules.leverage, 1);
        assert_eq!(pos.exchange_account_id, None);
        assert_eq!(pos.setup_tag.as_deref(), Some("breakout"));
    }
}
