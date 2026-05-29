//! Startup Rehydration Service (EXT-24)
//!
//! Reconstructs in-memory OrderGroups from persisted ManagedPositions on startup.
//! This bridges the gap between PostgreSQL persistence (ManagedPosition) and the
//! in-memory OrderGroupManager used by list_trades and FillDetectorService.

// @anchor exchange:router:rehydration
// @tags api

use engine::shadow::order_group::{
    BreakEvenConfig, OrderGroup, OrderGroupStatus, TakeProfitTarget,
};
use engine::{EngineHandle, OrderRole};
use rust_decimal::Decimal;
use sqlx::PgPool;
use std::collections::{HashMap, HashSet};
use uuid::Uuid;

use super::cex_client::{CexClient, SidecarCredentials};
use super::exchange_api::to_cex_symbol;
use super::trade_manager::repository::PositionRepository;
use super::trade_manager::types::{ManagedPosition, PositionState};
use crate::repositories::exchange_account::ExchangeAccountRepository;

/// Rehydrates OrderGroups from the database at startup.
///
/// 019d: Uses EngineHandle instead of direct Arc<RwLock<ShadowEngine>> access.
pub struct RehydrationService {
    repository: PositionRepository,
    engine_handle: EngineHandle,
    pool: PgPool,
}

impl RehydrationService {
    pub fn new(repository: PositionRepository, engine_handle: EngineHandle, pool: PgPool) -> Self {
        Self {
            repository,
            engine_handle,
            pool,
        }
    }

    /// Load persisted positions and reconstruct OrderGroups in the ShadowEngine.
    ///
    /// Must be called after DB and ShadowEngine are initialized, but before
    /// PriceFeedService, TradeManagerService, FillDetectorService, and HTTP server.
    ///
    /// 019d: Uses EngineHandle load_order_groups + register_exchange_order_id.
    pub async fn rehydrate(&self) -> Result<RehydrationSummary, String> {
        let positions = self
            .repository
            .load_active()
            .await
            .map_err(|e| format!("Failed to load positions from DB: {}", e))?;

        if positions.is_empty() {
            tracing::info!("Rehydration: no active positions to restore");
            return Ok(RehydrationSummary {
                positions_loaded: 0,
                exchange_ids_registered: 0,
            });
        }

        // CON-01a: Batch-fetch exchange_names for positions with exchange_account_id
        let account_ids: Vec<Uuid> = positions
            .iter()
            .filter_map(|p| p.exchange_account_id)
            .collect::<HashSet<_>>()
            .into_iter()
            .collect();

        let exchange_name_map: HashMap<Uuid, String> = if !account_ids.is_empty() {
            sqlx::query_as::<_, (Uuid, String)>(
                "SELECT id, exchange_name FROM exchange_accounts WHERE id = ANY($1)"
            )
            .bind(&account_ids)
            .fetch_all(&self.pool)
            .await
            .unwrap_or_default()
            .into_iter()
            .collect()
        } else {
            HashMap::new()
        };

        // Build order groups from persisted positions
        let mut groups: Vec<OrderGroup> = positions.iter().map(build_order_group).collect();

        // Set exchange_name on groups from the lookup
        for (group, position) in groups.iter_mut().zip(positions.iter()) {
            if let Some(acc_id) = position.exchange_account_id {
                group.exchange_name = exchange_name_map.get(&acc_id).cloned();
            }
        }

        let positions_loaded = groups.len();

        // Bulk-load groups into the actor
        self.engine_handle
            .load_order_groups(groups)
            .await
            .map_err(|e| format!("Failed to load order groups into engine: {}", e))?;

        // Register exchange order IDs in the reverse index
        let mut exchange_ids_registered: usize = 0;
        for position in &positions {
            let group_id = position.id;
            if let Some(ref id) = position.exchange_order_ids.entry_order_id {
                let _ = self
                    .engine_handle
                    .register_exchange_order_id(group_id, OrderRole::Entry, id.clone())
                    .await;
                exchange_ids_registered += 1;
            }
            if let Some(ref id) = position.exchange_order_ids.stop_loss_order_id {
                let _ = self
                    .engine_handle
                    .register_exchange_order_id(group_id, OrderRole::StopLoss, id.clone())
                    .await;
                exchange_ids_registered += 1;
            }
            if let Some(ref id) = position.exchange_order_ids.take_profit_order_id {
                let _ = self
                    .engine_handle
                    .register_exchange_order_id(group_id, OrderRole::TakeProfit, id.clone())
                    .await;
                exchange_ids_registered += 1;
            }
        }

        let summary = RehydrationSummary {
            positions_loaded,
            exchange_ids_registered,
        };

        tracing::info!(
            "Rehydrated {} positions ({} exchange IDs registered)",
            summary.positions_loaded,
            summary.exchange_ids_registered,
        );

        Ok(summary)
    }

