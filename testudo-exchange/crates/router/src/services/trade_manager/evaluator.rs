//! Rule Evaluator
//!
//! Pure logic module that evaluates management rules against current price.
//! No I/O, no side effects - just computes which actions should be taken.

use rust_decimal::Decimal;
use rust_decimal_macros::dec;

use super::types::{ManagedPosition, ManagementAction, PositionSide, PositionState};

/// Calculate progress toward target as a ratio (0.0 to 1.0+).
///
/// For long: (current - entry) / (target - entry)
/// For short: (entry - current) / (entry - target)
fn progress(position: &ManagedPosition, current_price: Decimal) -> Decimal {
    let range = match position.side {
        PositionSide::Long => position.target_price - position.entry_price,
        PositionSide::Short => position.entry_price - position.target_price,
    };

    if range <= Decimal::ZERO {
        return Decimal::ZERO;
    }

    let movement = match position.side {
        PositionSide::Long => current_price - position.entry_price,
        PositionSide::Short => position.entry_price - current_price,
    };

    movement / range
}

/// Evaluate management rules for a position at the current price.
///
/// Returns a list of actions to execute. Pure function - no I/O.
///
/// Rules evaluated in order:
/// 1. Break-even: if progress >= break_even_at/100 and not yet triggered
/// 2. Trailing stop: if BE triggered and trailing enabled, move stop in profitable direction
/// 3. Partial TP: if progress >= 1.0 and not yet fired, close partial_tp.close_percent
pub fn evaluate(position: &ManagedPosition, current_price: Decimal) -> Vec<ManagementAction> {
    // Only evaluate positions that are being managed
    if position.state != PositionState::Filled && position.state != PositionState::Managing {
        return Vec::new();
    }

    let mut actions = Vec::new();
    let prog = progress(position, current_price);

    // 1. Break-even check
    if !position.be_triggered {
        let be_threshold = Decimal::from(position.rules.break_even_at) / dec!(100);
        if prog >= be_threshold {
            actions.push(ManagementAction::MoveStopToEntry);
        }
    }

    // 2. Trailing stop check (only after BE triggered)
    if position.be_triggered {
        if let Some(ref trailing) = position.rules.trailing_stop {
            if trailing.enabled {
                let range = match position.side {
                    PositionSide::Long => position.target_price - position.entry_price,
                    PositionSide::Short => position.entry_price - position.target_price,
                };
                let trail_dist = range * Decimal::from(trailing.distance_percent) / dec!(100);

                let new_stop = match position.side {
                    PositionSide::Long => current_price - trail_dist,
                    PositionSide::Short => current_price + trail_dist,
                };

                // Only move stop in profitable direction
                let should_move = match position.side {
                    PositionSide::Long => new_stop > position.current_stop,
                    PositionSide::Short => new_stop < position.current_stop,
                };

                if should_move {
                    actions.push(ManagementAction::AdjustTrailingStop {
                        new_price: new_stop,
                    });
                }
            }
        }
    }

    // 3. Partial TP check
    if !position.partial_tp_fired {
        if let Some(ref partial_tp) = position.rules.partial_tp {
            if partial_tp.enabled && prog >= dec!(1) {
                let close_qty =
                    position.remaining_qty * Decimal::from(partial_tp.close_percent) / dec!(100);
                if close_qty > Decimal::ZERO {
                    actions.push(ManagementAction::PartialClose {
                        quantity: close_qty,
                    });
                }
            }
        }
    }

    actions
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::services::trade_manager::types::*;
    use rust_decimal_macros::dec;
    use uuid::Uuid;

    fn long_position(
        be_at: u32,
        trailing: Option<TrailingStopRule>,
        partial: Option<PartialTpRule>,
    ) -> ManagedPosition {
        let mut pos = ManagedPosition::new(
            Uuid::new_v4(),
            "BTC_USDT".to_string(),
            PositionSide::Long,
            dec!(50000), // entry
            dec!(49000), // stop
            dec!(52000), // target
            dec!(0.2),   // qty
            ManagementRules {
                risk_percent: dec!(2),
                break_even_at: be_at,
                leverage: 1,
                trailing_stop: trailing,
                partial_tp: partial,
            },
        );
        pos.state = PositionState::Filled;
        pos
    }

    fn short_position(
        be_at: u32,
        trailing: Option<TrailingStopRule>,
        partial: Option<PartialTpRule>,
    ) -> ManagedPosition {
        let mut pos = ManagedPosition::new(
            Uuid::new_v4(),
            "BTC_USDT".to_string(),
            PositionSide::Short,
            dec!(50000), // entry
            dec!(51000), // stop
            dec!(48000), // target
            dec!(0.2),   // qty
            ManagementRules {
                risk_percent: dec!(2),
                break_even_at: be_at,
                leverage: 1,
                trailing_stop: trailing,
                partial_tp: partial,
            },
        );
        pos.state = PositionState::Filled;
        pos
    }

    // ==================== Break-Even Tests ====================

    #[test]
    fn test_long_be_triggers_at_50_percent() {
        // Entry=50000, target=52000, range=2000
        // 50% of 2000 = 1000, so BE triggers at 51000
        let pos = long_position(50, None, None);

        // At 50999 - not yet
        let actions = evaluate(&pos, dec!(50999));
        assert!(actions.is_empty());

        // At 51000 - triggers
        let actions = evaluate(&pos, dec!(51000));
        assert_eq!(actions.len(), 1);
        assert_eq!(actions[0], ManagementAction::MoveStopToEntry);
    }

    #[test]
    fn test_long_be_does_not_retrigger() {
        let mut pos = long_position(50, None, None);
        pos.be_triggered = true;

        let actions = evaluate(&pos, dec!(51500));
        // No MoveStopToEntry action since already triggered
        assert!(!actions
            .iter()
            .any(|a| *a == ManagementAction::MoveStopToEntry));
    }

    #[test]
    fn test_short_be_triggers_correctly() {
        // Short: entry=50000, target=48000, range=2000
        // 50% of 2000 = 1000, so BE triggers when price drops to 49000
        let pos = short_position(50, None, None);

        // At 49001 - not yet (price still too high)
        let actions = evaluate(&pos, dec!(49001));
        assert!(actions.is_empty());

        // At 49000 - triggers (progress = (50000-49000)/(50000-48000) = 0.5)
        let actions = evaluate(&pos, dec!(49000));
        assert_eq!(actions.len(), 1);
        assert_eq!(actions[0], ManagementAction::MoveStopToEntry);
    }

    // ==================== Trailing Stop Tests ====================

    #[test]
    fn test_long_trailing_stop_moves_up() {
        // Entry=50000, target=52000, range=2000
        // Trailing distance = 20% of 2000 = 400
        let mut pos = long_position(
            50,
            Some(TrailingStopRule {
                enabled: true,
                distance_percent: 20,
            }),
            None,
        );
        pos.be_triggered = true;
        pos.current_stop = dec!(50000); // After BE, stop at entry

        // Price at 51500 -> new stop = 51500 - 400 = 51100
        let actions = evaluate(&pos, dec!(51500));
        assert_eq!(actions.len(), 1);
        assert_eq!(
            actions[0],
            ManagementAction::AdjustTrailingStop {
                new_price: dec!(51100)
            }
        );
    }

    #[test]
    fn test_long_trailing_stop_never_moves_down() {
        let mut pos = long_position(
            50,
            Some(TrailingStopRule {
                enabled: true,
                distance_percent: 20,
            }),
            None,
        );
        pos.be_triggered = true;
        pos.current_stop = dec!(51100); // Already trailed up

        // Price drops to 51200 -> new stop would be 51200 - 400 = 50800 (< current 51100)
        let actions = evaluate(&pos, dec!(51200));
        // No trailing stop adjustment (would move down)
        assert!(!actions
            .iter()
            .any(|a| matches!(a, ManagementAction::AdjustTrailingStop { .. })));
    }

    #[test]
    fn test_short_trailing_stop_moves_down() {
        // Short: entry=50000, target=48000, range=2000
        // Trailing distance = 20% of 2000 = 400
        let mut pos = short_position(
            50,
            Some(TrailingStopRule {
                enabled: true,
                distance_percent: 20,
            }),
            None,
        );
        pos.be_triggered = true;
        pos.current_stop = dec!(50000); // After BE, stop at entry

        // Price at 48500 -> new stop = 48500 + 400 = 48900 (below current 50000)
        let actions = evaluate(&pos, dec!(48500));
        assert_eq!(actions.len(), 1);
        assert_eq!(
            actions[0],
            ManagementAction::AdjustTrailingStop {
                new_price: dec!(48900)
            }
        );
    }

    #[test]
    fn test_short_trailing_stop_never_moves_up() {
        let mut pos = short_position(
            50,
            Some(TrailingStopRule {
                enabled: true,
                distance_percent: 20,
            }),
            None,
        );
        pos.be_triggered = true;
        pos.current_stop = dec!(48900); // Already trailed down

        // Price bounces to 49000 -> new stop = 49000 + 400 = 49400 (> current 48900)
        let actions = evaluate(&pos, dec!(49000));
        // No trailing stop adjustment (would move up for short)
        assert!(!actions
            .iter()
            .any(|a| matches!(a, ManagementAction::AdjustTrailingStop { .. })));
    }

    // ==================== Partial TP Tests ====================

    #[test]
    fn test_partial_tp_fires_at_target() {
        // Long: entry=50000, target=52000
        // Progress = 1.0 at target price
        let pos = long_position(
            50,
            None,
            Some(PartialTpRule {
                enabled: true,
                close_percent: 50,
            }),
        );

        // Below target - no partial TP
        let actions = evaluate(&pos, dec!(51999));
        assert!(!actions
            .iter()
            .any(|a| matches!(a, ManagementAction::PartialClose { .. })));

        // At target
        let actions = evaluate(&pos, dec!(52000));
        // BE should also trigger (progress=1.0 >= 0.5)
        let partial = actions
            .iter()
            .find(|a| matches!(a, ManagementAction::PartialClose { .. }));
        assert!(partial.is_some());
        if let ManagementAction::PartialClose { quantity } = partial.unwrap() {
            assert_eq!(*quantity, dec!(0.1)); // 50% of 0.2
        }
    }

    #[test]
    fn test_partial_tp_fires_only_once() {
        let mut pos = long_position(
            50,
            None,
            Some(PartialTpRule {
                enabled: true,
                close_percent: 50,
            }),
        );
        pos.partial_tp_fired = true;

        let actions = evaluate(&pos, dec!(53000));
        assert!(!actions
            .iter()
            .any(|a| matches!(a, ManagementAction::PartialClose { .. })));
    }

    // ==================== Edge Cases ====================

    #[test]
    fn test_no_actions_when_price_unchanged() {
        let pos = long_position(50, None, None);

        // Price at entry - 0% progress
        let actions = evaluate(&pos, dec!(50000));
        assert!(actions.is_empty());
    }

    #[test]
    fn test_no_actions_for_pending_position() {
        let pos = long_position(50, None, None);
        // State is Pending by default in constructor, we set Filled above
        let mut pending = pos;
        pending.state = PositionState::Pending;

        let actions = evaluate(&pending, dec!(51500));
        assert!(actions.is_empty());
    }

    #[test]
    fn test_no_actions_for_closed_position() {
        let mut pos = long_position(50, None, None);
        pos.state = PositionState::Closed;

        let actions = evaluate(&pos, dec!(51500));
        assert!(actions.is_empty());
    }

    // ==================== Combined Rules ====================

    #[test]
    fn test_all_rules_combined_long() {
        // Long: entry=50000, stop=49000, target=52000
        // BE at 50%, trailing 20%, partial TP 50%
        let mut pos = long_position(
            50,
            Some(TrailingStopRule {
                enabled: true,
                distance_percent: 20,
            }),
            Some(PartialTpRule {
                enabled: true,
                close_percent: 50,
            }),
        );

        // Step 1: Price at 50500 (25% progress) - no actions
        let actions = evaluate(&pos, dec!(50500));
        assert!(actions.is_empty());

        // Step 2: Price at 51000 (50% progress) - BE triggers
        let actions = evaluate(&pos, dec!(51000));
        assert_eq!(actions.len(), 1);
        assert_eq!(actions[0], ManagementAction::MoveStopToEntry);

        // Apply BE
        pos.be_triggered = true;
        pos.current_stop = dec!(50000);

        // Step 3: Price at 51500 (75%) - trailing stop moves
        // trail_dist = 2000 * 20% = 400
        // new_stop = 51500 - 400 = 51100
        let actions = evaluate(&pos, dec!(51500));
        assert_eq!(actions.len(), 1);
        assert_eq!(
            actions[0],
            ManagementAction::AdjustTrailingStop {
                new_price: dec!(51100)
            }
        );

        // Apply trailing
        pos.current_stop = dec!(51100);

        // Step 4: Price at 52000 (100%) - trailing moves + partial TP fires
        // new_stop = 52000 - 400 = 51600 (> 51100)
        let actions = evaluate(&pos, dec!(52000));
        assert_eq!(actions.len(), 2);
        assert!(actions
            .iter()
            .any(|a| matches!(a, ManagementAction::AdjustTrailingStop { .. })));
        assert!(actions
            .iter()
            .any(|a| matches!(a, ManagementAction::PartialClose { .. })));
    }

    #[test]
    fn test_all_rules_combined_short() {
        // Short: entry=50000, stop=51000, target=48000
        // BE at 50%, trailing 20%, partial TP 50%
        let mut pos = short_position(
            50,
            Some(TrailingStopRule {
                enabled: true,
                distance_percent: 20,
            }),
            Some(PartialTpRule {
                enabled: true,
                close_percent: 50,
            }),
        );

        // Step 1: Price at 49500 (25% progress) - no actions
        let actions = evaluate(&pos, dec!(49500));
        assert!(actions.is_empty());

        // Step 2: Price at 49000 (50% progress) - BE triggers
        let actions = evaluate(&pos, dec!(49000));
        assert_eq!(actions.len(), 1);
        assert_eq!(actions[0], ManagementAction::MoveStopToEntry);

        // Apply BE
        pos.be_triggered = true;
        pos.current_stop = dec!(50000);

        // Step 3: Price at 48500 (75%) - trailing moves
        // trail_dist = 2000 * 20% = 400
        // new_stop = 48500 + 400 = 48900 (< 50000)
        let actions = evaluate(&pos, dec!(48500));
        assert_eq!(actions.len(), 1);
        assert_eq!(
            actions[0],
            ManagementAction::AdjustTrailingStop {
                new_price: dec!(48900)
            }
        );

        // Apply trailing
        pos.current_stop = dec!(48900);

        // Step 4: Price at 48000 (100%) - trailing + partial TP
        // new_stop = 48000 + 400 = 48400 (< 48900)
        let actions = evaluate(&pos, dec!(48000));
        assert_eq!(actions.len(), 2);
    }

    #[test]
    fn test_progress_calculation() {
        let pos = long_position(50, None, None);
        // Entry=50000, target=52000, range=2000

        assert_eq!(progress(&pos, dec!(50000)), dec!(0)); // at entry
        assert_eq!(progress(&pos, dec!(51000)), dec!(0.5)); // 50%
        assert_eq!(progress(&pos, dec!(52000)), dec!(1)); // at target
        assert_eq!(progress(&pos, dec!(53000)), dec!(1.5)); // beyond target
    }

    #[test]
    fn test_progress_calculation_short() {
        let pos = short_position(50, None, None);
        // Entry=50000, target=48000, range=2000

        assert_eq!(progress(&pos, dec!(50000)), dec!(0)); // at entry
        assert_eq!(progress(&pos, dec!(49000)), dec!(0.5)); // 50%
        assert_eq!(progress(&pos, dec!(48000)), dec!(1)); // at target
        assert_eq!(progress(&pos, dec!(47000)), dec!(1.5)); // beyond target
    }
}
