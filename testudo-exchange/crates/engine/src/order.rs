// @anchor exchange:engine:order
// @tags domain

use crate::{types::engine::OrderRequests, Engine};
use pg_queue::PgQueueManager;
use std::sync::Arc;
use tokio::sync::Mutex;

/// Handle order requests from PostgreSQL queue
/// Uses pg_notify for responses instead of Redis pub/sub
pub async fn handle_order_pg(
    payload: serde_json::Value,
    pg_queue: &Arc<PgQueueManager>,
    engine: Arc<Mutex<Engine>>,
) {
    match serde_json::from_value::<OrderRequests>(payload) {
        Ok(order) => match order {
            OrderRequests::CreateOrder(order) => {
                println!("Create Order: {:?}", order);
                let pubsub_id = match order.pubsub_id {
                    Some(id) => id.to_string(),
                    None => {
                        tracing::warn!("CreateOrder missing pubsub_id, cannot respond");
                        return;
                    }
                };

                // create_order_pg was deprecated and removed (CLN-07).
                // All order creation now goes through the Shadow Engine via Decision Loop.
                let response = serde_json::json!({
                    "status": "Failed to Create Order",
                    "reason": "Legacy engine order creation removed — use Shadow Engine",
                });

                if let Ok(response_string) = serde_json::to_string(&response) {
                    let _ = pg_queue.notify.notify(&pubsub_id, &response_string).await;
                }
            }

            OrderRequests::GetOpenOrder(open_order) => {
                println!("Get Open Order: {:?}", open_order);
                let pubsub_id = match open_order.pubsub_id {
                    Some(id) => id.to_string(),
                    None => {
                        tracing::warn!("GetOpenOrder missing pubsub_id, cannot respond");
                        return;
                    }
                };

                let response = {
                    let mut engine_guard = engine.lock().await;
                    match engine_guard.get_open_order(open_order) {
                        Ok(open_order) => {
                            println!("Successfully retrieved open order!");
                            serde_json::json!(open_order)
                        }
                        Err(_) => {
                            println!("Order retrieval failed");
                            serde_json::json!({
                                "status": "Failed to Retrieve Open Order",
                            })
                        }
                    }
                };

                if let Ok(response_string) = serde_json::to_string(&response) {
                    let _ = pg_queue.notify.notify(&pubsub_id, &response_string).await;
                }
            }

            OrderRequests::CancelOrder(cancel_order) => {
                println!("Cancel Order: {:?}", cancel_order);
                let pubsub_id = match cancel_order.pubsub_id {
                    Some(id) => id.to_string(),
                    None => {
                        tracing::warn!("CancelOrder missing pubsub_id, cannot respond");
                        return;
                    }
                };

                let response = {
                    let mut engine_guard = engine.lock().await;
                    match engine_guard.cancel_order(cancel_order) {
                        Ok(cancel_order_id) => {
                            println!("Successfully cancelled order!");
                            serde_json::json!({
                                "status": "Cancelled Order",
                                "order_id": cancel_order_id,
                            })
                        }
                        Err(err) => {
                            println!("Order cancellation failed - {}", err);
                            serde_json::json!({
                                "status": "Failed to Cancel Order",
                            })
                        }
                    }
                };

                if let Ok(response_string) = serde_json::to_string(&response) {
                    let _ = pg_queue.notify.notify(&pubsub_id, &response_string).await;
                }
            }

            OrderRequests::GetOpenOrders(open_orders) => {
                println!("Open Order: {:?}", open_orders);
                let pubsub_id = match open_orders.pubsub_id {
                    Some(id) => id.to_string(),
                    None => {
                        tracing::warn!("GetOpenOrders missing pubsub_id, cannot respond");
                        return;
                    }
                };

                // Serialize while holding the lock since get_open_orders returns references
                let response_string = {
                    let mut engine_guard = engine.lock().await;
                    let open_orders_vec = engine_guard.get_open_orders(open_orders);
                    serde_json::to_string(&open_orders_vec)
                };

                if let Ok(response_string) = response_string {
                    let _ = pg_queue.notify.notify(&pubsub_id, &response_string).await;
                }
                println!("Successfully retrieved open orders!");
            }

            OrderRequests::CancelAllOrders(cancel_all_orders) => {
                println!("Cancel All Orders: {:?}", cancel_all_orders);
                let user_id = cancel_all_orders.user_id.clone();
                let pubsub_id = match cancel_all_orders.pubsub_id {
                    Some(id) => id.to_string(),
                    None => {
                        tracing::warn!("CancelAllOrders missing pubsub_id, cannot respond");
                        return;
                    }
                };

                let response = {
                    let mut engine_guard = engine.lock().await;
                    match engine_guard.cancel_all_orders(cancel_all_orders) {
                        Ok(_) => {
                            println!("Successfully cancelled all orders!");
                            serde_json::json!({
                                "status": "Cancelled All Orders",
                                "user_id": user_id,
                            })
                        }
                        Err(err) => {
                            println!("Order cancellation failed - {}", err);
                            serde_json::json!({
                                "status": "Failed to Cancel All Orders",
                            })
                        }
                    }
                };

                if let Ok(response_string) = serde_json::to_string(&response) {
                    let _ = pg_queue.notify.notify(&pubsub_id, &response_string).await;
                }
            }

            OrderRequests::GetDepth(depth) => {
                println!("Get Depth: {:?}", depth);
                let pubsub_id = match depth.pubsub_id {
                    Some(id) => id.to_string(),
                    None => {
                        tracing::warn!("GetDepth missing pubsub_id, cannot respond");
                        return;
                    }
                };

                let depth_result = {
                    let engine_guard = engine.lock().await;
                    engine_guard.get_depth(depth)
                };

                let depth_json = serde_json::json!({
                    "bids": depth_result.0,
                    "asks": depth_result.1,
                });

                if let Ok(response_string) = serde_json::to_string(&depth_json) {
                    let _ = pg_queue.notify.notify(&pubsub_id, &response_string).await;
                }
                println!("Successfully retrieved depth!");
            }
        },
        Err(err) => {
            println!("Failed to deserialize order request: {:?}", err);
        }
    }
}
