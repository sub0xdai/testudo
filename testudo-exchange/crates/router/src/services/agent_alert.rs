//! AGENT-02: Centralized agent alert and execution report emission.
//!
//! Functions in this module convert internal risk events (RiskWarning,
//! RiskRejection) into AgentAlert structs and broadcast them via
//! PostgreSQL NOTIFY on the agent_alert and agent_execution channels.

// @anchor exchange:router:agent_alert
// @tags api

use chrono::{DateTime, Duration, Utc};
use common_utils::agent::{AgentAlert, AlertSeverity, AlertType, ExecutionReport};
use common_utils::risk::{RiskRejection, RiskWarning};
use rust_decimal::Decimal;
use sqlx::PgPool;
use uuid::Uuid;

/// Default TTL for agent wallet approvals (30 days on Hyperliquid L1).
const AGENT_WALLET_TTL_DAYS: i64 = 30;

/// Build an AgentAlert from a RiskWarning, if the warning is alert-worthy.
pub fn alert_from_warning(warning: &RiskWarning, _user_id: Uuid) -> Option<AgentAlert> {
    match warning {
        RiskWarning::ApproachingDrawdownLimit {
            current_drawdown_percent,
            limit_percent,
        } => Some(AgentAlert {
            alert_type: AlertType::DrawdownWarning,
            severity: AlertSeverity::Notable,
            message: format!(
                "Drawdown at {:.1}%, approaching {:.1}% limit",
                current_drawdown_percent, limit_percent
            ),
            current_value: Some(*current_drawdown_percent),
            limit_value: Some(*limit_percent),
            timestamp: Utc::now(),
        }),
        _ => None,
    }
}

/// Build an AgentAlert from a RiskRejection, if the rejection is alert-worthy.
pub fn alert_from_rejection(rejection: &RiskRejection, _user_id: Uuid) -> Option<AgentAlert> {
    match rejection {
        RiskRejection::DailyDrawdownExceeded {
            current_drawdown_percent,
            limit_percent,
        } => Some(AgentAlert {
            alert_type: AlertType::DrawdownLimit,
            severity: AlertSeverity::Concerning,
            message: format!(
                "Daily drawdown limit exceeded: {:.1}% (limit {:.1}%)",
                current_drawdown_percent, limit_percent
            ),
            current_value: Some(*current_drawdown_percent),
            limit_value: Some(*limit_percent),
            timestamp: Utc::now(),
        }),
        RiskRejection::MaxPositionsReached { current, maximum } => Some(AgentAlert {
            alert_type: AlertType::MaxPositionsReached,
            severity: AlertSeverity::Info,
            message: format!("Max positions reached: {} of {}", current, maximum),
            current_value: Some(Decimal::from(*current)),
            limit_value: Some(Decimal::from(*maximum)),
            timestamp: Utc::now(),
        }),
        _ => None,
    }
}

/// Build an ExecutionReport from order placement result fields.
pub fn build_execution_report(
    trade_group_id: Uuid,
    order_id: &str,
    status: &str,
    fill_price: Option<Decimal>,
    exchange: &str,
    latency_ms: u64,
) -> ExecutionReport {
    ExecutionReport {
        trade_group_id,
        order_id: order_id.to_string(),
        status: status.to_string(),
        fill_price,
        exchange: exchange.to_string(),
        latency_ms,
        timestamp: Utc::now(),
    }
}

/// Emit an alert via PostgreSQL NOTIFY on the agent_alert channel.
/// Fire-and-forget: errors are logged, not propagated.
pub async fn emit_alert(pool: &PgPool, user_id: Uuid, alert: &AgentAlert) {
    let channel = format!("agent.alert.{}", user_id);
    let payload = match serde_json::to_string(alert) {
        Ok(p) => p,
        Err(e) => {
            tracing::error!(error = %e, "Failed to serialize AgentAlert");
            return;
        }
    };

    if let Err(e) = sqlx::query("SELECT pg_notify($1, $2)")
        .bind("agent_alert")
        .bind(&payload)
        .execute(pool)
        .await
    {
        tracing::warn!(channel = %channel, error = %e, "pg_notify agent_alert failed");
    }
}

