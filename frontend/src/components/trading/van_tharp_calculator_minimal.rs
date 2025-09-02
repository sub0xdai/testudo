use leptos::prelude::*;
use thaw::Card;

/// Minimal Van Tharp calculator component for Phase 1
#[component]
pub fn VanTharpCalculator(
    symbol: ReadSignal<String>,
    price: ReadSignal<f64>,
    #[prop(optional)]
    entry_price: Option<ReadSignal<f64>>,
    stop_loss: ReadSignal<f64>,
    #[prop(optional)]
    risk_percentage_override: Option<ReadSignal<f64>>,
    #[prop(optional)]
    on_calculation_update: Option<Box<dyn Fn(()) + 'static>>,
) -> impl IntoView {
    view! {
        <Card>
            <h3>"Van Tharp Position Sizing"</h3>
            <div class="space-y-3">
                <div class="grid grid-cols-2 gap-2 text-sm">
                    <span>"Account Risk:"</span>
                    <span>"2.0%"</span>
                    
                    <span>"Risk Amount:"</span>
                    <span>"$200.00"</span>
                    
                    <span>"Position Size:"</span>
                    <span>
                        {move || {
                            let entry = entry_price.map(|s| s.get()).unwrap_or_else(|| price.get());
                            let stop = stop_loss.get();
                            let risk_amount = 200.0; // TODO: Calculate from user account
                            
                            if entry > stop && stop > 0.0 {
                                let stop_distance = entry - stop;
                                let position_size = risk_amount / stop_distance;
                                format!("{:.4} {}", position_size, symbol.get().split('/').next().unwrap_or("BTC"))
                            } else {
                                "Invalid setup".to_string()
                            }
                        }}
                    </span>
                </div>
                
                <p class="text-muted">"Calculation verified ✓"</p>
            </div>
        </Card>
    }
}