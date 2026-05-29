// @anchor exchange:engine:orderbook
// @tags domain

use crate::engine::error::CoreEngineError;
use rust_decimal::Decimal;
use rust_decimal_macros::dec;
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, HashMap, HashSet};

use crate::types::engine::{AssetPair, CancelOrder, Fill, Order, OrderSide, ProcessOrderResult};

/// Order location for efficient order lookup and removal
#[derive(Debug, Clone, Serialize, Deserialize)]
struct OrderLocation {
    price: Decimal,
    side: OrderSide,
}

/// Order book with price-time priority matching
///
/// # Performance Optimizations (006-performance-overhaul)
///
/// - **FR-3.2: User Order Index** - `user_orders` index enables O(1) lookup of user's orders
/// - **FR-3.4: Range Matching** - Uses `BTreeMap::range()` for efficient price level matching
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OrderBook {
    pub bids: BTreeMap<Decimal, Vec<Order>>,
    pub asks: BTreeMap<Decimal, Vec<Order>>,
    pub asset_pair: AssetPair,
    pub trade_id: i64,
    last_update_id: i64,

    /// FR-3.2: Index of user_id -> order_ids for O(1) user order lookup
    /// This avoids scanning all orders when retrieving a user's open orders.
    #[serde(default)]
    user_orders: HashMap<String, HashSet<String>>,

    /// FR-3.2: Index of order_id -> location for O(1) order lookup
    /// Enables direct access to an order without scanning price levels.
    #[serde(default)]
    order_locations: HashMap<String, OrderLocation>,
}

impl OrderBook {
    pub fn new(asset_pair: AssetPair, trade_id: i64) -> OrderBook {
        OrderBook {
            asks: BTreeMap::new(),
            bids: BTreeMap::new(),
            asset_pair,
            trade_id,
            last_update_id: 0,
            user_orders: HashMap::new(),
            order_locations: HashMap::new(),
        }
    }

    /// FR-3.2.2: Add order to user index
    fn index_order(&mut self, order: &Order, side: OrderSide) {
        // Add to user_orders index
        self.user_orders
            .entry(order.user_id.clone())
            .or_default()
            .insert(order.order_id.clone());

        // Add to order_locations index
        self.order_locations.insert(
            order.order_id.clone(),
            OrderLocation {
                price: order.price,
                side,
            },
        );
    }

    /// FR-3.2.2: Remove order from user index
    fn unindex_order(&mut self, order: &Order) {
        // Remove from user_orders index
        if let Some(orders) = self.user_orders.get_mut(&order.user_id) {
            orders.remove(&order.order_id);
            if orders.is_empty() {
                self.user_orders.remove(&order.user_id);
            }
        }

        // Remove from order_locations index
        self.order_locations.remove(&order.order_id);
    }

    pub fn ticker(&self) -> String {
        format!("{:?}_{:?}", self.asset_pair.base, self.asset_pair.quote)
    }

    pub fn process_order(&mut self, mut order: Order) -> ProcessOrderResult {
        let order_result: ProcessOrderResult;

        match order.side {
            OrderSide::BUY => {
                order_result = self.match_asks(&order);
                order.filled_quantity = order_result.executed_quantity;
                if order_result.executed_quantity < order.quantity {
                    // FR-3.2.2: Index the order before adding to book
                    self.index_order(&order, OrderSide::BUY);
                    self.bids
                        .entry(order.price)
                        .and_modify(|orders| orders.push(order.clone()))
                        .or_insert(vec![order]);
                }
                order_result
            }
            OrderSide::SELL => {
                order_result = self.match_bids(&order);

                if order_result.executed_quantity < order.quantity {
                    // FR-3.2.2: Index the order before adding to book
                    self.index_order(&order, OrderSide::SELL);
                    self.asks
                        .entry(order.price)
                        .and_modify(|orders| orders.push(order.clone()))
                        .or_insert(vec![order]);
                }
                order_result
            }
        }
    }

