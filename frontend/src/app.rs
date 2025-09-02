// Frontend main app with Thaw UI configuration

use leptos::prelude::*;
use leptos::task::spawn_local;
use crate::components::auth::AuthProvider;
use crate::components::ui::{
    WebSocketService, WebSocketStatus, NotificationSystem, TradingNotifications, use_notification
};
use crate::components::layout::NavigationBar;
use crate::components::trading::{OrderForm, OrderData};
use thaw::{
    Button, ButtonAppearance, ButtonSize, Card, Flex, FlexAlign, FlexJustify, Space, SpaceGap, Tag, Icon, 
    ConfigProvider, Theme, Layout, Grid, GridItem
};
use icondata as i;

// Create custom Testudo Trading Terminal theme
fn create_testudo_theme() -> RwSignal<Theme> {
    let theme = Theme::dark();
    
    // The theme will automatically use our CSS variables defined in globals.css
    // since we've mapped them to Thaw's expected variable names.
    // Our comprehensive CSS variable mapping ensures all Thaw components
    // will use our monochromatic trading terminal color scheme.
    
    RwSignal::new(theme)
}

#[component]
pub fn App() -> impl IntoView {
    // Create custom trading terminal theme with our CSS variables
    let theme = create_testudo_theme();
    
    view! {
        <ConfigProvider theme=theme>
            <style>
                {include_str!("../styles/thaw-custom.css")}
            </style>
            
            <NotificationSystem>
                <AuthProvider>
                    <WebSocketService>
                        <div class="terminal-bg min-h-screen text-text-primary">
                            <TradingTerminal />
                        </div>
                    </WebSocketService>
                </AuthProvider>
            </NotificationSystem>
        </ConfigProvider>
    }
}

#[component]
fn TradingTerminal() -> impl IntoView {
    // Responsive breakpoint logic
    let (is_mobile, set_is_mobile) = signal(false);
    
    // Check window size (simplified - in real app would use proper breakpoint detection)
    let check_mobile = move || {
        // This is a simplified version - would use proper media query detection
        false // For now, always desktop layout
    };

    view! {
        <Layout class="h-screen">
            // Navigation Header
            <Layout>
                <NavigationBar />
            </Layout>
            
            // Main Content Area with Responsive Grid
            <Layout class="flex-1">
                <Grid cols=12 x_gap=Signal::derive(|| 16) y_gap=Signal::derive(|| 16) class="h-full p-4">
                    // Chart Area - 9 cols on desktop, 12 on mobile
                    <GridItem 
                        column=Signal::derive(move || if is_mobile.get() { 12 } else { 9 })
                        class="h-full"
                    >
                        <Card class="chart-container h-full">
                            <div class="panel-header border-b p-3">
                                <Flex justify=FlexJustify::SpaceBetween align=FlexAlign::Center>
                                    <h2 class="text-roman-gold font-medium">"Market Chart"</h2>
                                    <Space>
                                        <span class="text-text-secondary">"BTC/USDT"</span>
                                        <span class="font-mono font-semibold">"$45,234.56"</span>
                                        <Tag>"+2.34%"</Tag>
                                        <WebSocketStatus />
                                    </Space>
                                </Flex>
                            </div>
                            <div class="h-full relative flex items-center justify-center p-8">
                                <div class="text-center">
                                    <div class="w-16 h-16 border-2 border-roman-gold border-t-transparent rounded-full animate-spin mx-auto mb-4"></div>
                                    <p class="text-text-secondary mb-2">"TradingView Chart Loading..."</p>
                                    <p class="text-text-muted text-sm">"Pure Thaw UI Layout Active"</p>
                                </div>
                                <FloatingExecutionButtons />
                            </div>
                        </Card>
                    </GridItem>
                    
                    // Order Panel - 3 cols desktop, hidden on mobile
                    <Show when=move || !is_mobile.get()>
                        <GridItem column=3 class="h-full">
                            <OrderFormWithNotifications />
                        </GridItem>
                    </Show>
                </Grid>
            </Layout>
            
            // Status Panel (Bottom)
            <Layout class="border-t">
                <StatusPanel />
            </Layout>
            
            // Mobile Navigation (Conditional)
            <Show when=move || is_mobile.get()>
                <MobileNavigation />
            </Show>
        </Layout>
    }
}