    /// FR-3: Verify rehydrated positions against the live exchange.
    ///
    /// For each position with an exchange_account_id, fetches open orders from
    /// the exchange and checks if SL/TP orders still exist. If an order has
    /// filled during downtime, updates the OrderGroup status accordingly.
    ///
    /// Gated by `REHYDRATION_VERIFY_EXCHANGE=true` env var.
    pub async fn verify_exchange(
        &self,
        cex_client: &CexClient,
        exchange_account_repo: &ExchangeAccountRepository,
        sandbox: bool,
    ) -> VerificationSummary {
        let mut verified = 0usize;
        let mut stale_detected = 0usize;
        let mut errors = 0usize;

        // Load positions that have exchange accounts for verification
        let positions = match self.repository.load_active().await {
            Ok(p) => p,
            Err(e) => {
                tracing::error!("Exchange verification: failed to load positions: {}", e);
                return VerificationSummary {
                    verified,
                    stale_detected,
                    errors: 1,
                };
            }
        };

        for position in &positions {
            let account_id = match position.exchange_account_id {
                Some(id) => id,
                None => continue, // Paper trade or missing account
            };

            // Load credentials for this exchange account
            let creds = match exchange_account_repo
                .load_credentials(account_id, position.user_id)
                .await
            {
                Ok(c) => c,
                Err(e) => {
                    tracing::warn!(
                        "Exchange verification: failed to load credentials for account {}: {}",
                        account_id,
                        e
                    );
                    errors += 1;
                    continue;
                }
            };

            let sidecar_creds = SidecarCredentials {
                api_key: creds.api_key,
                secret: creds.api_secret,
                password: creds.passphrase,
            };

            // FR-1 (016): Convert internal symbol format to CEX format
            let cex_symbol = to_cex_symbol(&position.symbol);

            // Fetch open orders for this symbol
            let open_orders = match cex_client
                .fetch_open_orders(
                    &creds.exchange_name,
                    &sidecar_creds,
                    sandbox,
                    &cex_symbol,
                )
                .await
            {
                Ok(orders) => orders,
                Err(e) => {
                    // FR-3 (016): Transient error — skip, will retry next restart
                    tracing::warn!(
                        "Exchange verification: fetchOpenOrders failed for {} on {} (transient, will retry next restart): {}",
                        position.symbol,
                        creds.exchange_name,
                        e
                    );
                    errors += 1;
                    continue;
                }
            };

            // Build a set of currently open exchange order IDs
            let open_ids: std::collections::HashSet<String> =
                open_orders.iter().map(|o| o.id.clone()).collect();

            // For Pending positions, verify the entry order still exists on exchange.
            // If the entry is gone, check positions to determine if it filled or was cancelled.
            if position.state == PositionState::Pending {
                let entry_alive = position
                    .exchange_order_ids
                    .entry_order_id
                    .as_ref()
                    .map_or(false, |id| open_ids.contains(id));

                if !entry_alive {
                    // Entry order is no longer open — did it fill or get cancelled?
                    // Check if there's a live position on the exchange for this symbol.
                    let has_position = match cex_client
                        .fetch_positions(
                            &creds.exchange_name,
                            &sidecar_creds,
                            sandbox,
                            Some(&cex_symbol),
                        )
                        .await
                    {
                        Ok(positions) => positions.iter().any(|p| {
                            p.contracts
                                .parse::<f64>()
                                .map_or(false, |c| c.abs() > 0.0)
                        }),
                        Err(e) => {
                            // FR-3 (016): Transient error — skip, will retry next restart
                            tracing::warn!(
                                "Exchange verification: fetchPositions failed for {} ({}) (transient, will retry next restart): {}",
                                position.symbol,
                                position.id,
                                e
                            );
                            errors += 1;
                            verified += 1;
                            continue;
                        }
                    };

                    if has_position {
                        // Entry filled — transition to Active via EngineHandle
                        if let Err(e) = self
                            .engine_handle
                            .on_entry_filled(position.id, position.entry_price)
                            .await
                        {
                            tracing::error!(
                                "Exchange verification: failed to mark entry filled for {}: {}",
                                position.id,
                                e
                            );
                        } else {
                            tracing::info!(
                                "Exchange verification: entry filled during downtime for pending {} ({}) — now Active",
                                position.symbol,
                                position.id
                            );
                        }
                        // Update DB state to 'filled'
                        if let Err(e) = self.repository.update_state(
                            position.id,
                            &PositionState::Filled,
                            position.be_triggered,
                            position.partial_tp_fired,
                            position.current_stop,
                            position.remaining_qty,
                        ).await {
                            tracing::error!(
                                "Exchange verification: failed to update DB state for {}: {}",
                                position.id,
                                e
                            );
                            errors += 1;
                        }
                    } else {
                        // FR-3 (016): Definitive — no entry order AND no position on exchange.
                        // Mark as Cancelled to prevent ghost pending positions.
                        if let Err(e) = self
                            .engine_handle
                            .update_group_status(position.id, OrderGroupStatus::Cancelled)
                            .await
                        {
                            tracing::error!(
                                "Exchange verification: failed to cancel group {}: {}",
                                position.id,
                                e
                            );
                        } else {
                            tracing::info!(
                                "Exchange verification: marking stale pending {} ({}) as Cancelled — entry not found and no position on exchange",
                                position.symbol,
                                position.id
                            );
                        }
                        if let Err(e) = self.repository.mark_closed(position.id).await {
                            tracing::error!(
                                "Exchange verification: failed to mark closed in DB for {}: {}",
                                position.id,
                                e
                            );
                            errors += 1;
                        }
                    }

                    stale_detected += 1;
                    verified += 1;
                    continue;
                }
            }

            // Check if our tracked SL/TP orders still exist on the exchange
            let sl_alive = position
                .exchange_order_ids
                .stop_loss_order_id
                .as_ref()
                .map_or(false, |id| open_ids.contains(id));
            let tp_alive = position
                .exchange_order_ids
                .take_profit_order_id
                .as_ref()
                .map_or(false, |id| open_ids.contains(id));

            if !sl_alive && position.exchange_order_ids.stop_loss_order_id.is_some()
                || !tp_alive && position.exchange_order_ids.take_profit_order_id.is_some()
            {
                // SL/TP order IDs missing from open orders.  Before concluding they
                // filled, verify that the exchange position is actually closed.
                // Order IDs can change via edits/replacements on some exchanges.
                let still_has_position = match cex_client
                    .fetch_positions(
                        &creds.exchange_name,
                        &sidecar_creds,
                        sandbox,
                        Some(&cex_symbol),
                    )
                    .await
                {
                    Ok(positions) => positions.iter().any(|p| {
                        p.contracts
                            .parse::<f64>()
                            .map_or(false, |c| c.abs() > 0.0)
                    }),
                    Err(e) => {
                        tracing::warn!(
                            "Exchange verification: fetchPositions failed for {} ({}), skipping: {}",
                            position.symbol,
                            position.id,
                            e
                        );
                        errors += 1;
                        verified += 1;
                        continue;
                    }
                };

                if still_has_position {
                    // Position still open — SL/TP orders were likely edited/replaced.
                    // Don't transition to terminal state; just log and move on.
                    tracing::info!(
                        "Exchange verification: SL/TP order IDs missing but position still open for {} ({}) — orders may have been replaced",
                        position.symbol,
                        position.id
                    );
                } else {
                    // Position genuinely closed — mark as terminal via EngineHandle
                    if !sl_alive && position.exchange_order_ids.stop_loss_order_id.is_some() {
                        if let Err(e) = self.engine_handle.on_stop_loss_filled(position.id).await {
                            tracing::error!(
                                "Exchange verification: failed to mark SL filled for {}: {}",
                                position.id,
                                e
                            );
                        } else {
                            tracing::info!(
                                "Exchange verification: SL filled during downtime for {} ({})",
                                position.symbol,
                                position.id
                            );
                        }
                    } else if !tp_alive
                        && position.exchange_order_ids.take_profit_order_id.is_some()
                    {
                        if let Err(e) = self
                            .engine_handle
                            .update_group_status(position.id, OrderGroupStatus::TookProfit)
                            .await
                        {
                            tracing::error!(
                                "Exchange verification: failed to mark TP filled for {}: {}",
                                position.id,
                                e
                            );
                        } else {
                            tracing::info!(
                                "Exchange verification: TP filled during downtime for {} ({})",
                                position.symbol,
                                position.id
                            );
                        }
                    }

                    stale_detected += 1;
                }
            }

            verified += 1;
        }

        tracing::info!(
            "Exchange verification: {} positions verified, {} stale detected, {} errors",
            verified,
            stale_detected,
            errors
        );

        VerificationSummary {
            verified,
            stale_detected,
            errors,
        }
    }

