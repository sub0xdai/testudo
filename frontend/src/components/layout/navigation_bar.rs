use leptos::prelude::*;
use thaw::{Button, ButtonAppearance, ButtonSize, Space, SpaceGap, Flex, FlexAlign, FlexJustify, FlexGap, Card, Tag, Icon};
use icondata as i;

#[component]
pub fn NavigationBar() -> impl IntoView {
    view! {
        <nav class="thaw-navbar">
            <Card class="w-full">
                <Flex justify=FlexJustify::SpaceBetween align=FlexAlign::Center class="px-6 py-3">
                    // Left side - Logo and Market Selector
                    <Flex align=FlexAlign::Center gap=FlexGap::Large>
                        // Testudo Logo
                        <Flex align=FlexAlign::Center gap=FlexGap::Small>
                            <div class="w-8 h-8 bg-roman-gold rounded-sm flex items-center justify-center">
                                <span class="text-terminal-bg font-bold text-sm">"T"</span>
                            </div>
                            <h1 class="text-roman-gold font-bold text-lg">"Testudo"</h1>
                        </Flex>
                        
                        // Market Selector (simplified)
                        <MarketSelector />
                    </Flex>
                    
                    // Right side - Navigation and User Area
                    <Flex align=FlexAlign::Center gap=FlexGap::Large>
                        // Main Navigation Menu
                        <Space gap=SpaceGap::Medium>
                            <Button 
                                appearance=ButtonAppearance::Subtle
                                size=ButtonSize::Medium
                                class="hover:text-roman-gold transition-colors"
                            >"Markets"</Button>
                            <Button 
                                appearance=ButtonAppearance::Subtle
                                size=ButtonSize::Medium
                                class="hover:text-roman-gold transition-colors"
                            >"Portfolio"</Button>
                            <Button 
                                appearance=ButtonAppearance::Subtle
                                size=ButtonSize::Medium
                                class="hover:text-roman-gold transition-colors"
                            >"Analytics"</Button>
                            <Button 
                                appearance=ButtonAppearance::Subtle
                                size=ButtonSize::Medium
                                class="hover:text-roman-gold transition-colors"
                            >"Settings"</Button>
                            <ApiSettingsButton />
                        </Space>
                        
                        // User Area
                        <Flex align="center" gap=SpaceGap::Medium>
                            <AccountBalance />
                            <UserButton />
                        </Flex>
                    </Flex>
                </Flex>
            </Card>
        </nav>
    }
}

#[component]
pub fn MarketSelector() -> impl IntoView {
    let selected_market = RwSignal::new("BTC/USDT".to_string());

    view! {
        <Button 
            appearance=ButtonAppearance::Subtle
            size=ButtonSize::Medium
            class="hover:text-roman-gold transition-colors"
        >
            <Icon icon=i::AiLineChartOutlined />
            " " {move || selected_market.get()} " "
            <Icon icon=i::AiCaretDownOutlined />
        </Button>
    }
}

#[component]
pub fn ApiSettingsButton() -> impl IntoView {
    view! {
        <Button 
            appearance=ButtonAppearance::Subtle
            size=ButtonSize::Medium
            class="hover:text-roman-gold transition-colors"
        >
            <Icon icon=i::AiApiOutlined />
            " API "
            <Icon icon=i::AiCaretDownOutlined />
            <Tag class="status-online ml-1 text-xs">"●"</Tag>
        </Button>
    }
}

#[component]
pub fn AccountBalance() -> impl IntoView {
    let balance = RwSignal::new(10_000.0);
    
    view! {
        <Card class="hover-lift">
            <Flex align=FlexAlign::Center gap=FlexGap::Small class="px-3 py-2">
                <Icon icon=i::AiWalletOutlined class="text-roman-gold" />
                <Space vertical=true gap="xs">
                    <span class="text-xs text-gray-400">"Balance"</span>
                    <span class="font-mono font-semibold">
                        {move || format!("${:.2}", balance.get())}
                    </span>
                </Space>
            </Flex>
        </Card>
    }
}

#[component]
pub fn UserButton() -> impl IntoView {
    view! {
        <Button 
            appearance=ButtonAppearance::Transparent
            size=ButtonSize::Medium
            class="hover:bg-surface-03 rounded-full"
        >
            <Icon icon=i::AiUserOutlined />
        </Button>
    }
}