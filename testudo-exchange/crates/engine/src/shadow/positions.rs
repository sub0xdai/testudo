//! Shadow Position Tracking
//!
//! Tracks open positions from filled orders and calculates P&L
//! using mark price from live market data.
//!
//! # P&L Calculation (from PRD)
//!
//! - Unrealized P&L = (Mark Price - Entry Price) * Size for longs
//! - Unrealized P&L = (Entry Price - Mark Price) * Size for shorts
//! - Uses Mark Price (not bid/ask) to avoid P&L noise from spread flickering

use chrono::{DateTime, Utc};
use rust_decimal::Decimal;
use rust_decimal_macros::dec;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::time::Instant;
use uuid::Uuid;

use super::orders::{ShadowOrder, ShadowOrderSide};

/// Position side (long or short)
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum PositionSide {
    Long,
    Short,
}

impl From<ShadowOrderSide> for PositionSide {
    fn from(side: ShadowOrderSide) -> Self {
        match side {
            ShadowOrderSide::Buy => PositionSide::Long,
            ShadowOrderSide::Sell => PositionSide::Short,
        }
    }
}

/// A shadow position representing an open trade
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ShadowPosition {
    pub id: Uuid,
    pub user_id: Uuid,
    pub symbol: String,
    pub side: PositionSide,
    pub size: Decimal,
    pub entry_price: Decimal,
    pub mark_price: Decimal,
    pub unrealized_pnl: Decimal,
    pub realized_pnl: Decimal,
    pub opened_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,

    // For tracking the source orders
    pub entry_order_id: Uuid,
    pub exit_order_ids: Vec<Uuid>,

    /// AUD-02: Timestamp when position was closed (for GC).
    #[serde(skip)]
    pub completed_at: Option<Instant>,
}

impl ShadowPosition {
    /// Create a new position from a filled order
    pub fn from_fill(order: &ShadowOrder, fill_price: Decimal) -> Self {
        let side = match order.side {
            ShadowOrderSide::Buy => PositionSide::Long,
            ShadowOrderSide::Sell => PositionSide::Short,
        };

        let now = Utc::now();
        Self {
            id: Uuid::new_v4(),
            user_id: order.user_id,
            symbol: order.symbol.clone(),
            side,
            size: order.quantity,
            entry_price: fill_price,
            mark_price: fill_price,
            unrealized_pnl: dec!(0),
            realized_pnl: dec!(0),
            opened_at: now,
            updated_at: now,
            entry_order_id: order.id,
            exit_order_ids: Vec::new(),
            completed_at: None,
        }
    }

    /// Update mark price and recalculate unrealized P&L
    pub fn update_mark_price(&mut self, mark_price: Decimal) {
        self.mark_price = mark_price;
        self.unrealized_pnl = self.calculate_unrealized_pnl();
        self.updated_at = Utc::now();
    }

    /// Calculate unrealized P&L based on current mark price
    fn calculate_unrealized_pnl(&self) -> Decimal {
        match self.side {
            PositionSide::Long => (self.mark_price - self.entry_price) * self.size,
            PositionSide::Short => (self.entry_price - self.mark_price) * self.size,
        }
    }

    /// Calculate unrealized P&L percentage
    pub fn unrealized_pnl_percent(&self) -> Decimal {
        let cost = self.entry_price * self.size;
        if cost == dec!(0) {
            return dec!(0);
        }
        (self.unrealized_pnl / cost) * dec!(100)
    }

    /// Close part of the position
    pub fn reduce_size(&mut self, amount: Decimal, exit_price: Decimal, exit_order_id: Uuid) {
        let closed_pnl = match self.side {
            PositionSide::Long => (exit_price - self.entry_price) * amount,
            PositionSide::Short => (self.entry_price - exit_price) * amount,
        };

        self.realized_pnl += closed_pnl;
        self.size -= amount;
        self.exit_order_ids.push(exit_order_id);
        self.updated_at = Utc::now();

        // Recalculate unrealized P&L for remaining size
        self.unrealized_pnl = self.calculate_unrealized_pnl();
    }

    /// Check if position is fully closed
    pub fn is_closed(&self) -> bool {
        self.size <= dec!(0)
    }

    /// Get total P&L (realized + unrealized)
    pub fn total_pnl(&self) -> Decimal {
        self.realized_pnl + self.unrealized_pnl
    }
}