    pub async fn collect_live_subscription_tuples(
        &self,
    ) -> Result<Vec<LiveSubscriptionTuple>, String> {
        let positions = self
            .repository
            .load_active()
            .await
            .map_err(|e| format!("Failed to load positions from DB: {}", e))?;

        Ok(build_live_subscription_tuples(positions))
    }
}

fn build_live_subscription_tuples(positions: Vec<ManagedPosition>) -> Vec<LiveSubscriptionTuple> {
    let mut seen = HashSet::new();
    let mut tuples = Vec::new();

    for position in positions {
        let Some(exchange_account_id) = position.exchange_account_id else {
            continue;
        };

        let key = (
            position.user_id,
            exchange_account_id,
            position.symbol.clone(),
        );

        if seen.insert(key.clone()) {
            tuples.push(LiveSubscriptionTuple {
                user_id: key.0,
                exchange_account_id: key.1,
                symbol: key.2,
            });
        }
    }

    tuples
}

#[derive(Debug, Clone)]
pub struct LiveSubscriptionTuple {
    pub user_id: Uuid,
    pub exchange_account_id: Uuid,
    pub symbol: String,
}

/// Summary of exchange verification results.
#[derive(Debug)]
pub struct VerificationSummary {
    pub verified: usize,
    pub stale_detected: usize,
    pub errors: usize,
}

