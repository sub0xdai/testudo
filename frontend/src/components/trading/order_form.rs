use leptos::prelude::*;
use thaw::{Card, Space, SpaceGap, Flex, FlexAlign, FlexJustify, Button, ButtonSize, Tag, Icon};
use icondata as i;
use crate::components::ui::trading_button::TradingButtonGroup;
use crate::components::ui::{VanTharpTooltip, RMultipleTooltip, RiskPercentTooltip};

#[derive(Clone, Debug, PartialEq)]
pub enum OrderType {
    Market,
    Limit,
    Stop,
    StopLimit,
}

impl std::fmt::Display for OrderType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            OrderType::Market => write!(f, "Market"),
            OrderType::Limit => write!(f, "Limit"),
            OrderType::Stop => write!(f, "Stop"),
            OrderType::StopLimit => write!(f, "Stop-Limit"),
        }
    }
}

#[component]
pub fn OrderForm(
    #[prop(into)] symbol: Signal<String>,
    #[prop(into)] current_price: Signal<f64>,
    #[prop(into)] account_balance: Signal<f64>,
    #[prop(into, optional)] on_submit_buy: Option<Callback<OrderData>>,
    #[prop(into, optional)] on_submit_sell: Option<Callback<OrderData>>,
) -> impl IntoView {
    let (order_type, set_order_type) = signal(OrderType::Market);
    let (amount, set_amount) = signal(0.0);
    let (price, set_price) = signal(0.0);
    let (stop_price, set_stop_price) = signal(0.0);
    
    let (is_executing, set_is_executing) = signal(false);
    
    // Van Tharp calculations
    let position_size = Signal::derive(move || {
        let account = account_balance.get();
        let risk_percent = 0.025; // 2.5% risk per trade
        let entry_price = if order_type.get() == OrderType::Market { 
            current_price.get() 
        } else { 
            price.get() 
        };
        
        if entry_price > 0.0 {
            (account * risk_percent) / entry_price
        } else {
            0.0
        }
    });
    
    let risk_amount = Signal::derive(move || account_balance.get() * 0.025);
    let r_multiple = Signal::derive(move || 3.0); // 1:3 risk-reward ratio
    
    view! {
        <Card class="h-full order-form">
            <div class="panel-header border-b p-3">
                <Flex justify=FlexJustify::SpaceBetween align=FlexAlign::Center>
                    <h2 class="text-roman-gold font-medium">"Place Order"</h2>
                    <Tag class="text-xs">
                        {move || symbol.get()}
                    </Tag>
                </Flex>
            </div>
            
            <div class="p-4">
                <Space vertical=true gap=SpaceGap::Medium>
                    // Order Type Selection
                    <OrderTypeSelector 
                        selected=order_type
                        on_change=set_order_type
                    />
                    
                    // Current Price Display
                    <Card class="bg-terminal-panel p-3">
                        <Flex justify=FlexJustify::SpaceBetween align=FlexAlign::Center>
                            <span class="text-sm text-gray-400">"Current Price:"</span>
                            <span class="font-mono font-semibold text-roman-gold">
                                {move || format!("${:.2}", current_price.get())}
                            </span>
                        </Flex>
                    </Card>
                    
                    // Amount Input
                    <AmountInput 
                        value=amount 
                        on_change=set_amount
                        symbol=symbol
                    />
                    
                    // Price Input (for limit/stop orders)
                    <Show when=move || order_type.get() != OrderType::Market>
                        <PriceInput 
                            value=price
                            on_change=set_price
                            label="Limit Price"
                        />
                    </Show>
                    
                    // Stop Price Input (for stop orders)
                    <Show when=move || matches!(order_type.get(), OrderType::Stop | OrderType::StopLimit)>
                        <PriceInput 
                            value=stop_price
                            on_change=set_stop_price
                            label="Stop Price"
                        />
                    </Show>
                    
                    // Van Tharp Risk Assessment
                    <VanTharpAssessment 
                        position_size=position_size
                        risk_amount=risk_amount
                        r_multiple=r_multiple
                        account_balance=account_balance
                    />
                    
                    // Order Summary
                    <OrderSummary 
                        order_type=order_type
                        amount=amount
                        price=price
                        current_price=current_price
                        symbol=symbol
                    />
                    
                    // Execution Buttons
                    <TradingButtonGroup
                        on_long=move || {
                            if let Some(callback) = on_submit_buy {
                                set_is_executing.set(true);
                                let order = OrderData::new(
                                    symbol.get(),
                                    order_type.get(),
                                    amount.get(),
                                    if order_type.get() == OrderType::Market { current_price.get() } else { price.get() },
                                    true, // is_buy
                                );
                                callback.run(order);
                            }
                        }
                        on_short=move || {
                            if let Some(callback) = on_submit_sell {
                                set_is_executing.set(true);
                                let order = OrderData::new(
                                    symbol.get(),
                                    order_type.get(),
                                    amount.get(),
                                    if order_type.get() == OrderType::Market { current_price.get() } else { price.get() },
                                    false, // is_buy
                                );
                                callback.run(order);
                            }
                        }
                        long_loading=Signal::derive(move || is_executing.get())
                        short_loading=Signal::derive(move || is_executing.get())
                        vertical=true
                        block=true
                    />
                </Space>
            </div>
        </Card>
    }
}