/// Check if an agent wallet is approaching expiry or has expired.
///
/// Returns an `AgentAlert` if the wallet is within 24h of expiry,
/// within 1h of expiry (escalated severity), or already expired.
pub fn check_agent_wallet_expiry(
    agent_approved_at: DateTime<Utc>,
    now: DateTime<Utc>,
) -> Option<AgentAlert> {
    let ttl = Duration::days(AGENT_WALLET_TTL_DAYS);
    let expires_at = agent_approved_at + ttl;
    let remaining = expires_at - now;

    if remaining.num_seconds() <= 0 {
        return Some(AgentAlert {
            alert_type: AlertType::AgentWalletExpired,
            severity: AlertSeverity::Concerning,
            message: "Agent wallet has expired. Re-authorize in Account settings.".to_string(),
            current_value: None,
            limit_value: None,
            timestamp: now,
        });
    }

    let one_hour = Duration::hours(1);
    let twenty_four_hours = Duration::hours(24);

    if remaining <= one_hour {
        return Some(AgentAlert {
            alert_type: AlertType::AgentWalletExpiring,
            severity: AlertSeverity::Concerning,
            message: format!(
                "Agent wallet expires in {} minutes. Re-authorize now.",
                remaining.num_minutes()
            ),
            current_value: None,
            limit_value: None,
            timestamp: now,
        });
    }

    if remaining <= twenty_four_hours {
        return Some(AgentAlert {
            alert_type: AlertType::AgentWalletExpiring,
            severity: AlertSeverity::Notable,
            message: format!(
                "Agent wallet expires in {:.0} hours. Plan re-authorization.",
                remaining.num_hours()
            ),
            current_value: None,
            limit_value: None,
            timestamp: now,
        });
    }

    None
}