/// Build an OrderGroup from a persisted ManagedPosition.
fn build_order_group(position: &ManagedPosition) -> OrderGroup {
    let now = chrono::Utc::now();

    let status = match position.state {
        PositionState::Pending => OrderGroupStatus::Pending,
        PositionState::Filled | PositionState::Managing => OrderGroupStatus::Active,
        PositionState::Closed => OrderGroupStatus::Closed,
    };

    // Use position.id as group.id for correlation
    let group_id = position.id;
    // Synthetic entry order ID — no real shadow order exists for live trades
    let entry_order_id = Uuid::new_v4();

    let break_even_config = if position.rules.break_even_at > 0 {
        Some(BreakEvenConfig {
            trigger_percent: Decimal::from(position.rules.break_even_at),
            offset: None,
            triggered: position.be_triggered,
        })
    } else {
        None
    };

    let take_profit_targets = vec![TakeProfitTarget {
        price: position.target_price,
        percent_to_close: Decimal::from(100),
        order_id: None,
        filled: false,
    }];

    OrderGroup {
        id: group_id,
        user_id: position.user_id,
        symbol: position.symbol.clone(),
        entry_order_id,
        entry_price: Some(position.entry_price),
        entry_quantity: position.quantity,
        stop_loss_order_id: None,
        stop_loss_price: Some(position.current_stop),
        take_profit_order_ids: Vec::new(),
        take_profit_targets,
        status,
        break_even_config,
        exchange_order_id: position.exchange_order_ids.entry_order_id.clone(),
        exchange_sl_order_id: position.exchange_order_ids.stop_loss_order_id.clone(),
        exchange_tp_order_id: position.exchange_order_ids.take_profit_order_id.clone(),
        exchange_account_id: position.exchange_account_id,
        created_at: position.created_at,
        updated_at: now,
        completed_at: None,
        exchange_name: None, // Set post-build from exchange_accounts lookup
        risk_amount: None, // Not available from rehydrated positions
        setup_tag: position.setup_tag.clone(),
        // QNT-01a: kelly_inputs snapshot is not persisted on managed_positions
        // today. Rehydrated groups carry None — at close, the trade will
        // record as fixed-mode (NULL kelly_inputs). This is acceptable for
        // MVP since router restarts are rare; if preservation becomes
        // important, add a `kelly_inputs` column to managed_positions.
        kelly_inputs: None,
    }
}