    /// Match a buy order against asks
    ///
    /// FR-3.4.1: Uses `range(..=order.price)` to only iterate asks at or below the buy price.
    /// This is O(log n + k) where k is the number of matching price levels.
    pub fn match_asks(&mut self, order: &Order) -> ProcessOrderResult {
        let mut fills: Vec<Fill> = vec![];
        let mut executed_quantity: Decimal = dec!(0);
        let mut filled_order_ids: Vec<String> = vec![];

        // FR-3.4.1: Only iterate asks at prices <= buy order price
        // BTreeMap is sorted ascending, so range(..=price) gives us all asks we can match
        let matching_prices: Vec<Decimal> =
            self.asks.range(..=order.price).map(|(p, _)| *p).collect();

        for price in matching_prices {
            if let Some(asks) = self.asks.get_mut(&price) {
                for ask in asks.iter_mut() {
                    if executed_quantity >= order.quantity {
                        break;
                    }

                    let remaining = order.quantity - executed_quantity;
                    let available = ask.quantity - ask.filled_quantity;
                    let filled_quantity = std::cmp::min(remaining, available);

                    if filled_quantity > dec!(0) {
                        self.trade_id += 1;
                        executed_quantity += filled_quantity;
                        ask.filled_quantity += filled_quantity;

                        fills.push(Fill {
                            price: ask.price,
                            quantity: filled_quantity,
                            trade_id: self.trade_id,
                            other_user_id: ask.user_id.clone(),
                            order_id: ask.order_id.clone(),
                        });

                        // Track fully filled orders for index cleanup
                        if ask.filled_quantity >= ask.quantity {
                            filled_order_ids.push(ask.order_id.clone());
                        }
                    }
                }

                // Remove asks that have been completely filled
                asks.retain(|ask| ask.filled_quantity < ask.quantity);
            }
        }

        // FR-3.2.2 + AUD-02 FR-7: Clean up indexes for fully filled orders
        for fill in &fills {
            if filled_order_ids.contains(&fill.order_id) {
                self.order_locations.remove(&fill.order_id);
                if let Some(ids) = self.user_orders.get_mut(&fill.other_user_id) {
                    ids.remove(&fill.order_id);
                    if ids.is_empty() {
                        self.user_orders.remove(&fill.other_user_id);
                    }
                }
            }
        }

        // Clean up empty price levels
        self.asks.retain(|_, orders| !orders.is_empty());

        ProcessOrderResult {
            fills,
            executed_quantity,
        }
    }

    /// Match a sell order against bids
    ///
    /// FR-3.4.2: Uses `range(order.price..)` to only iterate bids at or above the sell price.
    /// This is O(log n + k) where k is the number of matching price levels.
    pub fn match_bids(&mut self, order: &Order) -> ProcessOrderResult {
        let mut fills: Vec<Fill> = vec![];
        let mut executed_quantity: Decimal = dec!(0);
        let mut filled_order_ids: Vec<String> = vec![];

        // FR-3.4.2: Only iterate bids at prices >= sell order price
        // BTreeMap is sorted ascending, so range(price..) gives bids at or above sell price
        // We reverse to match highest bids first (price-time priority)
        let matching_prices: Vec<Decimal> = self
            .bids
            .range(order.price..)
            .map(|(p, _)| *p)
            .rev()
            .collect();

        for price in matching_prices {
            if let Some(bids) = self.bids.get_mut(&price) {
                for bid in bids.iter_mut() {
                    if executed_quantity >= order.quantity {
                        break;
                    }

                    let remaining = order.quantity - executed_quantity;
                    let available = bid.quantity - bid.filled_quantity;
                    let filled_quantity = std::cmp::min(remaining, available);

                    if filled_quantity > dec!(0) {
                        self.trade_id += 1;
                        executed_quantity += filled_quantity;
                        bid.filled_quantity += filled_quantity;

                        fills.push(Fill {
                            price: bid.price,
                            quantity: filled_quantity,
                            trade_id: self.trade_id,
                            other_user_id: bid.user_id.clone(),
                            order_id: bid.order_id.clone(),
                        });

                        // Track fully filled orders for index cleanup
                        if bid.filled_quantity >= bid.quantity {
                            filled_order_ids.push(bid.order_id.clone());
                        }
                    }
                }

                // Remove bids that have been completely filled
                bids.retain(|bid| bid.filled_quantity < bid.quantity);
            }
        }

        // FR-3.2.2 + AUD-02 FR-7: Clean up indexes for fully filled orders
        for fill in &fills {
            if filled_order_ids.contains(&fill.order_id) {
                self.order_locations.remove(&fill.order_id);
                if let Some(ids) = self.user_orders.get_mut(&fill.other_user_id) {
                    ids.remove(&fill.order_id);
                    if ids.is_empty() {
                        self.user_orders.remove(&fill.other_user_id);
                    }
                }
            }
        }

        // Clean up empty price levels
        self.bids.retain(|_, orders| !orders.is_empty());

        ProcessOrderResult {
            fills,
            executed_quantity,
        }
    }

