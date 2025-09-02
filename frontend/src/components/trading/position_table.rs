use leptos::prelude::*;
use thaw::{Card, Tag, Space, SpaceGap, Flex, Button, ButtonSize, Icon};
use icondata as i;

#[derive(Clone, Debug)]
pub struct Position {
    pub id: String,
    pub symbol: String,
    pub side: PositionSide,
    pub size: f64,
    pub entry_price: f64,
    pub current_price: f64,
    pub unrealized_pnl: f64,
    pub unrealized_pnl_pct: f64,
    pub margin: f64,
    pub created_at: String,
}

#[derive(Clone, Debug, PartialEq)]
pub enum PositionSide {
    Long,
    Short,
}

impl std::fmt::Display for PositionSide {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            PositionSide::Long => write!(f, "LONG"),
            PositionSide::Short => write!(f, "SHORT"),
        }
    }
}

impl Position {
    pub fn new(
        id: String,
        symbol: String,
        side: PositionSide,
        size: f64,
        entry_price: f64,
        current_price: f64,
        margin: f64,
        created_at: String,
    ) -> Self {
        let unrealized_pnl = match side {
            PositionSide::Long => (current_price - entry_price) * size,
            PositionSide::Short => (entry_price - current_price) * size,
        };
        
        let unrealized_pnl_pct = (unrealized_pnl / (entry_price * size)) * 100.0;
        
        Self {
            id,
            symbol,
            side,
            size,
            entry_price,
            current_price,
            unrealized_pnl,
            unrealized_pnl_pct,
            margin,
            created_at,
        }
    }
    
    pub fn is_profitable(&self) -> bool {
        self.unrealized_pnl > 0.0
    }
}

#[component]
pub fn PositionTable(
    #[prop(into)] positions: Signal<Vec<Position>>,
    #[prop(into, optional)] on_close_position: Option<Callback<String>>,
    #[prop(default = true)] show_actions: bool,
) -> impl IntoView {
    view! {
        <Card class="w-full">
            <div class="panel-header border-b p-3">
                <Flex justify=FlexJustify::SpaceBetween align=FlexAlign::Center>
                    <h2 class="text-roman-gold font-medium">"Open Positions"</h2>
                    <Tag class="text-xs">
                        {move || format!("{} positions", positions.get().len())}
                    </Tag>
                </Flex>
            </div>
            
            <div class="p-4">
                <Show 
                    when=move || !positions.get().is_empty()
                    fallback=|| view! {
                        <div class="text-center py-8 text-gray-400">
                            <Icon icon=i::AiFileTextOutlined class="text-4xl mb-4" />
                            <p>"No open positions"</p>
                        </div>
                    }
                >
                    <div class="overflow-x-auto">
                        <table class="w-full text-sm">
                            <thead class="border-b border-gray-600">
                                <tr class="text-left">
                                    <th class="pb-2 font-medium text-gray-300">"Symbol"</th>
                                    <th class="pb-2 font-medium text-gray-300">"Side"</th>
                                    <th class="pb-2 font-medium text-gray-300">"Size"</th>
                                    <th class="pb-2 font-medium text-gray-300">"Entry Price"</th>
                                    <th class="pb-2 font-medium text-gray-300">"Current Price"</th>
                                    <th class="pb-2 font-medium text-gray-300">"PnL"</th>
                                    <th class="pb-2 font-medium text-gray-300">"PnL %"</th>
                                    <th class="pb-2 font-medium text-gray-300">"Margin"</th>
                                    <Show when=move || show_actions>
                                        <th class="pb-2 font-medium text-gray-300">"Actions"</th>
                                    </Show>
                                </tr>
                            </thead>
                            <tbody>
                                <For
                                    each=move || positions.get()
                                    key=|pos| pos.id.clone()
                                    children=move |position: Position| {
                                        view! {
                                            <PositionRow 
                                                position=position.clone()
                                                on_close=on_close_position
                                                show_actions=show_actions
                                            />
                                        }
                                    }
                                />
                            </tbody>
                        </table>
                    </div>
                </Show>
            </div>
        </Card>
    }
}

