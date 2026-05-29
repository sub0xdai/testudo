// @anchor exchange:router:depth
// @tags api

use actix_web::web::Data;
use pg_queue::QueueName;
use std::time::{Duration, Instant};
use uuid::Uuid;

use crate::types::{
    app::AppState,
    routes::{GetDepthInput, OrderRequests},
};

pub async fn get_depth(
    query: actix_web::web::Query<GetDepthInput>,
    app_state: Data<AppState>,
) -> actix_web::HttpResponse {
    let starttime = Instant::now();
    let mut market_data = query.into_inner();
    let pubsub_id = Uuid::new_v4();
    market_data.pubsub_id = Some(pubsub_id);

    let get_depth_request = OrderRequests::GetDepth(market_data);
    println!("Get Depth: {:?}", get_depth_request);

    // Use PostgreSQL request-response pattern
    let pg_queue = &app_state.pg_queue;

    match pg_queue
        .request_response
        .push_and_wait::<OrderRequests, serde_json::Value>(
            QueueName::Orders,
            &get_depth_request,
            Duration::from_secs(5),
        )
        .await
    {
        Ok(response) => {
            println!("Time: {:?}", starttime.elapsed());
            actix_web::HttpResponse::Ok().json(response)
        }
        Err(e) => {
            tracing::error!("Failed to get depth: {:?}", e);
            println!("Time: {:?}", starttime.elapsed());
            actix_web::HttpResponse::InternalServerError()
                .json(serde_json::json!({"error": "queue_error", "message": "Failed to fetch depth data"}))
        }
    }
}
