use crate::engine::error::CoreEngineError;
use crate::engine::orderbook::OrderBook;
use crate::types::engine::{
    Asset, AssetPair, CancelAllOrders, CancelOrder, CreateOrder, GetDepth, GetOpenOrder,
    GetOpenOrders, Order, OrderSide, ProcessOrderResult,
};
use db_processor::query::get_latest_trade_id_from_db;
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use sqlx::{Pool, Postgres};
use std::collections::HashMap;
use std::sync::Mutex;

pub enum AmountType {
    AVAILABLE,
    LOCKED,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Amount {
    available: Decimal,
    locked: Decimal,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UserBalances {
    user_id: String,
    balance: HashMap<Asset, Amount>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Engine {
    pub orderbooks: Vec<OrderBook>,
    pub balances: HashMap<String, Mutex<UserBalances>>,
}

impl Default for Engine {
    fn default() -> Self {
        Self::new()
    }
}

impl Engine {
    pub fn new() -> Engine {
        Engine {
            orderbooks: vec![],
            balances: HashMap::new(),
        }
    }

    pub async fn init_engine(&mut self, pool: &Pool<Postgres>) -> Result<(), CoreEngineError> {
        let market = "SOL_USDC".to_string();
        let trade_id: i64 = get_latest_trade_id_from_db(pool, market.clone())
            .await
            .map_err(|e| CoreEngineError::Internal {
                detail: format!("Failed to fetch latest trade_id for {}: {}", market, e),
            })?;

        let orderbook = OrderBook::new(
            AssetPair {
                base: Asset::SOL,
                quote: Asset::USDC,
            },
            trade_id + 1,
        );

        self.orderbooks.push(orderbook);

        Ok(())
    }

    pub fn init_user_balance(&mut self, user_id: &str) {
        let initial_balances = UserBalances {
            user_id: user_id.to_string(),
            balance: HashMap::new(),
        };

        // Add dummy values for USDC and SOL
        let usdc_balance = Amount {
            available: Decimal::new(1000000, 0), // Dummy value: 1000000 USDC
            locked: Decimal::new(0, 0),          // 0 locked
        };

        let sol_balance = Amount {
            available: Decimal::new(10000, 0), // Dummy value: 10000 SOL
            locked: Decimal::new(0, 0),        // 0 locked
        };

        // Initialize the balance HashMap for the user
        let mut balances_map = initial_balances.balance;
        balances_map.insert(Asset::USDC, usdc_balance);
        balances_map.insert(Asset::SOL, sol_balance);

        // Add the initialized UserBalances to the Engine's balances map
        self.balances.insert(
            user_id.to_string(),
            Mutex::new(UserBalances {
                user_id: user_id.to_string(),
                balance: balances_map,
            }),
        );
    }

    pub fn get_open_order(&mut self, open_order: GetOpenOrder) -> Result<&Order, CoreEngineError> {
        let orderbook = match self
            .orderbooks
            .iter_mut()
            .find(|orderbook| orderbook.ticker() == open_order.market)
        {
            Some(ob) => ob,
            None => {
                return Err(CoreEngineError::OrderbookNotFound {
                    market: open_order.market,
                });
            }
        };

        let open_order = orderbook.get_open_order(open_order.user_id, open_order.order_id);

        open_order
    }

    pub fn cancel_order(&mut self, cancel_order: CancelOrder) -> Result<String, CoreEngineError> {
        let orderbook = match self
            .orderbooks
            .iter_mut()
            .find(|orderbook| orderbook.ticker() == cancel_order.market)
        {
            Some(ob) => ob,
            None => {
                return Err(CoreEngineError::OrderbookNotFound {
                    market: cancel_order.market,
                });
            }
        };

        let assets: Vec<&str> = cancel_order.market.split('_').collect();
        let base_asset_str = assets[0];
        let quote_asset_str = assets[1];
        let base_asset = Asset::parse(base_asset_str)?;
        let quote_asset = Asset::parse(quote_asset_str)?;
        let cancel_order_id = cancel_order.order_id.clone();

        let order = orderbook.cancel_order(cancel_order)?;

        let quantity = match order.side {
            OrderSide::BUY => (order.quantity - order.filled_quantity) * order.price,
            OrderSide::SELL => order.quantity - order.filled_quantity,
        };

        match order.side {
            OrderSide::BUY => {
                self.update_balance_with_lock(
                    order.user_id.clone(),
                    quote_asset.clone(),
                    quantity,
                    AmountType::AVAILABLE,
                )?;

                self.update_balance_with_lock(
                    order.user_id.clone(),
                    quote_asset.clone(),
                    -quantity,
                    AmountType::LOCKED,
                )?;
            }

            OrderSide::SELL => {
                self.update_balance_with_lock(
                    order.user_id.clone(),
                    base_asset.clone(),
                    quantity,
                    AmountType::AVAILABLE,
                )?;

                self.update_balance_with_lock(
                    order.user_id.clone(),
                    base_asset.clone(),
                    -quantity,
                    AmountType::LOCKED,
                )?;
            }
        }

        Ok(cancel_order_id)
    }

    pub fn get_open_orders(&mut self, open_orders: GetOpenOrders) -> Vec<&Order> {
        let orderbook = match self
            .orderbooks
            .iter_mut()
            .find(|orderbook| orderbook.ticker() == open_orders.market)
        {
            Some(ob) => ob,
            None => {
                tracing::warn!(market = %open_orders.market, "No matching orderbook found");
                return Vec::new();
            }
        };

        let open_orders: Vec<&Order> = orderbook.get_open_orders(open_orders.user_id);

        open_orders
    }

    pub fn cancel_all_orders(
        &mut self,
        cancel_all_orders: CancelAllOrders,
    ) -> Result<String, CoreEngineError> {
        let orderbook = match self
            .orderbooks
            .iter_mut()
            .find(|orderbook| orderbook.ticker() == cancel_all_orders.market)
        {
            Some(ob) => ob,
            None => {
                return Err(CoreEngineError::OrderbookNotFound {
                    market: cancel_all_orders.market,
                });
            }
        };

        let assets: Vec<&str> = cancel_all_orders.market.split('_').collect();
        let base_asset_str = assets[0];
        let quote_asset_str = assets[1];
        let base_asset = Asset::parse(base_asset_str)?;
        let quote_asset = Asset::parse(quote_asset_str)?;

        let open_orders = orderbook.cancel_all_orders(cancel_all_orders.user_id.clone());

        let mut balance_updates: Vec<(String, Asset, Decimal, AmountType)> = Vec::new();

        for order in open_orders {
            let quantity = match order.side {
                OrderSide::BUY => (order.quantity - order.filled_quantity) * order.price,
                OrderSide::SELL => order.quantity - order.filled_quantity,
            };

            match order.side {
                OrderSide::BUY => {
                    balance_updates.push((
                        order.user_id.clone(),
                        quote_asset.clone(),
                        quantity,
                        AmountType::AVAILABLE,
                    ));
                    balance_updates.push((
                        order.user_id.clone(),
                        quote_asset.clone(),
                        -quantity,
                        AmountType::LOCKED,
                    ));
                }

                OrderSide::SELL => {
                    balance_updates.push((
                        order.user_id.clone(),
                        base_asset.clone(),
                        quantity,
                        AmountType::AVAILABLE,
                    ));
                    balance_updates.push((
                        order.user_id.clone(),
                        base_asset.clone(),
                        -quantity,
                        AmountType::LOCKED,
                    ));
                }
            }
        }

        // Perform balance updates after the loop, ensuring only one mutable borrow of `self`
        for (user_id, asset, amount, amount_type) in balance_updates {
            self.update_balance_with_lock(user_id, asset, amount, amount_type)?;
        }

        // Return a success message after cancelling all orders
        Ok(format!(
            "All orders for user {} cancelled successfully",
            cancel_all_orders.user_id
        ))
    }

    pub fn get_depth(&self, depth: GetDepth) -> (Vec<(Decimal, Decimal)>, Vec<(Decimal, Decimal)>) {
        let orderbook = match self
            .orderbooks
            .iter()
            .find(|orderbook| orderbook.ticker() == depth.symbol)
        {
            Some(ob) => ob,
            None => {
                tracing::warn!(market = %depth.symbol, "No matching orderbook found");
                return (Vec::new(), Vec::new());
            }
        };

        orderbook.get_depth()
    }

    pub fn check_and_lock_funds(&mut self, order: &CreateOrder) -> Result<(), CoreEngineError> {
        let assets: Vec<&str> = order.market.split('_').collect();
        let base_asset_str = assets[0];
        let quote_asset_str = assets[1];

        // Convert string assets to Asset enum
        let base_asset = Asset::parse(base_asset_str)?;
        let quote_asset = Asset::parse(quote_asset_str)?;

        let user_id = &order.user_id;

        let user_balance_mutex = self
            .balances
            .get_mut(user_id)
            .ok_or(CoreEngineError::UserNotFound {
                user_id: user_id.clone(),
            })?;

        // Access the user's balances (using get_mut since we already have mutable access)
        let user_balance = user_balance_mutex
            .get_mut()
            .map_err(|_| CoreEngineError::MutexLockFailed)?;

        match order.side {
            OrderSide::BUY => {
                let balance = user_balance
                    .balance
                    .get_mut(&quote_asset)
                    .ok_or(CoreEngineError::BalanceNotFound {
                        user_id: user_id.clone(),
                        asset: quote_asset_str.to_string(),
                    })?;

                let total_cost = order.price * order.quantity;
                if balance.available >= total_cost {
                    let total_before = balance.available + balance.locked;
                    balance.available -= total_cost;
                    balance.locked += total_cost;
                    assert!(balance.available >= Decimal::ZERO, "available negative after lock BUY");
                    assert!(balance.locked >= Decimal::ZERO, "locked negative after lock BUY");
                    assert_eq!(
                        total_before,
                        balance.available + balance.locked,
                        "Balance conservation violated in lock BUY"
                    );
                } else {
                    return Err(CoreEngineError::InsufficientFunds {
                        user_id: user_id.clone(),
                        required: total_cost,
                        available: balance.available,
                    });
                }
            }

            OrderSide::SELL => {
                // User must have order.quantity of base_asset
                let balance = user_balance
                    .balance
                    .get_mut(&base_asset)
                    .ok_or(CoreEngineError::BalanceNotFound {
                        user_id: user_id.clone(),
                        asset: base_asset_str.to_string(),
                    })?;

                if balance.available >= order.quantity {
                    let total_before = balance.available + balance.locked;
                    balance.available -= order.quantity;
                    balance.locked += order.quantity;
                    assert!(balance.available >= Decimal::ZERO, "available negative after lock SELL");
                    assert!(balance.locked >= Decimal::ZERO, "locked negative after lock SELL");
                    assert_eq!(
                        total_before,
                        balance.available + balance.locked,
                        "Balance conservation violated in lock SELL"
                    );
                } else {
                    return Err(CoreEngineError::InsufficientQuantity {
                        user_id: user_id.clone(),
                        required: order.quantity,
                        available: balance.available,
                    });
                }
            }
        }

        Ok(())
    }

    pub fn update_user_balance(
        &mut self,
        base_asset: Asset,
        quote_asset: Asset,
        order: Order,
        order_result: &ProcessOrderResult,
    ) -> Result<(), CoreEngineError> {
        match order.side {
            OrderSide::BUY => {
                for fill in &order_result.fills {
                    // Update buyer's balances (current user)
                    self.update_balance_with_lock(
                        order.user_id.clone(),
                        base_asset.clone(),
                        fill.quantity,
                        AmountType::AVAILABLE,
                    )?;
                    self.update_balance_with_lock(
                        order.user_id.clone(),
                        quote_asset.clone(),
                        -(fill.price * fill.quantity),
                        AmountType::LOCKED,
                    )?;

                    // Update seller's balances (other user)
                    self.update_balance_with_lock(
                        fill.other_user_id.clone(),
                        quote_asset.clone(),
                        fill.price * fill.quantity,
                        AmountType::AVAILABLE,
                    )?;
                    self.update_balance_with_lock(
                        fill.other_user_id.clone(),
                        base_asset.clone(),
                        -fill.quantity,
                        AmountType::LOCKED,
                    )?;
                }
            }
            OrderSide::SELL => {
                for fill in &order_result.fills {
                    // Update seller's balances (current user)
                    self.update_balance_with_lock(
                        order.user_id.clone(),
                        base_asset.clone(),
                        -fill.quantity,
                        AmountType::LOCKED,
                    )?;
                    self.update_balance_with_lock(
                        order.user_id.clone(),
                        quote_asset.clone(),
                        fill.price * fill.quantity,
                        AmountType::AVAILABLE,
                    )?;

                    // Update buyer's balances (other user)
                    self.update_balance_with_lock(
                        fill.other_user_id.clone(),
                        base_asset.clone(),
                        fill.quantity,
                        AmountType::AVAILABLE,
                    )?;
                    self.update_balance_with_lock(
                        fill.other_user_id.clone(),
                        quote_asset.clone(),
                        -(fill.price * fill.quantity),
                        AmountType::LOCKED,
                    )?;
                }
            }
        }
        Ok(())
    }

    // Helper function to update balance with lock
    fn update_balance_with_lock(
        &self,
        user_id: String,
        asset: Asset,
        amount: Decimal,
        amount_type: AmountType,
    ) -> Result<(), CoreEngineError> {
        // Access the user's balance via the Mutex
        let balances = &self.balances;
        let user_balance_mutex = balances
            .get(&user_id)
            .ok_or(CoreEngineError::UserNotFound {
                user_id: user_id.clone(),
            })?;

        // Lock the Mutex to access the user's balances
        let mut user_balance = user_balance_mutex
            .lock()
            .map_err(|_| CoreEngineError::MutexLockFailed)?;

        let balance = user_balance
            .balance
            .get_mut(&asset)
            .ok_or(CoreEngineError::BalanceNotFound {
                user_id: user_id.clone(),
                asset: format!("{:?}", asset),
            })?;

        match amount_type {
            AmountType::AVAILABLE => balance.available += amount,
            AmountType::LOCKED => balance.locked += amount,
        }

        assert!(balance.available >= Decimal::ZERO, "available negative after balance update");
        assert!(balance.locked >= Decimal::ZERO, "locked negative after balance update");

        Ok(())
    }
}