#[component]
fn PositionRow(
    #[prop(into)] position: Position,
    #[prop(into, optional)] on_close: Option<Callback<String>>,
    #[prop(default = true)] show_actions: bool,
) -> impl IntoView {
    let is_profitable = position.is_profitable();
    let pnl_class = if is_profitable { 
        "profit-glow text-green-400" 
    } else { 
        "loss-glow text-red-400" 
    };
    
    let side_class = match position.side {
        PositionSide::Long => "text-green-400",
        PositionSide::Short => "text-red-400",
    };
    
    view! {
        <tr class="border-b border-gray-700 hover:bg-surface-02 transition-colors">
            <td class="py-3">
                <span class="font-mono font-semibold">{position.symbol}</span>
            </td>
            <td class="py-3">
                <Tag class=format!("text-xs {}", side_class)>
                    {position.side.to_string()}
                </Tag>
            </td>
            <td class="py-3">
                <span class="font-mono">{format!("{:.8}", position.size)}</span>
            </td>
            <td class="py-3">
                <span class="font-mono">{format!("${:.2}", position.entry_price)}</span>
            </td>
            <td class="py-3">
                <span class="font-mono price-update">{format!("${:.2}", position.current_price)}</span>
            </td>
            <td class="py-3">
                <span class=format!("font-mono {}", pnl_class)>
                    {format!("${:.2}", position.unrealized_pnl)}
                </span>
            </td>
            <td class="py-3">
                <span class=format!("font-mono {}", pnl_class)>
                    {format!("{:+.2}%", position.unrealized_pnl_pct)}
                </span>
            </td>
            <td class="py-3">
                <span class="font-mono">{format!("${:.2}", position.margin)}</span>
            </td>
            <Show when=move || show_actions>
                <td class="py-3">
                    <Button
                        size=ButtonSize::Small
                        class="text-xs hover:text-red-400"
                        on_click={
                            let position_id = position.id.clone();
                            move |_| {
                                if let Some(callback) = on_close {
                                    callback(position_id.clone());
                                }
                            }
                        }
                    >
                        <Icon icon=i::AiCloseOutlined />
                        " Close"
                    </Button>
                </td>
            </Show>
        </tr>
    }
}

#[component]
pub fn PositionSummary(
    #[prop(into)] positions: Signal<Vec<Position>>,
) -> impl IntoView {
    let total_pnl = move || {
        positions.get().iter()
            .map(|p| p.unrealized_pnl)
            .sum::<f64>()
    };
    
    let total_margin = move || {
        positions.get().iter()
            .map(|p| p.margin)
            .sum::<f64>()
    };
    
    let winning_positions = move || {
        positions.get().iter()
            .filter(|p| p.is_profitable())
            .count()
    };
    
    let total_positions = move || positions.get().len();
    
    let is_profitable = move || total_pnl() > 0.0;
    
    view! {
        <Card class="bg-terminal-panel border-roman-gold border">
            <div class="p-4">
                <Space vertical=true gap=SpaceGap::Small>
                    <Flex align=FlexAlign::Center gap=SpaceGap::Small>
                        <Icon icon=i::AiPieChartOutlined class="text-roman-gold" />
                        <span class="text-sm font-medium text-roman-gold">"Portfolio Summary"</span>
                    </Flex>
                    
                    <div class="grid grid-cols-2 gap-4 mt-3">
                        <div>
                            <Flex justify=FlexJustify::SpaceBetween>
                                <span class="text-xs text-gray-400">"Total PnL:"</span>
                                <span class=move || {
                                    let base = "text-xs font-mono font-semibold";
                                    if is_profitable() { 
                                        format!("{} text-green-400", base)
                                    } else { 
                                        format!("{} text-red-400", base)
                                    }
                                }>
                                    {move || format!("${:.2}", total_pnl())}
                                </span>
                            </Flex>
                        </div>
                        <div>
                            <Flex justify=FlexJustify::SpaceBetween>
                                <span class="text-xs text-gray-400">"Total Margin:"</span>
                                <span class="text-xs font-mono">
                                    {move || format!("${:.2}", total_margin())}
                                </span>
                            </Flex>
                        </div>
                        <div>
                            <Flex justify=FlexJustify::SpaceBetween>
                                <span class="text-xs text-gray-400">"Winning:"</span>
                                <span class="text-xs font-mono text-green-400">
                                    {move || format!("{}/{}", winning_positions(), total_positions())}
                                </span>
                            </Flex>
                        </div>
                        <div>
                            <Flex justify=FlexJustify::SpaceBetween>
                                <span class="text-xs text-gray-400">"Win Rate:"</span>
                                <span class="text-xs font-mono">
                                    {move || {
                                        let total = total_positions();
                                        if total > 0 {
                                            format!("{:.1}%", (winning_positions() as f64 / total as f64) * 100.0)
                                        } else {
                                            "0.0%".to_string()
                                        }
                                    }}
                                </span>
                            </Flex>
                        </div>
                    </div>
                </Space>
            </div>
        </Card>
    }
}