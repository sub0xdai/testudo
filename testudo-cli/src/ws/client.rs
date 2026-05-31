// @anchor infra:cli:ws:client
// @tags api

//! WebSocket client for the Testudo ws-stream service.
//!
//! Protocol: connect via tokio-tungstenite, send JSON-RPC-style SUBSCRIBE
//! messages, receive WsResponse `{"stream":"...", "data":{...}}` frames.
//! AgentAlert and ExecutionReport are parsed from the `data` field.

use crate::ws::stream::{AgentAlert, EventStream, ExecutionReport, WsEvent};
use futures_util::StreamExt;
use std::time::Duration;
use tokio::sync::mpsc;
use tokio_tungstenite::connect_async;

/// WebSocket client for the ws-stream service.
pub struct WsClient {
    ws_url: String,
    agent_key: String,
}

/// Accumulator for exponential backoff reconnection logic.
pub struct Backoff {
    initial: Duration,
    max: Duration,
    current: Duration,
    attempt: u32,
}

impl Backoff {
    pub fn new(initial_secs: u64, max_secs: u64) -> Self {
        let initial = Duration::from_secs(initial_secs);
        Self {
            initial,
            max: Duration::from_secs(max_secs),
            current: initial,
            attempt: 0,
        }
    }

    /// Duration to wait before the next retry, or None if max attempts reached.
    pub fn next_delay(&mut self, max_attempts: u32) -> Option<Duration> {
        if self.attempt >= max_attempts {
            return None;
        }
        let delay = self.current;
        self.current = std::cmp::min(self.current * 2, self.max);
        self.attempt += 1;
        Some(delay)
    }

    /// Reset backoff to initial state (after successful connection).
    pub fn reset(&mut self) {
        self.current = self.initial;
        self.attempt = 0;
    }

    pub fn attempt_count(&self) -> u32 {
        self.attempt
    }
}

impl Default for Backoff {
    fn default() -> Self {
        Self::new(1, 60)
    }
}

impl WsClient {
    pub fn new(ws_url: &str, agent_key: &str) -> Self {
        Self {
            ws_url: ws_url.trim_end_matches('/').to_string(),
            agent_key: agent_key.to_string(),
        }
    }

    pub fn ws_url(&self) -> &str {
        &self.ws_url
    }

    /// Connect to the ws-stream service and subscribe to the given channels.
    ///
    /// `channels`: e.g. `["agent.alert.<user_id>", "agent.execution.<user_id>"]`
    ///
    /// Returns an EventStream that yields WsEvent variants. The internal read
    /// task handles reconnection with exponential backoff.
    pub async fn connect(
        &self,
        channels: &[String],
    ) -> Result<EventStream, crate::api::types::ApiError> {
        let (tx, rx) = mpsc::unbounded_channel();
        let url = self.ws_url.clone();
        let channels = channels.to_vec();
        let _agent_key = self.agent_key.clone();
        let max_retries: u32 = 10;

        tokio::spawn(async move {
            let mut backoff = Backoff::default();

            loop {
                match Self::connect_and_read(&url, &channels, &tx).await {
                    Ok(()) => {
                        // Clean disconnect — stream finished normally
                        break;
                    }
                    Err(e) => {
                        eprintln!("WebSocket error: {}", e);

                        match backoff.next_delay(max_retries) {
                            Some(delay) => {
                                eprintln!(
                                    "Reconnecting in {}s (attempt {}/{})...",
                                    delay.as_secs(),
                                    backoff.attempt_count(),
                                    max_retries
                                );
                                tokio::time::sleep(delay).await;
                            }
                            None => {
                                eprintln!(
                                    "WebSocket permanently failed after {} attempts",
                                    max_retries
                                );
                                let _ = tx.send(WsEvent::Disconnected);
                                break;
                            }
                        }
                    }
                }
            }
        });

        Ok(EventStream::new(rx))
    }

