use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum DatabaseRequests {
    InsertTrade(DbTrade),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DbTrade {
    pub trade_id: i64,
    pub market: String,
    pub price: Decimal,
    pub quantity: Decimal,
    pub user_id: String,
    pub other_user_id: String,
    pub order_id: String,
    pub timestamp: i64,
}