    pub fn get_open_order(&self, user_id: String, order_id: String) -> Result<&Order, CoreEngineError> {
        let order = self
            .bids
            .values()
            .chain(self.asks.values()) // Combine bids and asks
            .flat_map(|orders| orders.iter()) // Flatten the Vec<Order> for each price level
            .find(|order| order.user_id == user_id && order.order_id == order_id);

        order.ok_or(CoreEngineError::OrderbookNotFound {
            market: self.ticker(),
        })
    }

    /// Get all open orders for a user
    ///
    /// FR-3.2.3: Uses user_orders index for O(k) lookup where k is the user's order count.
    /// Previously O(n) where n is total orders in the book.
    pub fn get_open_orders(&self, user_id: String) -> Vec<&Order> {
        // FR-3.2.3: Use index for efficient lookup
        let order_ids = match self.user_orders.get(&user_id) {
            Some(ids) => ids,
            None => return vec![],
        };

        let mut orders = Vec::with_capacity(order_ids.len());

        for order_id in order_ids {
            // Use order_locations to find the order directly
            if let Some(location) = self.order_locations.get(order_id) {
                let book = match location.side {
                    OrderSide::BUY => &self.bids,
                    OrderSide::SELL => &self.asks,
                };

                if let Some(orders_at_price) = book.get(&location.price) {
                    if let Some(order) = orders_at_price.iter().find(|o| &o.order_id == order_id) {
                        orders.push(order);
                    }
                }
            }
        }

        orders
    }

    /// Cancel a specific order
    ///
    /// FR-3.2.2: Maintains user_orders and order_locations indexes.
    pub fn cancel_order(&mut self, cancel_order: CancelOrder) -> Result<Order, CoreEngineError> {
        let (order, should_remove_price) = {
            let orders_map = match cancel_order.side {
                OrderSide::BUY => &mut self.bids,
                OrderSide::SELL => &mut self.asks,
            };

            if let Some(orders) = orders_map.get_mut(&cancel_order.price) {
                if let Some(index) = orders
                    .iter()
                    .position(|order| order.order_id == cancel_order.order_id)
                {
                    let order = orders.remove(index);
                    let should_remove = orders.is_empty();
                    (Some(order), should_remove)
                } else {
                    (None, false)
                }
            } else {
                (None, false)
            }
        };

        match order {
            Some(order) => {
                // FR-3.2.2: Update indexes
                self.unindex_order(&order);

                // Clean up empty price level
                if should_remove_price {
                    match cancel_order.side {
                        OrderSide::BUY => self.bids.remove(&cancel_order.price),
                        OrderSide::SELL => self.asks.remove(&cancel_order.price),
                    };
                }

                Ok(order)
            }
            None => Err(CoreEngineError::CancelOrderFailed),
        }
    }

    /// Cancel all orders for a user
    ///
    /// FR-3.2.2: Uses user_orders index for efficient cancellation.
    /// Returns empty vec since all orders are now cancelled.
    pub fn cancel_all_orders(&mut self, user_id: String) -> Vec<&Order> {
        // FR-3.2.3: Use index to find all user's orders
        let order_ids: Vec<String> = self
            .user_orders
            .get(&user_id)
            .map(|ids| ids.iter().cloned().collect())
            .unwrap_or_default();

        // Remove each order using the index
        for order_id in order_ids {
            if let Some(location) = self.order_locations.remove(&order_id) {
                let orders_map = match location.side {
                    OrderSide::BUY => &mut self.bids,
                    OrderSide::SELL => &mut self.asks,
                };

                if let Some(orders) = orders_map.get_mut(&location.price) {
                    orders.retain(|o| o.order_id != order_id);
                    if orders.is_empty() {
                        orders_map.remove(&location.price);
                    }
                }
            }
        }

        // Clear user's entry from user_orders index
        self.user_orders.remove(&user_id);

        // Return empty vec - all orders cancelled
        vec![]
    }

    pub fn get_depth(&self) -> (Vec<(Decimal, Decimal)>, Vec<(Decimal, Decimal)>) {
        let mut bids_depth: Vec<(Decimal, Decimal)> = Vec::new();
        let mut asks_depth: Vec<(Decimal, Decimal)> = Vec::new();

        // Aggregate quantities for each price level in bids
        for (price, orders) in self.bids.iter() {
            let total_quantity = orders
                .iter()
                .fold(Decimal::ZERO, |acc, order| acc + order.quantity);
            bids_depth.push((*price, total_quantity));
        }

        // Aggregate quantities for each price level in asks
        for (price, orders) in self.asks.iter() {
            let total_quantity = orders
                .iter()
                .fold(Decimal::ZERO, |acc, order| acc + order.quantity);
            asks_depth.push((*price, total_quantity));
        }

        (bids_depth, asks_depth)
    }
}