    async fn connect_and_read(
        url: &str,
        channels: &[String],
        tx: &mpsc::UnboundedSender<WsEvent>,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let (ws_stream, _) = connect_async(url).await?;
        let (mut write, mut read) = ws_stream.split();

        // Send SUBSCRIBE for each channel
        for (i, channel) in channels.iter().enumerate() {
            let sub = serde_json::json!({
                "method": "SUBSCRIBE",
                "params": [channel],
                "id": i + 1,
            });
            let msg = tokio_tungstenite::tungstenite::Message::Text(sub.to_string());
            futures_util::SinkExt::send(&mut write, msg).await?;
        }

        // Read loop
        while let Some(msg) = read.next().await {
            match msg {
                Ok(tokio_tungstenite::tungstenite::Message::Text(text)) => {
                    if let Some(event) = Self::parse_frame(&text) {
                        if tx.send(event).is_err() {
                            break; // Receiver dropped
                        }
                    }
                }
                Ok(tokio_tungstenite::tungstenite::Message::Close(_)) => break,
                Err(e) => {
                    eprintln!("WebSocket read error: {}", e);
                    break;
                }
                _ => {}
            }
        }

        Ok(())
    }

    fn parse_frame(text: &str) -> Option<WsEvent> {
        let frame: serde_json::Value = serde_json::from_str(text).ok()?;
        let stream = frame.get("stream")?.as_str()?;
        let data = frame.get("data")?;

        if stream.starts_with("agent.alert") {
            if let Ok(alert) = serde_json::from_value::<AgentAlert>(data.clone()) {
                return Some(WsEvent::Alert(alert));
            }
        } else if stream.starts_with("agent.execution") {
            if let Ok(report) = serde_json::from_value::<ExecutionReport>(data.clone()) {
                return Some(WsEvent::Execution(report));
            }
        }

        Some(WsEvent::Unknown(stream.to_string(), data.clone()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn backoff_starts_at_one_second() {
        let mut b = Backoff::new(1, 60);
        let delay = b.next_delay(10);
        assert_eq!(delay, Some(Duration::from_secs(1)));
    }

    #[test]
    fn backoff_doubles() {
        let mut b = Backoff::new(1, 60);
        assert_eq!(b.next_delay(10), Some(Duration::from_secs(1)));
        assert_eq!(b.next_delay(10), Some(Duration::from_secs(2)));
        assert_eq!(b.next_delay(10), Some(Duration::from_secs(4)));
        assert_eq!(b.next_delay(10), Some(Duration::from_secs(8)));
    }

    #[test]
    fn backoff_caps_at_max() {
        let mut b = Backoff::new(1, 3);
        assert_eq!(b.next_delay(10), Some(Duration::from_secs(1)));
        assert_eq!(b.next_delay(10), Some(Duration::from_secs(2)));
        // 2 * 2 = 4, capped at 3
        assert_eq!(b.next_delay(10), Some(Duration::from_secs(3)));
        // Stays at max
        assert_eq!(b.next_delay(10), Some(Duration::from_secs(3)));
    }

    #[test]
    fn backoff_resets() {
        let mut b = Backoff::new(1, 60);
        b.next_delay(10);
        b.next_delay(10);
        b.reset();
        assert_eq!(b.next_delay(10), Some(Duration::from_secs(1)));
    }

    #[test]
    fn backoff_exhausts_attempts() {
        let mut b = Backoff::new(1, 60);
        // Consume all 3 allowed attempts
        b.next_delay(3); // 1
        b.next_delay(3); // 2
        b.next_delay(3); // 3
        assert_eq!(b.next_delay(3), None);
    }

    #[test]
    fn ws_client_builds_from_url_and_key() {
        let client = WsClient::new("ws://localhost:8081", "testudo_sk_abc");
        assert_eq!(client.ws_url(), "ws://localhost:8081");
    }

    #[test]
    fn ws_client_strips_trailing_slash() {
        let client = WsClient::new("ws://localhost:8081/", "key");
        assert_eq!(client.ws_url(), "ws://localhost:8081");
    }
}
