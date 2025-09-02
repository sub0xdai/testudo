use leptos::prelude::*;
use thaw::{Card, Tag, Space, SpaceGap, Flex, FlexAlign, Icon};
use icondata as i;

#[component]
pub fn PriceCard(
    #[prop(into)] symbol: String,
    #[prop(into)] price: Signal<f64>,
    #[prop(into)] change_pct: Signal<f64>,
    #[prop(into, optional)] on_click: Option<Callback<()>>,
    #[prop(default = true)] hoverable: bool,
    #[prop(default = true)] show_chart_icon: bool,
) -> impl IntoView {
    let is_positive = move || change_pct.get() >= 0.0;
    let glow_class = move || {
        if is_positive() { "profit-glow" } else { "loss-glow" }
    };
    
    let formatted_price = move || format!("${:.2}", price.get());
    let formatted_change = move || format!("{:+.2}%", change_pct.get());
    
    view! {
        <Card 
            hoverable=hoverable
            class=move || {
                let mut classes = "hover-lift cursor-pointer price-card".to_string();
                if hoverable {
                    classes.push(' ');
                    classes.push_str(&glow_class());
                }
                classes
            }
            on_click=move |_| {
                if let Some(callback) = on_click {
                    callback(());
                }
            }
        >
            <div class="p-4">
                <Flex justify="space-between" align="start">
                    <Space vertical=true gap=SpaceGap::Small>
                        <Flex align=FlexAlign::Center gap=SpaceGap::Small>
                            <Show when=move || show_chart_icon>
                                <Icon 
                                    icon=i::AiLineChartOutlined 
                                    class="text-roman-gold"
                                />
                            </Show>
                            <span class="font-mono font-semibold text-sm">
                                {symbol}
                            </span>
                        </Flex>
                        <div class="font-mono text-xl font-bold price-update">
                            {formatted_price}
                        </div>
                    </Space>
                    
                    <Tag class=Signal::derive(move || {
                        let base_class = "text-xs font-medium";
                        if is_positive() {
                            format!("{} text-green-400", base_class)
                        } else {
                            format!("{} text-red-400", base_class)
                        }
                    })>
                        {formatted_change}
                    </Tag>
                </Flex>
            </div>
        </Card>
    }
}

#[component]
pub fn PriceGrid(
    #[prop(into)] markets: Signal<Vec<MarketData>>,
    #[prop(into, optional)] on_select: Option<Callback<String>>,
) -> impl IntoView {
    use thaw::{Grid, GridItem};
    
    view! {
        <Grid cols=Signal::derive(|| 3) x_gap=Signal::derive(|| 16) y_gap=Signal::derive(|| 16) class="w-full">
            <For
                each=move || markets.get()
                key=|market| market.symbol.clone()
                children=move |market: MarketData| {
                    view! {
                        <GridItem>
                            <PriceCard
                                symbol=market.symbol.clone()
                                price=Signal::derive(move || market.price)
                                change_pct=Signal::derive(move || market.change_pct)
                                on_click={
                                    let symbol = market.symbol.clone();
                                    move || {
                                        if let Some(callback) = on_select {
                                            callback(symbol.clone());
                                        }
                                    }
                                }
                            />
                        </GridItem>
                    }
                }
            />
        </Grid>
    }
}

// Data structure for market information
#[derive(Clone, Debug)]
pub struct MarketData {
    pub symbol: String,
    pub price: f64,
    pub change_pct: f64,
    pub volume_24h: f64,
}

impl MarketData {
    pub fn new(symbol: String, price: f64, change_pct: f64, volume_24h: f64) -> Self {
        Self {
            symbol,
            price,
            change_pct,
            volume_24h,
        }
    }
}