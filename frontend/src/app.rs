use leptos::prelude::*;
use leptos_router::{components::*, hooks::*, *};
use leptos::ev::*;
use crate::components::auth::{AuthProvider, AuthStatus, ProtectedRoute, RoutePermission, MinimumRiskProfile};
use crate::components::ui::{WebSocketService, WebSocketStatus, use_market_data};
use crate::components::trading::VanTharpCalculator;

#[component]
pub fn App() -> impl IntoView {
    view! {
        <AuthProvider>
            <WebSocketService>
                <Router>
                    <div class="min-h-screen bg-terminal-bg text-text-primary font-sans">
                        <Routes>
                            <Route path="" view=TradingTerminal/>
                            <Route path="/login" view=LoginPage/>
                        </Routes>
                    </div>
                </Router>
            </WebSocketService>
        </AuthProvider>
    }
}

#[component]
fn TradingTerminal() -> impl IntoView {
    // Sample trading data - in production, this would come from WebSocket/API
    let (symbol, _set_symbol) = signal("BTC/USDT".to_string());
    let (current_price, _set_current_price) = signal(45234.56);
    let (stop_loss_price, set_stop_loss_price) = signal(43000.0);
    
    view! {
        <div class="terminal-grid h-screen">
            // Header Bar
            <header class="terminal-header bg-terminal-panel border-b border-terminal-border p-4">
                <div class="flex items-center justify-between">
                    <h1 class="text-roman-gold font-display text-xl font-bold">
                        "Testudo Command Center"
                    </h1>
                    <div class="flex items-center space-x-6 text-sm">
                        <div class="flex items-center space-x-4">
                            <span class="text-text-secondary">"Market Status: "</span>
                            <span class="text-green-400">"ACTIVE"</span>
                            <span class="text-text-secondary">"│"</span>
                            <WebSocketStatus />
                        </div>
                        <div class="border-l border-terminal-border pl-6">
                            <AuthStatus />
                        </div>
                    </div>
                </div>
            </header>

            // Main Trading Area - Three Panel Layout
            <main class="terminal-main">
                // Central Chart Panel
                <section class="chart-panel bg-terminal-card border border-terminal-border">
                    <div class="panel-header border-b border-terminal-border p-3">
                        <h2 class="text-text-primary font-medium">"Market Chart"</h2>
                    </div>
                    <div class="chart-container p-4 h-full flex items-center justify-center">
                        <div class="text-center">
                            <div class="w-16 h-16 border-2 border-roman-gold border-t-transparent rounded-full animate-spin mx-auto mb-4"></div>
                            <p class="text-text-secondary">"TradingView Chart Loading..."</p>
                            <p class="text-text-muted text-sm mt-2">"Phase 2: Core Shell Complete"</p>
                        </div>
                    </div>
                </section>

                // Right Order Panel
                <section class="order-panel bg-terminal-card border border-terminal-border">
                    <div class="panel-header border-b border-terminal-border p-3">
                        <h2 class="text-text-primary font-medium">"Order Entry"</h2>
                    </div>
                    <div class="order-content p-4">
                        <div class="space-y-4">
                            <div class="grid grid-cols-2 gap-2 text-sm mb-4">
                                <div class="text-text-secondary">"Symbol:"</div>
                                <div class="text-text-primary font-mono">{move || symbol.get()}</div>
                                <div class="text-text-secondary">"Price:"</div>
                                <div class="text-text-primary font-mono">{move || format!("${:.2}", current_price.get())}</div>
                                <div class="text-text-secondary">"Stop Loss:"</div>
                                <div class="text-text-primary font-mono">
                                    <input 
                                        type="number" 
                                        step="0.01"
                                        value={move || stop_loss_price.get()}
                                        on:input=move |ev| {
                                            if let Ok(value) = event_target_value(&ev).parse::<f64>() {
                                                set_stop_loss_price.set(value);
                                            }
                                        }
                                        class="bg-terminal-bg border border-terminal-border rounded px-2 py-1 text-text-primary font-mono text-sm w-full"
                                    />
                                </div>
                            </div>
                            
                            <div class="border-t border-terminal-border pt-4">
                                <VanTharpCalculator
                                    symbol=symbol
                                    price=current_price
                                    stop_loss=stop_loss_price
                                />
                            </div>

                            <div class="border-t border-terminal-border pt-4">
                                <ProtectedRoute 
                                    required_permission=RoutePermission::ExecuteTrades
                                    minimum_risk_profile=MinimumRiskProfile::Standard
                                    show_loading=false
                                >
                                    <button class="w-full bg-green-600 hover:bg-green-700 text-white py-2 px-4 rounded font-medium transition-colors">
                                        "Execute Trade"
                                    </button>
                                </ProtectedRoute>
                            </div>
                        </div>
                    </div>
                </section>

                // Bottom Status Panel  
                <section class="status-panel bg-terminal-card border border-terminal-border">
                    <div class="panel-header border-b border-terminal-border p-3">
                        <h2 class="text-text-primary font-medium">"Positions & System Status"</h2>
                    </div>
                    <div class="status-content p-4">
                        <div class="grid grid-cols-4 gap-6">
                            // Open Positions
                            <div>
                                <h3 class="text-text-secondary text-sm mb-2">"Open Positions"</h3>
                                <div class="space-y-1 text-sm">
                                    <div class="flex justify-between">
                                        <span class="font-mono">"BTC/USDT"</span>
                                        <span class="text-green-400">"+2.4%"</span>
                                    </div>
                                    <div class="flex justify-between">
                                        <span class="font-mono">"ETH/USDT"</span>
                                        <span class="text-red-400">"-1.2%"</span>
                                    </div>
                                </div>
                            </div>

                            // OODA Loop Status
                            <div>
                                <h3 class="text-text-secondary text-sm mb-2">"OODA Status"</h3>
                                <div class="space-y-1 text-sm">
                                    <div class="flex justify-between">
                                        <span>"Observe:"</span>
                                        <span class="text-green-400">"ACTIVE"</span>
                                    </div>
                                    <div class="flex justify-between">
                                        <span>"Orient:"</span>
                                        <span class="text-green-400">"READY"</span>
                                    </div>
                                    <div class="flex justify-between">
                                        <span>"Decide:"</span>
                                        <span class="text-yellow-400">"PENDING"</span>
                                    </div>
                                    <div class="flex justify-between">
                                        <span>"Act:"</span>
                                        <span class="text-text-muted">"IDLE"</span>
                                    </div>
                                </div>
                            </div>

                            // System Health
                            <div>
                                <h3 class="text-text-secondary text-sm mb-2">"System Health"</h3>
                                <div class="space-y-1 text-sm">
                                    <div class="flex justify-between">
                                        <span>"API Latency:"</span>
                                        <span class="text-green-400">"45ms"</span>
                                    </div>
                                    <div class="flex justify-between">
                                        <span>"WS Stream:"</span>
                                        <span class="text-green-400">"LIVE"</span>
                                    </div>
                                    <div class="flex justify-between">
                                        <span>"Risk Engine:"</span>
                                        <span class="text-green-400">"ACTIVE"</span>
                                    </div>
                                    <div class="flex justify-between">
                                        <span>"Circuit Breaker:"</span>
                                        <span class="text-text-muted">"OFF"</span>
                                    </div>
                                </div>
                            </div>

                            // Performance
                            <div>
                                <h3 class="text-text-secondary text-sm mb-2">"Performance"</h3>
                                <div class="space-y-1 text-sm">
                                    <div class="flex justify-between">
                                        <span>"Daily P&L:"</span>
                                        <span class="text-green-400">"+$234.56"</span>
                                    </div>
                                    <div class="flex justify-between">
                                        <span>"Total R:"</span>
                                        <span class="text-green-400">"+1.2R"</span>
                                    </div>
                                    <div class="flex justify-between">
                                        <span>"Win Rate:"</span>
                                        <span class="text-text-primary">"68%"</span>
                                    </div>
                                    <div class="flex justify-between">
                                        <span>"Avg R:"</span>
                                        <span class="text-green-400">"2.4R"</span>
                                    </div>
                                </div>
                            </div>
                        </div>
                    </div>
                </section>
            </main>
        </div>
    }
}

