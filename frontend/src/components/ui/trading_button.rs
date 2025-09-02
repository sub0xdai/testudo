use leptos::prelude::*;
use thaw::{Button, ButtonAppearance, ButtonSize, Icon};
use icondata as i;

#[component]
pub fn TradingButton(
    #[prop(into)] text: String,
    #[prop(default = false)] is_long: bool,
    #[prop(into)] on_click: Callback<()>,
    #[prop(into, default = ButtonAppearance::Primary)] appearance: ButtonAppearance,
    #[prop(into, default = ButtonSize::Large)] size: ButtonSize,
    #[prop(default = false)] block: bool,
    #[prop(into, default = Signal::derive(|| false))] loading: Signal<bool>,
) -> impl IntoView {
    let color_class = if is_long { 
        "long-button animate-element hover:shadow-[0_0_20px_rgba(0,255,133,0.3)] active:scale-[0.98] transition-all duration-200" 
    } else { 
        "short-button animate-element hover:shadow-[0_0_20px_rgba(255,0,102,0.3)] active:scale-[0.98] transition-all duration-200" 
    };
    
    let icon = if is_long { 
        i::AiArrowUpOutlined 
    } else { 
        i::AiArrowDownOutlined 
    };
    
    view! {
        <Button
            appearance=appearance
            size=size
            class=move || {
                let mut classes = color_class.to_string();
                if block {
                    classes.push_str(" w-full min-w-[120px]");
                }
                classes
            }
            loading=loading
            on_click=move |_| on_click(())
        >
            <Icon icon=icon />
            " " {text}
        </Button>
    }
}

#[component] 
pub fn TradingButtonGroup(
    #[prop(into)] on_long: Callback<()>,
    #[prop(into)] on_short: Callback<()>,
    #[prop(into, default = Signal::derive(|| false))] long_loading: Signal<bool>,
    #[prop(into, default = Signal::derive(|| false))] short_loading: Signal<bool>,
    #[prop(default = false)] vertical: bool,
    #[prop(default = false)] block: bool,
) -> impl IntoView {
    use thaw::Space;
    
    view! {
        <Space vertical=vertical gap=if vertical { "small" } else { "medium" }>
            <TradingButton
                text="BUY / LONG".to_string()
                is_long=true
                on_click=on_long
                loading=long_loading
                block=block
            />
            <TradingButton
                text="SELL / SHORT".to_string()
                is_long=false
                on_click=on_short
                loading=short_loading
                block=block
            />
        </Space>
    }
}