/// Summary of rehydration results for logging.
#[derive(Debug)]
pub struct RehydrationSummary {
    pub positions_loaded: usize,
    pub exchange_ids_registered: usize,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::services::trade_manager::types::*;
    use chrono::Utc;
    use rust_decimal_macros::dec;

    #[test]
    fn test_build_order_group_active_position() {
        let account_id = Uuid::new_v4();
        let position = ManagedPosition {
            id: Uuid::new_v4(),
            user_id: Uuid::new_v4(),
            symbol: "BTC_USDT".to_string(),
            side: PositionSide::Long,
            entry_price: dec!(50000),
            stop_price: dec!(49000),
            target_price: dec!(52000),
            quantity: dec!(0.2),
            rules: ManagementRules {
                risk_percent: dec!(2),
                break_even_at: 50,
                leverage: 10,
                trailing_stop: None,
                partial_tp: None,
            },
            state: PositionState::Filled,
            be_triggered: false,
            partial_tp_fired: false,
            current_stop: dec!(49000),
            remaining_qty: dec!(0.2),
            exchange_order_ids: ExchangeOrderIds {
                entry_order_id: Some("entry-123".to_string()),
                stop_loss_order_id: Some("sl-456".to_string()),
                take_profit_order_id: Some("tp-789".to_string()),
            },
            created_at: Utc::now(),
            exchange_account_id: Some(account_id),
            setup_tag: None,
        };

        let group = build_order_group(&position);

        assert_eq!(group.id, position.id);
        assert_eq!(group.user_id, position.user_id);
        assert_eq!(group.symbol, "BTC_USDT");
        assert_eq!(group.entry_price, Some(dec!(50000)));
        assert_eq!(group.entry_quantity, dec!(0.2));
        assert_eq!(group.stop_loss_price, Some(dec!(49000)));
        assert_eq!(group.status, OrderGroupStatus::Active);
        assert_eq!(group.exchange_order_id, Some("entry-123".to_string()));
        assert_eq!(group.exchange_sl_order_id, Some("sl-456".to_string()));
        assert_eq!(group.exchange_tp_order_id, Some("tp-789".to_string()));
        assert_eq!(group.exchange_account_id, Some(account_id));
        assert_eq!(group.take_profit_targets.len(), 1);
        assert_eq!(group.take_profit_targets[0].price, dec!(52000));
    }

    #[test]
    fn test_build_order_group_with_be_triggered() {
        let position = ManagedPosition {
            id: Uuid::new_v4(),
            user_id: Uuid::new_v4(),
            symbol: "ETH_USDT".to_string(),
            side: PositionSide::Short,
            entry_price: dec!(3000),
            stop_price: dec!(3100),
            target_price: dec!(2800),
            quantity: dec!(1),
            rules: ManagementRules {
                risk_percent: dec!(1),
                break_even_at: 30,
                leverage: 5,
                trailing_stop: Some(TrailingStopRule {
                    enabled: true,
                    distance_percent: 20,
                }),
                partial_tp: None,
            },
            state: PositionState::Managing,
            be_triggered: true,
            partial_tp_fired: false,
            current_stop: dec!(3000), // Moved to entry
            remaining_qty: dec!(1),
            exchange_order_ids: ExchangeOrderIds::default(),
            created_at: Utc::now(),
            exchange_account_id: None,
            setup_tag: None,
        };

        let group = build_order_group(&position);

        assert_eq!(group.status, OrderGroupStatus::Active);
        assert_eq!(group.stop_loss_price, Some(dec!(3000))); // current_stop, not original
        assert!(group.break_even_config.is_some());
        let be_config = group.break_even_config.unwrap();
        assert_eq!(be_config.trigger_percent, Decimal::from(30));
        assert!(be_config.triggered);
    }