/// Simple login page component
#[component]
fn LoginPage() -> impl IntoView {
    use crate::components::auth::use_auth;
    
    let auth = use_auth();
    
    view! {
        <div class="min-h-screen bg-terminal-bg flex items-center justify-center">
            <div class="max-w-md w-full bg-terminal-card border border-terminal-border rounded-lg p-8">
                <div class="text-center mb-8">
                    <h1 class="text-roman-gold font-display text-2xl font-bold mb-2">
                        "Testudo Trading Platform"
                    </h1>
                    <p class="text-text-secondary">
                        "Secure authentication required"
                    </p>
                </div>
                
                <div class="space-y-6">
                    <div class="text-center">
                        <button 
                            class="w-full bg-roman-gold hover:bg-yellow-600 text-terminal-bg font-medium py-3 px-6 rounded-lg transition-colors"
                            on:click=move |_| auth.login.set(())
                        >
                            "🔐 Login with Keycloak"
                        </button>
                    </div>
                    
                    <div class="border-t border-terminal-border pt-6 text-center text-sm text-text-muted">
                        <p>"Authentication follows Van Tharp risk management protocols."</p>
                        <p class="mt-2">"All trading operations require verified identity and risk profile."</p>
                    </div>
                </div>
            </div>
        </div>
    }
}