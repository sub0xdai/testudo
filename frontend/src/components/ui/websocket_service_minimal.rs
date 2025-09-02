use leptos::prelude::*;
use thaw::Tag;

/// Minimal WebSocket service for Phase 1
#[component]
pub fn WebSocketService(children: Children) -> impl IntoView {
    view! {
        <div class="websocket-service">
            {children()}
        </div>
    }
}

/// Minimal WebSocket status component
#[component]
pub fn WebSocketStatus() -> impl IntoView {
    view! {
        <Tag>"Connected"</Tag>
    }
}

/// Mock market data hook
pub fn use_market_data() -> ReadSignal<Option<()>> {
    // Corrected: Use the explicit `RwSignal` and get the read-only part.
    RwSignal::new(None).read_only()
}

/// Mock websocket sender hook
pub fn use_websocket_sender() -> ReadSignal<Option<()>> {
    // Corrected: Use the explicit `RwSignal` and get the read-only part.
    RwSignal::new(None).read_only()
}