/// Manages all shadow positions
pub struct ShadowPositionManager {
    /// Positions by ID
    positions: HashMap<Uuid, ShadowPosition>,
    /// Index of positions by user
    positions_by_user: HashMap<Uuid, Vec<Uuid>>,
    /// Index of open positions by symbol (for price updates)
    open_positions_by_symbol: HashMap<String, Vec<Uuid>>,
    /// Current mark prices by symbol
    mark_prices: HashMap<String, Decimal>,
}

impl Default for ShadowPositionManager {
    fn default() -> Self {
        Self::new()
    }
}

impl ShadowPositionManager {
    pub fn new() -> Self {
        Self {
            positions: HashMap::new(),
            positions_by_user: HashMap::new(),
            open_positions_by_symbol: HashMap::new(),
            mark_prices: HashMap::new(),
        }
    }

    /// Get all open positions for a user
    pub fn get_positions(&self, user_id: Uuid) -> Vec<ShadowPosition> {
        self.positions_by_user
            .get(&user_id)
            .map(|position_ids| {
                position_ids
                    .iter()
                    .filter_map(|id| self.positions.get(id))
                    .filter(|p| !p.is_closed())
                    .cloned()
                    .collect()
            })
            .unwrap_or_default()
    }

    /// Get a specific position
    pub fn get_position(&self, position_id: Uuid) -> Option<&ShadowPosition> {
        self.positions.get(&position_id)
    }

    /// Update or create a position from a filled order
    pub fn update_from_fill(&mut self, order: &ShadowOrder, fill_price: Decimal) {
        let user_id = order.user_id;
        let symbol = &order.symbol;

        // Check if user has an existing position in this symbol
        let existing_position = self.find_open_position(user_id, symbol);

        match existing_position {
            Some(position_id) => {
                // Update existing position
                if let Some(position) = self.positions.get_mut(&position_id) {
                    let order_side: PositionSide = order.side.into();

                    if order_side == position.side {
                        // Adding to position - calculate new average entry
                        let total_cost =
                            (position.entry_price * position.size) + (fill_price * order.quantity);
                        let new_size = position.size + order.quantity;
                        position.entry_price = total_cost / new_size;
                        position.size = new_size;
                        position.updated_at = Utc::now();
                    } else {
                        // Reducing/closing position
                        position.reduce_size(order.quantity, fill_price, order.id);

                        if position.is_closed() {
                            position.completed_at = Some(Instant::now());
                            // Remove from open positions index
                            if let Some(ids) = self.open_positions_by_symbol.get_mut(symbol) {
                                ids.retain(|id| *id != position_id);
                            }
                        }
                    }
                }
            }
            None => {
                // Create new position
                let position = ShadowPosition::from_fill(order, fill_price);
                let position_id = position.id;
                let symbol = position.symbol.clone();

                self.positions.insert(position_id, position);

                self.positions_by_user
                    .entry(user_id)
                    .or_default()
                    .push(position_id);

                self.open_positions_by_symbol
                    .entry(symbol)
                    .or_default()
                    .push(position_id);
            }
        }
    }

    /// Get the entry price of an open position for a user in a symbol
    pub fn get_entry_price(&self, user_id: Uuid, symbol: &str) -> Option<(Decimal, PositionSide)> {
        self.find_open_position(user_id, symbol)
            .and_then(|pid| self.positions.get(&pid))
            .map(|p| (p.entry_price, p.side))
    }

    /// Find an open position for a user in a symbol
    fn find_open_position(&self, user_id: Uuid, symbol: &str) -> Option<Uuid> {
        self.positions_by_user
            .get(&user_id)
            .and_then(|position_ids| {
                position_ids
                    .iter()
                    .find(|id| {
                        self.positions
                            .get(*id)
                            .map(|p| p.symbol == symbol && !p.is_closed())
                            .unwrap_or(false)
                    })
                    .copied()
            })
    }

    /// Update mark price for a symbol
    pub fn update_mark_price(&mut self, symbol: &str, mark_price: Decimal) {
        self.mark_prices.insert(symbol.to_string(), mark_price);

        // Update all open positions in this symbol
        if let Some(position_ids) = self.open_positions_by_symbol.get(symbol) {
            for position_id in position_ids.clone() {
                if let Some(position) = self.positions.get_mut(&position_id) {
                    position.update_mark_price(mark_price);
                }
            }
        }
    }

