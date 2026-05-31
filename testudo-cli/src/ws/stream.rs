// @anchor infra:cli:ws:stream
// @tags api

//! Event stream — mpsc-backed async iterator of WebSocket events.

pub use common_utils::{AgentAlert, AlertSeverity, AlertType, ExecutionReport};
use serde_json::Value;
use tokio::sync::mpsc;

/// Events received from the ws-stream service.
#[derive(Debug, Clone)]
pub enum WsEvent {
    /// Agent alert (risk breach, drawdown warning, etc.)
    Alert(AgentAlert),
    /// Trade execution report
    Execution(ExecutionReport),
    /// Unrecognized stream or data format
    Unknown(String, Value),
    /// Stream permanently disconnected (max retries exhausted)
    Disconnected,
}

/// Async event stream wrapping an mpsc receiver.
///
/// Created by `WsClient::connect()`. Call `recv()` in a loop to receive events.
/// Returns `None` when the stream is permanently closed (max retries exhausted
/// or the sender is dropped).
pub struct EventStream {
    rx: mpsc::UnboundedReceiver<WsEvent>,
}

impl EventStream {
    pub(crate) fn new(rx: mpsc::UnboundedReceiver<WsEvent>) -> Self {
        Self { rx }
    }

    /// Receive the next event, or None if the stream has ended.
    pub async fn recv(&mut self) -> Option<WsEvent> {
        self.rx.recv().await
    }
}