#[component]
fn FloatingExecutionButtons() -> impl IntoView {
    let (execution_mode, set_execution_mode) = signal(Option::<String>::None);

    view! {
        <div class="absolute bottom-6 right-6 z-10">
            <Space vertical=true gap=SpaceGap::Small>
                <Button
                    appearance=ButtonAppearance::Primary
                    size=ButtonSize::Large
                    class="long-button animate-element hover:shadow-[0_0_20px_rgba(0,255,133,0.3)] active:scale-[0.98] transition-all duration-200"
                    on_click=move |_| set_execution_mode.set(Some("LONG".to_string()))
                >
                    <Icon icon=i::AiArrowUpOutlined />
                    " LONG"
                </Button>
                <Button
                    appearance=ButtonAppearance::Primary
                    size=ButtonSize::Large
                    class="short-button animate-element hover:shadow-[0_0_20px_rgba(255,0,102,0.3)] active:scale-[0.98] transition-all duration-200"
                    on_click=move |_| set_execution_mode.set(Some("SHORT".to_string()))
                >
                    <Icon icon=i::AiArrowDownOutlined />
                    " SHORT"
                </Button>
            </Space>
        </div>

        <Show when=move || execution_mode.get().is_some()>
            <Card class="absolute top-4 left-4 z-20">
                <Space vertical=true gap=SpaceGap::Small>
                    <div class="text-roman-gold text-sm font-medium">
                        "Execution Mode: " {move || execution_mode.get().unwrap_or_default()}
                    </div>
                    <Button
                        appearance=ButtonAppearance::Subtle
                        size=ButtonSize::Small
                        on_click=move |_| set_execution_mode.set(None)
                    >
                        "Cancel"
                    </Button>
                </Space>
            </Card>
        </Show>
    }
}

// OrderPanel component replaced by OrderForm from components/trading/order_form.rs

#[component]
fn OrderFormWithNotifications() -> impl IntoView {
    let notification = use_notification();
    
    view! {
        <OrderForm 
            symbol=Signal::derive(|| "BTC/USDT".to_string())
            current_price=Signal::derive(|| 45234.56)
            account_balance=Signal::derive(|| 10000.0)
            on_submit_buy=Callback::new(move |order: OrderData| {
                // Simulate order execution  
                let notification_fn = notification.show;
                spawn_local(async move {
                    // Show executing notification
                    let executing = TradingNotifications::order_executed(
                        order.symbol.clone(),
                        "BUY".to_string(),
                        order.amount,
                    );
                    notification_fn(executing);
                });
            })
            on_submit_sell=Callback::new(move |order: OrderData| {
                // Simulate order execution
                let notification_fn = notification.show; 
                spawn_local(async move {
                    // Show executing notification
                    let executing = TradingNotifications::order_executed(
                        order.symbol.clone(),
                        "SELL".to_string(),
                        order.amount,
                    );
                    notification_fn(executing);
                });
            })
        />
    }
}