/// Emit an execution report via PostgreSQL NOTIFY on the agent_execution channel.
pub async fn emit_execution_report(pool: &PgPool, user_id: Uuid, report: &ExecutionReport) {
    let channel = format!("agent.execution.{}", user_id);
    let payload = match serde_json::to_string(report) {
        Ok(p) => p,
        Err(e) => {
            tracing::error!(error = %e, "Failed to serialize ExecutionReport");
            return;
        }
    };

    if let Err(e) = sqlx::query("SELECT pg_notify($1, $2)")
        .bind("agent_execution")
        .bind(&payload)
        .execute(pool)
        .await
    {
        tracing::warn!(channel = %channel, error = %e, "pg_notify agent_execution failed");
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rust_decimal_macros::dec;

    #[test]
    fn alert_from_drawdown_warning_is_notable() {
        let warning = RiskWarning::ApproachingDrawdownLimit {
            current_drawdown_percent: dec!(4.2),
            limit_percent: dec!(5),
        };
        let alert = alert_from_warning(&warning, Uuid::new_v4())
            .expect("drawdown warning should produce alert");
        assert_eq!(alert.alert_type, AlertType::DrawdownWarning);
        assert_eq!(alert.severity, AlertSeverity::Notable);
        assert_eq!(alert.current_value, Some(dec!(4.2)));
    }

    #[test]
    fn alert_from_drawdown_breach_is_concerning() {
        let rejection = RiskRejection::DailyDrawdownExceeded {
            current_drawdown_percent: dec!(5.5),
            limit_percent: dec!(5),
        };
        let alert = alert_from_rejection(&rejection, Uuid::new_v4())
            .expect("drawdown breach should produce alert");
        assert_eq!(alert.alert_type, AlertType::DrawdownLimit);
        assert_eq!(alert.severity, AlertSeverity::Concerning);
    }

    #[test]
    fn alert_from_max_positions_is_info() {
        let rejection = RiskRejection::MaxPositionsReached {
            current: 5,
            maximum: 5,
        };
        let alert = alert_from_rejection(&rejection, Uuid::new_v4())
            .expect("max positions should produce alert");
        assert_eq!(alert.alert_type, AlertType::MaxPositionsReached);
        assert_eq!(alert.severity, AlertSeverity::Info);
    }

    #[test]
    fn non_alert_warnings_return_none() {
        let warning = RiskWarning::TightStopLoss {
            stop_distance_percent: dec!(0.3),
        };
        assert!(alert_from_warning(&warning, Uuid::new_v4()).is_none());
    }

    #[test]
    fn non_alert_rejections_return_none() {
        let rejection = RiskRejection::StopLossRequired;
        assert!(alert_from_rejection(&rejection, Uuid::new_v4()).is_none());
    }

    // --- CP-3: Execution report construction ---

    #[test]
    fn execution_report_has_required_fields() {
        let report = build_execution_report(
            Uuid::new_v4(),
            "binance-order-42",
            "filled",
            Some(dec!(50000)),
            "binance",
            342,
        );
        assert_eq!(report.status, "filled");
        assert_eq!(report.exchange, "binance");
        assert_eq!(report.latency_ms, 342);
        assert_eq!(report.fill_price, Some(dec!(50000)));
        assert_eq!(report.order_id, "binance-order-42");
    }

    #[test]
    fn execution_report_serializes_as_valid_json() {
        let report = build_execution_report(
            Uuid::parse_str("550e8400-e29b-41d4-a716-446655440000").unwrap(),
            "hl-order-99",
            "rejected",
            None,
            "hyperliquid",
            150,
        );
        let json = serde_json::to_string(&report).unwrap();
        assert!(json.contains("trade_group_id"));
        assert!(json.contains("550e8400"));
        assert!(json.contains("hl-order-99"));
        assert!(json.contains("rejected"));
        assert!(json.contains("hyperliquid"));
        assert!(json.contains("latency_ms\":150"));
        // fill_price should be absent for rejected orders
        assert!(!json.contains("fill_price"));
    }

    // --- CP-4: Agent wallet expiry detection ---

    #[test]
    fn wallet_approved_29_days_ago_is_expiring_notable() {
        let now = Utc::now();
        let approved_at = now - Duration::days(29);
        let alert = check_agent_wallet_expiry(approved_at, now)
            .expect("29 days should trigger expiry warning");
        assert_eq!(alert.alert_type, AlertType::AgentWalletExpiring);
        assert_eq!(alert.severity, AlertSeverity::Notable);
        assert!(alert.message.contains("24 hours"));
    }

    #[test]
    fn wallet_approved_30_days_ago_is_expired() {
        let now = Utc::now();
        let approved_at = now - Duration::days(30) - Duration::minutes(1);
        let alert = check_agent_wallet_expiry(approved_at, now)
            .expect("30+ days should trigger expired alert");
        assert_eq!(alert.alert_type, AlertType::AgentWalletExpired);
        assert_eq!(alert.severity, AlertSeverity::Concerning);
    }

    #[test]
    fn wallet_approved_30_days_minus_30_minutes_is_expiring_concerning() {
        let now = Utc::now();
        let approved_at = now - Duration::days(30) + Duration::minutes(30);
        let alert = check_agent_wallet_expiry(approved_at, now)
            .expect("30 min remaining should trigger expiry");
        assert_eq!(alert.alert_type, AlertType::AgentWalletExpiring);
        assert_eq!(alert.severity, AlertSeverity::Concerning);
        assert!(alert.message.contains("minutes"));
    }

    #[test]
    fn wallet_approved_7_days_ago_returns_none() {
        let now = Utc::now();
        let approved_at = now - Duration::days(7);
        assert!(check_agent_wallet_expiry(approved_at, now).is_none());
    }
}
