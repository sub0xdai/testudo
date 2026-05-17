//! PostgreSQL-based WebSocket Manager
//!
//! Uses LISTEN/NOTIFY for pub/sub instead of Redis.

use futures_util::SinkExt;
use pg_queue::{ListenerService, PgPool};
use tokio_tungstenite::tungstenite::Message;

use crate::{
    types::{WsMessage, WsResponse},
    user::User,
};
use std::collections::{HashMap, HashSet};

pub struct PgWsManager {
    pub users: HashMap<String, User>,
    pub subscriptions: HashMap<String, Vec<String>>, // user_id -> [subscription_id]
    pub reverse_subscriptions: HashMap<String, Vec<String>>, // subscription_id -> [user_id]
    pub active_channels: HashSet<String>,            // Channels we're currently listening to
    pub pool: PgPool,
}

impl PgWsManager {
    pub fn new(pool: PgPool) -> Self {
        Self {
            users: HashMap::new(),
            subscriptions: HashMap::new(),
            reverse_subscriptions: HashMap::new(),
            active_channels: HashSet::new(),
            pool,
        }
    }

    pub fn add_user(&mut self, user: User) {
        self.users.insert(user.id.clone(), user);
    }

    /// Remove a user and clean up all subscription state.
    /// Returns channels that should be UNLISTENed (no remaining subscribers).
    pub fn remove_user(&mut self, id: &str) -> Vec<String> {
        self.users.remove(id);
        let mut unlisten_channels = Vec::new();

        if let Some(channels) = self.subscriptions.remove(id) {
            for channel in &channels {
                if let Some(subscribers) = self.reverse_subscriptions.get_mut(channel) {
                    subscribers.retain(|uid| uid != id);
                    if subscribers.is_empty() {
                        self.reverse_subscriptions.remove(channel);
                        self.active_channels.remove(channel);
                        unlisten_channels.push(channel.clone());
                    }
                }
            }
        }

        unlisten_channels
    }

    /// Get the channels that need to be listened to
    pub fn get_active_channels(&self) -> &HashSet<String> {
        &self.active_channels
    }

    /// Subscribe a user to a channel
    /// Returns true if this is a new channel that needs LISTEN
    pub fn subscribe(&mut self, user_id: &str, message: WsMessage) -> Option<String> {
        if message.method != "SUBSCRIBE" {
            return None;
        }

        let (subscription_type, topic) = match message.parse_subscription() {
            Some(result) => result,
            None => {
                tracing::warn!(params = ?message.params, "Invalid subscription format");
                return None;
            }
        };

        let subscription_id = format!("{:?}.{}", subscription_type, topic);

        if let Some(subscriptions) = self.subscriptions.get_mut(user_id) {
            subscriptions.push(subscription_id.clone());
        } else {
            self.subscriptions
                .insert(user_id.to_string(), vec![subscription_id.clone()]);
        }

        if let Some(users) = self.reverse_subscriptions.get_mut(&subscription_id) {
            users.push(user_id.to_string());
            None // Channel already being listened to
        } else {
            self.reverse_subscriptions
                .insert(subscription_id.clone(), vec![user_id.to_string()]);
            self.active_channels.insert(subscription_id.clone());
            Some(subscription_id) // New channel to listen to
        }
    }

    /// Unsubscribe a user from a channel
    /// Returns Some(channel) if we should UNLISTEN
    pub fn unsubscribe(&mut self, user_id: &str, message: WsMessage) -> Option<String> {
        if message.method != "UNSUBSCRIBE" {
            return None;
        }

        let (subscription_type, topic) = match message.parse_subscription() {
            Some(result) => result,
            None => {
                tracing::warn!(params = ?message.params, "Invalid unsubscription format");
                return None;
            }
        };

        let subscription_id = format!("{:?}.{}", subscription_type, topic);

        if let Some(subscriptions) = self.subscriptions.get_mut(user_id) {
            subscriptions.retain(|id| id != &subscription_id);
        }

        if let Some(users) = self.reverse_subscriptions.get_mut(&subscription_id) {
            users.retain(|id| id != user_id);

            if users.is_empty() {
                self.reverse_subscriptions.remove(&subscription_id);
                self.active_channels.remove(&subscription_id);
                return Some(subscription_id); // Should UNLISTEN
            }
        }

        None
    }

    /// Send a message to all subscribed users
    pub async fn send_to_ws_stream(&mut self, channel: &str, message: String) {
        // Try to parse as WsResponse for stream matching
        if let Ok(ws_message) = serde_json::from_str::<WsResponse>(&message) {
            if let Some(users) = self.reverse_subscriptions.get(&ws_message.stream) {
                for user_id in users.clone() {
                    if let Some(user) = self.users.get_mut(&user_id) {
                        let _ = user.ws_stream.send(Message::Text(message.clone())).await;
                    }
                }
            }
        } else {
            // Fall back to channel-based matching
            if let Some(users) = self.reverse_subscriptions.get(channel) {
                for user_id in users.clone() {
                    if let Some(user) = self.users.get_mut(&user_id) {
                        let _ = user.ws_stream.send(Message::Text(message.clone())).await;
                    }
                }
            }
        }
    }
}

/// Creates a listener that dynamically subscribes/unsubscribes to channels
pub async fn create_dynamic_listener(
    pool: &PgPool,
) -> Result<ListenerService, pg_queue::PgQueueError> {
    ListenerService::new(pool).await
}