#[component]
fn OrderTypeSelector(
    #[prop(into)] selected: Signal<OrderType>,
    #[prop(into)] on_change: WriteSignal<OrderType>,
) -> impl IntoView {
    view! {
        <Space vertical=true gap=SpaceGap::Small>
            <span class="text-sm font-medium">"Order Type"</span>
            <Flex gap=SpaceGap::Small wrap=true>
                <Button 
                    size=ButtonSize::Small
                    class=Signal::derive(move || if selected.get() == OrderType::Market { "bg-roman-gold text-black" } else { "" })
                    on_click=move |_| on_change.set(OrderType::Market)
                >
                    "Market"
                </Button>
                <Button 
                    size=ButtonSize::Small
                    class=Signal::derive(move || if selected.get() == OrderType::Limit { "bg-roman-gold text-black" } else { "" })
                    on_click=move |_| on_change.set(OrderType::Limit)
                >
                    "Limit"
                </Button>
                <Button 
                    size=ButtonSize::Small
                    class=Signal::derive(move || if selected.get() == OrderType::Stop { "bg-roman-gold text-black" } else { "" })
                    on_click=move |_| on_change.set(OrderType::Stop)
                >
                    "Stop"
                </Button>
            </Flex>
        </Space>
    }
}

#[component]
fn AmountInput(
    #[prop(into)] value: Signal<f64>,
    #[prop(into)] on_change: WriteSignal<f64>,
    #[prop(into)] symbol: Signal<String>,
) -> impl IntoView {
    view! {
        <Space vertical=true gap=SpaceGap::Small>
            <span class="text-sm font-medium">"Amount"</span>
            <Card class="p-3 cursor-text hover:border-roman-gold transition-colors">
                <Flex justify=FlexJustify::SpaceBetween align=FlexAlign::Center>
                    <input 
                        type="number"
                        step="0.00000001"
                        class="bg-transparent outline-none font-mono text-sm flex-1"
                        placeholder="0.00000000"
                        value=move || if value.get() == 0.0 { String::new() } else { format!("{:.8}", value.get()) }
                        on:input=move |e| {
                            let val = event_target_value(&e);
                            if let Ok(num) = val.parse::<f64>() {
                                on_change.set(num);
                            }
                        }
                    />
                    <span class="text-sm text-gray-400 ml-2">
                        {move || symbol.get().split('/').next().unwrap_or("").to_string()}
                    </span>
                </Flex>
            </Card>
        </Space>
    }
}

#[component] 
fn PriceInput(
    #[prop(into)] value: Signal<f64>,
    #[prop(into)] on_change: WriteSignal<f64>,
    #[prop(into)] label: String,
) -> impl IntoView {
    view! {
        <Space vertical=true gap=SpaceGap::Small>
            <span class="text-sm font-medium">{label}</span>
            <Card class="p-3 cursor-text hover:border-roman-gold transition-colors">
                <Flex justify=FlexJustify::SpaceBetween align=FlexAlign::Center>
                    <input 
                        type="number"
                        step="0.01"
                        class="bg-transparent outline-none font-mono text-sm flex-1"
                        placeholder="0.00"
                        value=move || if value.get() == 0.0 { String::new() } else { format!("{:.2}", value.get()) }
                        on:input=move |e| {
                            let val = event_target_value(&e);
                            if let Ok(num) = val.parse::<f64>() {
                                on_change.set(num);
                            }
                        }
                    />
                    <span class="text-sm text-gray-400 ml-2">"USD"</span>
                </Flex>
            </Card>
        </Space>
    }
}

