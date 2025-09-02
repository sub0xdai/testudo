use leptos::prelude::*;
use thaw::Icon;
use icondata as i;

// Simplified tooltip components for trading (no complex hover functionality for now)
#[component]
pub fn VanTharpTooltip() -> impl IntoView {
    view! {
        <Icon 
            icon=i::AiQuestionCircleOutlined 
            class="text-gray-400 hover:text-roman-gold transition-colors cursor-help text-sm"
            attr:title="Van Tharp Position Sizing: Systematic approach to position sizing based on account equity and risk tolerance."
        />
    }
}

#[component]
pub fn RMultipleTooltip() -> impl IntoView {
    view! {
        <Icon 
            icon=i::AiQuestionCircleOutlined 
            class="text-gray-400 hover:text-roman-gold transition-colors cursor-help text-sm"
            attr:title="R-Multiple: Risk-reward ratio. 1:3 means risk $1 to potentially make $3."
        />
    }
}

#[component]
pub fn RiskPercentTooltip() -> impl IntoView {
    view! {
        <Icon 
            icon=i::AiQuestionCircleOutlined 
            class="text-gray-400 hover:text-roman-gold transition-colors cursor-help text-sm"
            attr:title="Risk Percentage: Percentage of account equity at risk per trade. Max 6% per Testudo Protocol."
        />
    }
}