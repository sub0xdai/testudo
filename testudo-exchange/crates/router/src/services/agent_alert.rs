//! AGENT-02: Centralized agent alert and execution report emission.
//!
//! Functions in this module convert internal risk events (RiskWarning,
//! RiskRejection) into AgentAlert structs and broadcast them via
//! PostgreSQL NOTIFY on the agent_alert and agent_execution channels.

use chrono::Utc;
use common_utils::agent::{AgentAlert, AlertSeverity, AlertType, ExecutionReport};
use common_utils::risk::{RiskRejection, RiskWarning};
use rust_decimal::Decimal;
use sqlx::PgPool;
use uuid::Uuid;

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
}
