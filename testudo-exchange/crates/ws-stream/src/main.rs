// @anchor exchange:ws-stream:main
// @tags infra

use futures_util::StreamExt;
use pg_queue::ListenerService;
use sqlx_postgres::PostgresDb;
use std::io::Error;
use std::sync::Arc;
use std::time::Duration;
use tokio::net::{TcpListener, TcpStream};
use tokio::sync::Mutex;
use tokio_tungstenite::tungstenite::Error as WsError;
use types::WsMessage;

pub mod pg_ws_manager;
pub mod types;
pub mod user;
use pg_ws_manager::PgWsManager;
use user::User;

#[tokio::main]
async fn main() -> Result<(), Error> {
    dotenvy::dotenv().ok();
    let addr = std::env::var("WS_STREAM_URL").expect("WS_STREAM_URL must be set");

    // Connect to PostgreSQL
    let postgres = PostgresDb::new()
        .await
        .expect("Failed to connect to PostgreSQL");
    let pg_pool = postgres.get_pg_connection().expect("Failed to get PG pool");
    println!("PostgreSQL connection pool ready!");

    // Create the event loop and TCP listener we'll accept connections on.
    let try_socket = TcpListener::bind(&addr).await;
    let listener = try_socket.expect("Failed to bind");

    let ws_manager = Arc::new(Mutex::new(PgWsManager::new(pg_pool.clone())));

    // Create a listener for PostgreSQL NOTIFY
    let pg_listener = Arc::new(Mutex::new(
        ListenerService::new(&pg_pool)
            .await
            .expect("Failed to create PG listener"),
    ));

    // Spawn a task to process PostgreSQL NOTIFY messages
    let ws_manager_clone = ws_manager.clone();
    let pg_listener_clone = pg_listener.clone();
    tokio::spawn(async move {
        process_pg_notifications(ws_manager_clone, pg_listener_clone).await;
    });

    // Accept new connections in a loop
    while let Ok((stream, _)) = listener.accept().await {
        // 017 FR-6: Disable Nagle's algorithm for lower-latency frame delivery
        if let Err(e) = stream.set_nodelay(true) {
            tracing::warn!("Failed to set TCP_NODELAY: {}", e);
        }
        let ws_manager_clone = ws_manager.clone();
        let pg_listener_clone = pg_listener.clone();
        tokio::spawn(accept_connection(
            stream,
            ws_manager_clone,
            pg_listener_clone,
        ));
    }

    Ok(())
}

async fn accept_connection(
    stream: TcpStream,
    ws_manager: Arc<Mutex<PgWsManager>>,
    pg_listener: Arc<Mutex<ListenerService>>,
) {
    let user_addr = stream
        .peer_addr()
        .expect("connected streams should have a peer address");

    let ws_stream = tokio_tungstenite::accept_async(stream)
        .await
        .expect("Error during the websocket handshake occurred");

    let (write, mut read) = ws_stream.split();

    // Add a user whenever someone connects
    let user = User::new(user_addr.to_string(), write);

    // Lock the manager just long enough to add the user - to reduce the time the lock is held
    {
        let mut manager = ws_manager.lock().await;
        manager.add_user(user);
    }

    println!("Connection established with addr: {}", user_addr);

    while let Some(msg_result) = read.next().await {
        match msg_result {
            Ok(msg) => {
                if msg.is_text() {
                    let msg_text = msg.to_text().unwrap();
                    if let Ok(data) = serde_json::from_str::<WsMessage>(msg_text) {
                        process_data(
                            data,
                            &user_addr.to_string(),
                            ws_manager.clone(),
                            pg_listener.clone(),
                        )
                        .await;
                    }
                } else if msg.is_close() {
                    println!("Closing Connection to user with addr: {}", user_addr);
                    let mut manager = ws_manager.lock().await;
                    let unlisten_channels = manager.remove_user(&user_addr.to_string());
                    drop(manager);
                    // AUD-03 FR-9: Issue UNLISTEN for channels with no remaining subscribers
                    if !unlisten_channels.is_empty() {
                        let mut listener = pg_listener.lock().await;
                        for channel in &unlisten_channels {
                            let _ = listener.unlisten(channel).await;
                        }
                    }
                    break;
                }
            }
            Err(e) => {
                match e {
                    WsError::Protocol(protocol_err) => {
                        println!(
                            "WebSocket protocol error from {}: {:?}",
                            user_addr, protocol_err
                        );
                    }
                    WsError::ConnectionClosed | WsError::AlreadyClosed => {
                        println!("WebSocket connection closed from {}: {:?}", user_addr, e);
                    }
                    _ => {
                        println!("WebSocket error from {}: {:?}", user_addr, e);
                    }
                }
                // Remove user and break the loop on error
                let mut manager = ws_manager.lock().await;
                let unlisten_channels = manager.remove_user(&user_addr.to_string());
                drop(manager);
                // AUD-03 FR-9: Issue UNLISTEN for channels with no remaining subscribers
                if !unlisten_channels.is_empty() {
                    let mut listener = pg_listener.lock().await;
                    for channel in &unlisten_channels {
                        let _ = listener.unlisten(channel).await;
                    }
                }
                break;
            }
        }
    }
}

async fn process_data(
    data: WsMessage,
    user_addr: &str,
    ws_manager: Arc<Mutex<PgWsManager>>,
    pg_listener: Arc<Mutex<ListenerService>>,
) {
    let mut manager = ws_manager.lock().await;

    match data.method.as_str() {
        "SUBSCRIBE" => {
            // Subscribe returns the channel if we need to start listening
            if let Some(channel) = manager.subscribe(user_addr, data) {
                // Start listening to this new channel
                let mut listener = pg_listener.lock().await;
                if let Err(e) = listener.listen(&channel).await {
                    tracing::error!(channel = %channel, error = ?e, "Failed to LISTEN");
                } else {
                    println!("Now listening on channel: {}", channel);
                }
            }
        }
        "UNSUBSCRIBE" => {
            // Unsubscribe returns the channel if we should stop listening
            if let Some(channel) = manager.unsubscribe(user_addr, data) {
                // Stop listening to this channel
                let mut listener = pg_listener.lock().await;
                if let Err(e) = listener.unlisten(&channel).await {
                    tracing::error!(channel = %channel, error = ?e, "Failed to UNLISTEN");
                } else {
                    println!("Stopped listening on channel: {}", channel);
                }
            }
        }
        _ => {}
    }
}

async fn process_pg_notifications(
    ws_manager: Arc<Mutex<PgWsManager>>,
    pg_listener: Arc<Mutex<ListenerService>>,
) {
    println!("Listening for PostgreSQL NOTIFY messages");

    loop {
        // Try to receive a notification
        let notification = {
            let mut listener = pg_listener.lock().await;
            listener.recv_timeout(Duration::from_millis(100)).await
        };

        match notification {
            Ok(Some(notif)) => {
                // Route the message to subscribed users
                let mut manager = ws_manager.lock().await;
                manager
                    .send_to_ws_stream(&notif.channel, notif.payload)
                    .await;
            }
            Ok(None) => {
                // Timeout, continue polling
            }
            Err(e) => {
                tracing::error!(error = ?e, "Error receiving notification");
                tokio::time::sleep(Duration::from_millis(100)).await;
            }
        }
    }
}