#[component]
fn StatusPanel() -> impl IntoView {
    view! {
        <Card class="w-full">
            <div class="panel-header border-b p-3">
                <h2 class="text-roman-gold font-medium">"Positions & System Status"</h2>
            </div>
            <div class="p-4">
                <Grid cols=4 x_gap=Signal::derive(|| 16) y_gap=Signal::derive(|| 16)>
                    // Open Positions
                    <GridItem>
                        <Card class="hover-lift h-full">
                            <div class="p-3">
                                <Space vertical=true gap=SpaceGap::Small>
                                    <h3 class="font-medium text-sm">"Open Positions"</h3>
                                    <Space vertical=true gap=SpaceGap::Small>
                                        <Flex justify=FlexJustify::SpaceBetween align=FlexAlign::Center>
                                            <span class="font-mono text-xs">"BTC/USDT"</span>
                                            <Tag class="profit-glow text-xs">"+2.4%"</Tag>
                                        </Flex>
                                        <Flex justify=FlexJustify::SpaceBetween align=FlexAlign::Center>
                                            <span class="font-mono text-xs">"ETH/USDT"</span>
                                            <Tag class="loss-glow text-xs">"-1.2%"</Tag>
                                        </Flex>
                                    </Space>
                                </Space>
                            </div>
                        </Card>
                    </GridItem>
                    
                    // OODA Status
                    <GridItem>
                        <Card class="hover-lift h-full">
                            <div class="p-3">
                                <Space vertical=true gap=SpaceGap::Small>
                                    <h3 class="font-medium text-sm">"OODA Status"</h3>
                                    <Space vertical=true gap=SpaceGap::Small>
                                        <Flex justify=FlexJustify::SpaceBetween align=FlexAlign::Center>
                                            <span class="text-xs">"Observe"</span>
                                            <Tag class="status-online text-xs">"Active"</Tag>
                                        </Flex>
                                        <Flex justify=FlexJustify::SpaceBetween align=FlexAlign::Center>
                                            <span class="text-xs">"Orient"</span>
                                            <Tag class="status-online text-xs">"Ready"</Tag>
                                        </Flex>
                                    </Space>
                                </Space>
                            </div>
                        </Card>
                    </GridItem>
                    
                    // System Health
                    <GridItem>
                        <Card class="hover-lift h-full">
                            <div class="p-3">
                                <Space vertical=true gap=SpaceGap::Small>
                                    <h3 class="font-medium text-sm">"System Health"</h3>
                                    <Space vertical=true gap=SpaceGap::Small>
                                        <Flex justify=FlexJustify::SpaceBetween align=FlexAlign::Center>
                                            <span class="text-xs">"API"</span>
                                            <Tag class="status-online text-xs">"Connected"</Tag>
                                        </Flex>
                                        <Flex justify=FlexJustify::SpaceBetween align=FlexAlign::Center>
                                            <span class="text-xs">"WebSocket"</span>
                                            <Tag class="status-online text-xs">"Live"</Tag>
                                        </Flex>
                                    </Space>
                                </Space>
                            </div>
                        </Card>
                    </GridItem>
                    
                    // Performance
                    <GridItem>
                        <Card class="hover-lift h-full">
                            <div class="p-3">
                                <Space vertical=true gap=SpaceGap::Small>
                                    <h3 class="font-medium text-sm">"Performance"</h3>
                                    <Space vertical=true gap=SpaceGap::Small>
                                        <Flex justify=FlexJustify::SpaceBetween align=FlexAlign::Center>
                                            <span class="text-xs">"Latency"</span>
                                            <Tag class="text-xs">"<50ms"</Tag>
                                        </Flex>
                                        <Flex justify=FlexJustify::SpaceBetween align=FlexAlign::Center>
                                            <span class="text-xs">"Uptime"</span>
                                            <Tag class="text-xs">"99.9%"</Tag>
                                        </Flex>
                                    </Space>
                                </Space>
                            </div>
                        </Card>
                    </GridItem>
                </Grid>
            </div>
        </Card>
    }
}

#[component]
fn MobileNavigation() -> impl IntoView {
    view! {
        <div class="mobile-nav fixed bottom-0 left-0 right-0 p-2">
            <Card>
                <Flex justify=FlexJustify::SpaceAround align=FlexAlign::Center class="py-2">
                    <Button appearance=ButtonAppearance::Transparent size=ButtonSize::Small>
                        <Icon icon=i::AiHomeOutlined />
                    </Button>
                    <Button appearance=ButtonAppearance::Transparent size=ButtonSize::Small>
                        <Icon icon=i::AiLineChartOutlined />
                    </Button>
                    <Button appearance=ButtonAppearance::Transparent size=ButtonSize::Small>
                        <Icon icon=i::AiWalletOutlined />
                    </Button>
                    <Button appearance=ButtonAppearance::Transparent size=ButtonSize::Small>
                        <Icon icon=i::AiUserOutlined />
                    </Button>
                </Flex>
            </Card>
        </div>
    }
}

#[component]
fn LoginPage() -> impl IntoView {
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
                        <Button
                            appearance=ButtonAppearance::Primary
                            size=ButtonSize::Large
                            class="w-full"
                        >
                            "🔐 Login with Keycloak"
                        </Button>
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