    /// Get total unrealized P&L for a user across all positions
    pub fn get_total_unrealized_pnl(&self, user_id: Uuid) -> Decimal {
        self.get_positions(user_id)
            .iter()
            .map(|p| p.unrealized_pnl)
            .sum()
    }

    /// Get total realized P&L for a user across all positions
    pub fn get_total_realized_pnl(&self, user_id: Uuid) -> Decimal {
        self.positions_by_user
            .get(&user_id)
            .map(|position_ids| {
                position_ids
                    .iter()
                    .filter_map(|id| self.positions.get(id))
                    .map(|p| p.realized_pnl)
                    .sum()
            })
            .unwrap_or(dec!(0))
    }

    /// Get position count
    pub fn open_position_count(&self, user_id: Uuid) -> usize {
        self.get_positions(user_id).len()
    }

    /// AUD-02 FR-2: Remove closed positions older than cutoff.
    pub fn prune_terminal(&mut self, cutoff: Instant) -> usize {
        let to_remove: Vec<Uuid> = self
            .positions
            .iter()
            .filter(|(_, p)| {
                p.is_closed() && p.completed_at.is_some_and(|t| t < cutoff)
            })
            .map(|(id, _)| *id)
            .collect();

        for id in &to_remove {
            self.positions.remove(id);
        }

        for ids in self.positions_by_user.values_mut() {
            ids.retain(|id| !to_remove.contains(id));
        }

        to_remove.len()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn create_test_order(user_id: Uuid, side: ShadowOrderSide, quantity: Decimal) -> ShadowOrder {
        ShadowOrder::new(
            user_id,
            "BTC_USDC".to_string(),
            side,
            super::super::orders::ShadowOrderType::Market,
            quantity,
            None,
            None,
            None,
        )
    }

    #[test]
    fn test_position_creation_from_fill() {
        let user_id = Uuid::new_v4();
        let order = create_test_order(user_id, ShadowOrderSide::Buy, dec!(0.1));
        let position = ShadowPosition::from_fill(&order, dec!(50000));

        assert_eq!(position.symbol, "BTC_USDC");
        assert_eq!(position.side, PositionSide::Long);
        assert_eq!(position.size, dec!(0.1));
        assert_eq!(position.entry_price, dec!(50000));
        assert_eq!(position.unrealized_pnl, dec!(0));
    }

    #[test]
    fn test_position_pnl_calculation_long() {
        let user_id = Uuid::new_v4();
        let order = create_test_order(user_id, ShadowOrderSide::Buy, dec!(0.1));
        let mut position = ShadowPosition::from_fill(&order, dec!(50000));

        // Price goes up - profit
        position.update_mark_price(dec!(51000));
        // 0.1 BTC * (51000 - 50000) = 0.1 * 1000 = 100 USDC profit
        assert_eq!(position.unrealized_pnl, dec!(100));

        // Price goes down - loss
        position.update_mark_price(dec!(49000));
        // 0.1 BTC * (49000 - 50000) = 0.1 * -1000 = -100 USDC loss
        assert_eq!(position.unrealized_pnl, dec!(-100));
    }

    #[test]
    fn test_position_pnl_calculation_short() {
        let user_id = Uuid::new_v4();
        let order = create_test_order(user_id, ShadowOrderSide::Sell, dec!(0.1));
        let mut position = ShadowPosition::from_fill(&order, dec!(50000));

        // Price goes down - profit for short
        position.update_mark_price(dec!(49000));
        // 0.1 BTC * (50000 - 49000) = 0.1 * 1000 = 100 USDC profit
        assert_eq!(position.unrealized_pnl, dec!(100));

        // Price goes up - loss for short
        position.update_mark_price(dec!(51000));
        // 0.1 BTC * (50000 - 51000) = 0.1 * -1000 = -100 USDC loss
        assert_eq!(position.unrealized_pnl, dec!(-100));
    }

    #[test]
    fn test_position_reduce_size() {
        let user_id = Uuid::new_v4();
        let order = create_test_order(user_id, ShadowOrderSide::Buy, dec!(0.2));
        let mut position = ShadowPosition::from_fill(&order, dec!(50000));

        // Close half at profit
        let exit_order_id = Uuid::new_v4();
        position.reduce_size(dec!(0.1), dec!(52000), exit_order_id);

        // Realized P&L: 0.1 * (52000 - 50000) = 200
        assert_eq!(position.realized_pnl, dec!(200));
        assert_eq!(position.size, dec!(0.1));
        assert!(!position.is_closed());

        // Close remaining
        let exit_order_id2 = Uuid::new_v4();
        position.reduce_size(dec!(0.1), dec!(53000), exit_order_id2);

        // Additional realized P&L: 0.1 * (53000 - 50000) = 300
        assert_eq!(position.realized_pnl, dec!(500));
        assert!(position.is_closed());
    }

    #[test]
    fn test_position_manager_create_and_update() {
        let mut manager = ShadowPositionManager::new();
        let user_id = Uuid::new_v4();

        // Open a long position
        let buy_order = create_test_order(user_id, ShadowOrderSide::Buy, dec!(0.1));
        manager.update_from_fill(&buy_order, dec!(50000));

        let positions = manager.get_positions(user_id);
        assert_eq!(positions.len(), 1);
        assert_eq!(positions[0].size, dec!(0.1));

        // Add to the position
        let buy_order2 = create_test_order(user_id, ShadowOrderSide::Buy, dec!(0.1));
        manager.update_from_fill(&buy_order2, dec!(52000));

        let positions = manager.get_positions(user_id);
        assert_eq!(positions.len(), 1);
        assert_eq!(positions[0].size, dec!(0.2));
        // Average entry: (0.1 * 50000 + 0.1 * 52000) / 0.2 = 51000
        assert_eq!(positions[0].entry_price, dec!(51000));
    }

    #[test]
    fn test_position_manager_close_position() {
        let mut manager = ShadowPositionManager::new();
        let user_id = Uuid::new_v4();

        // Open a long position
        let buy_order = create_test_order(user_id, ShadowOrderSide::Buy, dec!(0.1));
        manager.update_from_fill(&buy_order, dec!(50000));

        // Close the position
        let sell_order = create_test_order(user_id, ShadowOrderSide::Sell, dec!(0.1));
        manager.update_from_fill(&sell_order, dec!(55000));

        // Position should be closed (empty list of open positions)
        let positions = manager.get_positions(user_id);
        assert!(positions.is_empty());

        // But realized P&L should be tracked
        let realized_pnl = manager.get_total_realized_pnl(user_id);
        // 0.1 * (55000 - 50000) = 500
        assert_eq!(realized_pnl, dec!(500));
    }

    #[test]
    fn test_mark_price_updates() {
        let mut manager = ShadowPositionManager::new();
        let user_id = Uuid::new_v4();

        let buy_order = create_test_order(user_id, ShadowOrderSide::Buy, dec!(0.1));
        manager.update_from_fill(&buy_order, dec!(50000));

        // Update mark price
        manager.update_mark_price("BTC_USDC", dec!(55000));

        let pnl = manager.get_total_unrealized_pnl(user_id);
        // 0.1 * (55000 - 50000) = 500
        assert_eq!(pnl, dec!(500));
    }

    // AUD-02: GC tests

    #[test]
    fn test_prune_terminal_removes_old_closed_positions() {
        let mut manager = ShadowPositionManager::new();
        let user_id = Uuid::new_v4();

        // Open and close a position
        let buy_order = create_test_order(user_id, ShadowOrderSide::Buy, dec!(0.1));
        manager.update_from_fill(&buy_order, dec!(50000));
        let sell_order = create_test_order(user_id, ShadowOrderSide::Sell, dec!(0.1));
        manager.update_from_fill(&sell_order, dec!(55000));

        // Position is closed
        assert!(manager.get_positions(user_id).is_empty());

        // Cutoff in the future — should prune
        let cutoff = Instant::now() + std::time::Duration::from_secs(1);
        let pruned = manager.prune_terminal(cutoff);
        assert_eq!(pruned, 1);

        // Realized PnL data is gone too
        assert_eq!(manager.get_total_realized_pnl(user_id), dec!(0));
    }

    #[test]
    fn test_prune_terminal_keeps_open_positions() {
        let mut manager = ShadowPositionManager::new();
        let user_id = Uuid::new_v4();

        let buy_order = create_test_order(user_id, ShadowOrderSide::Buy, dec!(0.1));
        manager.update_from_fill(&buy_order, dec!(50000));

        // Even with future cutoff, open positions stay
        let cutoff = Instant::now() + std::time::Duration::from_secs(1);
        let pruned = manager.prune_terminal(cutoff);
        assert_eq!(pruned, 0);
        assert_eq!(manager.get_positions(user_id).len(), 1);
    }
}