    #[test]
    fn test_build_order_group_pending_state() {
        let position = ManagedPosition {
            id: Uuid::new_v4(),
            user_id: Uuid::new_v4(),
            symbol: "SOL_USDT".to_string(),
            side: PositionSide::Long,
            entry_price: dec!(100),
            stop_price: dec!(95),
            target_price: dec!(120),
            quantity: dec!(10),
            rules: ManagementRules {
                risk_percent: dec!(2),
                break_even_at: 0,
                leverage: 1,
                trailing_stop: None,
                partial_tp: None,
            },
            state: PositionState::Pending,
            be_triggered: false,
            partial_tp_fired: false,
            current_stop: dec!(95),
            remaining_qty: dec!(10),
            exchange_order_ids: ExchangeOrderIds::default(),
            created_at: Utc::now(),
            exchange_account_id: None,
            setup_tag: None,
        };

        let group = build_order_group(&position);

        assert_eq!(group.status, OrderGroupStatus::Pending);
        assert!(group.break_even_config.is_none()); // break_even_at = 0
    }

    #[test]
    fn test_build_order_group_closed_state() {
        let position = ManagedPosition {
            id: Uuid::new_v4(),
            user_id: Uuid::new_v4(),
            symbol: "BTC_USDT".to_string(),
            side: PositionSide::Long,
            entry_price: dec!(50000),
            stop_price: dec!(49000),
            target_price: dec!(52000),
            quantity: dec!(0.1),
            rules: ManagementRules {
                risk_percent: dec!(1),
                break_even_at: 50,
                leverage: 1,
                trailing_stop: None,
                partial_tp: None,
            },
            state: PositionState::Closed,
            be_triggered: true,
            partial_tp_fired: false,
            current_stop: dec!(50000),
            remaining_qty: dec!(0.1),
            exchange_order_ids: ExchangeOrderIds::default(),
            created_at: Utc::now(),
            exchange_account_id: None,
            setup_tag: None,
        };

        let group = build_order_group(&position);
        assert_eq!(group.status, OrderGroupStatus::Closed);
    }

    /// FR-1 (016): Verify rehydration uses CEX symbol format.
    /// The to_cex_symbol conversion must be applied before any CEX sidecar calls.
    #[test]
    fn test_rehydration_symbol_format_conversion() {
        use crate::services::exchange_api::to_cex_symbol;

        // Internal format -> CEX format (strip underscore)
        assert_eq!(to_cex_symbol("BTC_USDT"), "BTCUSDT");
        assert_eq!(to_cex_symbol("ETH_USDT"), "ETHUSDT");
        assert_eq!(to_cex_symbol("SOL_USDT"), "SOLUSDT");

        // Passthrough for already-formatted or invalid symbols
        assert_eq!(to_cex_symbol("INVALID"), "INVALID");
        assert_eq!(to_cex_symbol("BTCUSDT"), "BTCUSDT"); // already in CEX format
    }

    #[test]
    fn test_build_live_subscription_tuples_dedupes_symbol_account_pairs() {
        let user_id = Uuid::new_v4();
        let account_id = Uuid::new_v4();

        let mut a = ManagedPosition::new(
            user_id,
            "BTC_USDT".to_string(),
            PositionSide::Long,
            dec!(50000),
            dec!(49000),
            dec!(52000),
            dec!(0.1),
            ManagementRules {
                risk_percent: dec!(1),
                break_even_at: 0,
                leverage: 1,
                trailing_stop: None,
                partial_tp: None,
            },
        );
        a.exchange_account_id = Some(account_id);

        let mut b = a.clone();
        b.id = Uuid::new_v4();

        let mut c = a.clone();
        c.id = Uuid::new_v4();
        c.symbol = "ETH_USDT".to_string();

        let tuples = build_live_subscription_tuples(vec![a, b, c]);
        assert_eq!(tuples.len(), 2);
        assert!(tuples.iter().any(|t| t.symbol == "BTC_USDT"));
        assert!(tuples.iter().any(|t| t.symbol == "ETH_USDT"));
    }
}