#[component]
fn VanTharpAssessment(
    #[prop(into)] position_size: Signal<f64>,
    #[prop(into)] risk_amount: Signal<f64>, 
    #[prop(into)] r_multiple: Signal<f64>,
    #[prop(into)] account_balance: Signal<f64>,
) -> impl IntoView {
    let risk_percentage = move || (risk_amount.get() / account_balance.get()) * 100.0;
    
    view! {
        <Card class="bg-terminal-panel border-roman-gold border">
            <div class="p-3">
                <Space vertical=true gap=SpaceGap::Small>
                    <Flex align=FlexAlign::Center gap=SpaceGap::Small>
                        <Icon icon=i::AiSafetyOutlined class="text-roman-gold" />
                        <span class="text-sm font-medium text-roman-gold">"Van Tharp Assessment"</span>
                        <VanTharpTooltip />
                    </Flex>
                    
                    <Space vertical=true gap=SpaceGap::Small class="mt-2">
                        <Flex justify=FlexJustify::SpaceBetween>
                            <span class="text-xs">"Position Size:"</span>
                            <span class="text-xs font-mono">
                                {move || format!("{:.8}", position_size.get())}
                            </span>
                        </Flex>
                        <Flex justify=FlexJustify::SpaceBetween align=FlexAlign::Center>
                            <Flex align=FlexAlign::Center gap=SpaceGap::Small>
                                <span class="text-xs">"Risk per Trade:"</span>
                                <RiskPercentTooltip />
                            </Flex>
                            <span class="text-xs font-mono">
                                {move || format!("{:.1}%", risk_percentage())}
                            </span>
                        </Flex>
                        <Flex justify=FlexJustify::SpaceBetween>
                            <span class="text-xs">"Risk Amount:"</span>
                            <span class="text-xs font-mono">
                                {move || format!("${:.2}", risk_amount.get())}
                            </span>
                        </Flex>
                        <Flex justify=FlexJustify::SpaceBetween align=FlexAlign::Center>
                            <Flex align=FlexAlign::Center gap=SpaceGap::Small>
                                <span class="text-xs">"R-Multiple:"</span>
                                <RMultipleTooltip />
                            </Flex>
                            <span class="text-xs font-mono text-green-400">
                                {move || format!("1:{:.1}", r_multiple.get())}
                            </span>
                        </Flex>
                    </Space>
                </Space>
            </div>
        </Card>
    }
}

#[component]
fn OrderSummary(
    #[prop(into)] order_type: Signal<OrderType>,
    #[prop(into)] amount: Signal<f64>,
    #[prop(into)] price: Signal<f64>,
    #[prop(into)] current_price: Signal<f64>,
    #[prop(into)] symbol: Signal<String>,
) -> impl IntoView {
    let total_value = move || {
        let effective_price = if order_type.get() == OrderType::Market {
            current_price.get()
        } else {
            price.get()
        };
        amount.get() * effective_price
    };
    
    view! {
        <Card class="bg-surface-02">
            <div class="p-3">
                <Space vertical=true gap=SpaceGap::Small>
                    <span class="text-sm font-medium">"Order Summary"</span>
                    <Flex justify=FlexJustify::SpaceBetween>
                        <span class="text-xs">"Type:"</span>
                        <span class="text-xs font-mono">{move || order_type.get().to_string()}</span>
                    </Flex>
                    <Flex justify=FlexJustify::SpaceBetween>
                        <span class="text-xs">"Amount:"</span>
                        <span class="text-xs font-mono">{move || format!("{:.8}", amount.get())}</span>
                    </Flex>
                    <Flex justify=FlexJustify::SpaceBetween>
                        <span class="text-xs">"Est. Total:"</span>
                        <span class="text-xs font-mono">{move || format!("${:.2}", total_value())}</span>
                    </Flex>
                </Space>
            </div>
        </Card>
    }
}

// Order data structure
#[derive(Clone, Debug)]
pub struct OrderData {
    pub symbol: String,
    pub order_type: OrderType,
    pub amount: f64,
    pub price: f64,
    pub is_buy: bool,
}

impl OrderData {
    pub fn new(symbol: String, order_type: OrderType, amount: f64, price: f64, is_buy: bool) -> Self {
        Self {
            symbol,
            order_type,
            amount,
            price,
            is_buy,
        }
    }
}