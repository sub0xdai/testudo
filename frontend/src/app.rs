use leptos::*;
use leptos_router::*;

#[component]
pub fn App() -> impl IntoView {
    view! {
        <Router>
            <div class="min-h-screen bg-terminal-bg text-text-primary font-sans">
                <Routes>
                    <Route path="" view=TradingTerminal/>
                </Routes>
            </div>
        </Router>
    }
}

#[component]
fn TradingTerminal() -> impl IntoView {
    view! {
        <div class="terminal-grid h-screen">
            // Header Bar
            <header class="terminal-header bg-terminal-panel border-b border-terminal-border p-4">
                <div class="flex items-center justify-between">
                    <h1 class="text-roman-gold font-display text-xl font-bold">
                        "Testudo Command Center"
                    </h1>
                    <div class="flex items-center space-x-4 text-sm">
                        <span class="text-text-secondary">"Market Status: "</span>
                        <span class="text-green-400">"ACTIVE"</span>
                        <span class="text-text-secondary">"│"</span>
                        <span class="text-text-secondary">"Connection: "</span>
                        <span class="text-green-400">"CONNECTED"</span>
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
                            <div class="grid grid-cols-2 gap-2 text-sm">
                                <div class="text-text-secondary">"Symbol:"</div>
                                <div class="text-text-primary font-mono">"BTC/USDT"</div>
                                <div class="text-text-secondary">"Price:"</div>
                                <div class="text-text-primary font-mono">"$45,234.56"</div>
                                <div class="text-text-secondary">"Account:"</div>
                                <div class="text-text-primary font-mono">"$10,000.00"</div>
                            </div>
                            
                            <div class="border-t border-terminal-border pt-4">
                                <h3 class="text-text-secondary text-sm mb-2">"Van Tharp Position Sizing"</h3>
                                <div class="grid grid-cols-2 gap-2 text-sm">
                                    <div class="text-text-muted">"Entry:"</div>
                                    <div class="text-text-primary font-mono">"$45,234.56"</div>
                                    <div class="text-text-muted">"Stop:"</div>
                                    <div class="text-text-primary font-mono">"$43,000.00"</div>
                                    <div class="text-text-muted">"Risk per R:"</div>
                                    <div class="text-text-primary font-mono">"$200.00"</div>
                                    <div class="text-text-muted">"Position Size:"</div>
                                    <div class="text-green-400 font-mono font-bold">"0.0895 BTC"</div>
                                </div>
                            </div>

                            <div class="border-t border-terminal-border pt-4">
                                <button class="w-full bg-green-600 hover:bg-green-700 text-white py-2 px-4 rounded font-medium transition-colors">
                                    "Execute Trade"
                                </button>
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