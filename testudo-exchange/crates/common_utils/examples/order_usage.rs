// Example usage of StandardOrder type
// Run with: cargo run --example order_usage

use common_utils::{OrderSide, OrderType, StandardOrder, StandardOrderBuilder, TimeInForce};
use rust_decimal::Decimal;
use std::str::FromStr;
use uuid::Uuid;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("StandardOrder Type Usage Examples");
    println!("==================================\n");

    // Example 1: Create a simple market order
    let user_id = Uuid::new_v4();
    let market_order = StandardOrder::market_buy(user_id, "BTC/USDT", Decimal::from_str("0.1")?)?;

    println!("1. Market Buy Order:");
    println!("   Symbol: {}", market_order.symbol);
    println!("   Side: {:?}", market_order.side);
    println!("   Type: {:?}", market_order.order_type);
    println!("   Quantity: {}", market_order.quantity);
    println!("   Status: {:?}", market_order.status);
    println!("   Active: {}", market_order.is_active());
    println!();

    // Example 2: Create a limit order using builder pattern
    let limit_order = StandardOrderBuilder::new()
        .user_id(user_id)
        .symbol("ETH/USDT")
        .side(OrderSide::Sell)
        .order_type(OrderType::Limit)
        .quantity(Decimal::from_str("2.5")?)
        .price(Decimal::from_str("2500.0")?)
        .time_in_force(TimeInForce::IOC)
        .exchange("binance")
        .build()?;

    println!("2. Limit Sell Order:");
    println!("   Symbol: {}", limit_order.symbol);
    println!("   Side: {:?}", limit_order.side);
    println!("   Price: {:?}", limit_order.price);
    println!("   Time in Force: {:?}", limit_order.time_in_force);
    println!("   Exchange: {:?}", limit_order.exchange);
    println!();

    // Example 3: Create a stop-loss order with proper validation
    let stop_loss_order = StandardOrderBuilder::new()
        .user_id(user_id)
        .symbol("SOL/USDT")
        .side(OrderSide::Sell)
        .order_type(OrderType::StopLoss)
        .quantity(Decimal::from_str("10.0")?)
        .stop_price(Decimal::from_str("85.0")?)
        .build()?;

    println!("3. Stop Loss Order:");
    println!("   Symbol: {}", stop_loss_order.symbol);
    println!("   Stop Price: {:?}", stop_loss_order.stop_price);
    println!();

    // Example 4: Convert Long/Short to Buy/Sell
    let long_order = StandardOrderBuilder::new()
        .user_id(user_id)
        .symbol("BTC/USDT")
        .side(OrderSide::Long)
        .order_type(OrderType::Market)
        .quantity(Decimal::from_str("0.5")?)
        .build()?;

    println!("4. Margin Order Conversion:");
    println!("   Original Side: {:?}", long_order.side);
    println!("   Spot Side: {:?}", long_order.to_spot_side());
    println!();

    // Example 5: Order status tracking
    let mut tracking_order =
        StandardOrder::market_buy(user_id, "ADA/USDT", Decimal::from_str("100.0")?)?;

    println!("5. Order Status Tracking:");
    println!(
        "   Initial Status: {:?} (Active: {})",
        tracking_order.status,
        tracking_order.is_active()
    );

    tracking_order.update_status(common_utils::OrderStatus::PartiallyFilled);
    println!(
        "   After Partial Fill: {:?} (Active: {})",
        tracking_order.status,
        tracking_order.is_active()
    );

    tracking_order.update_status(common_utils::OrderStatus::Filled);
    println!(
        "   After Full Fill: {:?} (Final: {})",
        tracking_order.status,
        tracking_order.is_final()
    );
    println!();

    // Example 6: JSON serialization
    let serializable_order = StandardOrder::limit_sell(
        user_id,
        "DOT/USDT",
        Decimal::from_str("5.0")?,
        Decimal::from_str("25.50")?,
    )?;

    let json = serde_json::to_string_pretty(&serializable_order).unwrap();
    println!("6. JSON Serialization:");
    println!("{}", json);

    // Example 7: Error handling - invalid order
    println!("\n7. Error Handling:");
    match StandardOrder::market_buy(user_id, "", Decimal::ZERO) {
        Ok(_) => println!("   This shouldn't happen!"),
        Err(e) => println!("   Validation Error: {}", e),
    }

    // Example 8: String parsing of enums
    println!("\n8. Enum Parsing:");
    let side = OrderSide::from_str("buy")?;
    let order_type = OrderType::from_str("limit")?;
    let time_in_force = TimeInForce::from_str("GTC")?;

    println!("   Parsed Side: {:?}", side);
    println!("   Parsed Type: {:?}", order_type);
    println!("   Parsed TIF: {:?}", time_in_force);

    Ok(())
}
