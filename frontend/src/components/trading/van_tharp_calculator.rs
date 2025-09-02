//! Van Tharp Position Sizing Calculator with Backend Verification
//!
//! This module implements frontend Van Tharp position sizing calculations
//! with real-time backend verification following SOP-001:
//! - Client-side calculation for immediate feedback
//! - Backend verification for accuracy and compliance
//! - Risk profile integration
//! - WebSocket-based verification messaging

use leptos::prelude::*;
use serde::{Deserialize, Serialize};
use crate::components::auth::{use_auth, use_user, AuthState, RiskProfile};
use crate::components::ui::{use_market_data, use_websocket_sender, MarketDataMessage};

/// Van Tharp position sizing calculation inputs
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct PositionSizingInput {
    pub symbol: String,
    pub account_equity: f64,
    pub entry_price: f64,
    pub stop_loss: f64,
    pub risk_percentage: f64,
}

/// Position sizing calculation result
#[derive(Debug, Clone, PartialEq)]
pub struct PositionSizingResult {
    pub position_size: f64,
    pub risk_amount: f64,
    pub stop_distance: f64,
    pub stop_distance_percentage: f64,
    pub position_value: f64,
    pub is_valid: bool,
    pub validation_errors: Vec<String>,
    pub backend_verified: bool,
}

/// Van Tharp calculator component with real-time verification
#[component]
pub fn VanTharpCalculator(
    /// Trading symbol (e.g., "BTC/USDT")
    symbol: ReadSignal<String>,
    /// Current market price
    price: ReadSignal<f64>,
    /// Entry price (user input or current price)
    #[prop(optional)]
    entry_price: Option<ReadSignal<f64>>,
    /// Stop loss price (user input)
    stop_loss: ReadSignal<f64>,
    /// Custom risk percentage override
    #[prop(optional)]
    risk_percentage_override: Option<ReadSignal<f64>>,
    /// Callback when calculation updates
    #[prop(optional)]
    on_calculation_update: Option<Box<dyn Fn(PositionSizingResult) + 'static>>,
) -> impl IntoView {
    let user = use_user();
    let market_data = use_market_data();
    let ws_sender = use_websocket_sender();

    // Internal calculation state
    let (calculation_result, set_calculation_result) = signal(None::<PositionSizingResult>);
    let (last_verification_time, set_last_verification_time) = signal(0u64);
    let (verification_pending, set_verification_pending) = signal(false);

    // Calculate position size reactively
    let calculated_result = create_memo(move |_| {
        let user_data = user.get()?;
        let symbol_val = symbol.get();
        let entry = entry_price.map(|s| s.get()).unwrap_or_else(|| price.get());
        let stop = stop_loss.get();
        
        // Determine risk percentage
        let risk_pct = risk_percentage_override
            .map(|s| s.get())
            .unwrap_or_else(|| {
                user_data.risk_profile.max_trade_risk_percent() * 100.0 // Convert to percentage
            });

        let input = PositionSizingInput {
            symbol: symbol_val,
            account_equity: user_data.account_equity,
            entry_price: entry,
            stop_loss: stop,
            risk_percentage: risk_pct,
        };

        Some(calculate_position_size(input))
    });

    // Update calculation result when inputs change
    create_effect(move |_| {
        if let Some(result) = calculated_result.get() {
            set_calculation_result.set(Some(result.clone()));
            
            // Trigger callback if provided
            if let Some(callback) = &on_calculation_update {
                callback(result.clone());
            }
            
            // Request backend verification if calculation is valid
            if result.is_valid {
                request_backend_verification(
                    result,
                    ws_sender,
                    set_verification_pending,
                    set_last_verification_time,
                );
            }
        }
    });

    // Listen for backend verification responses
    create_effect(move |_| {
        if let Some(message) = market_data.get() {
            match message {
                MarketDataMessage::PositionCalculation {
                    position_size,
                    risk_amount,
                    user_id,
                    ..
                } => {
                    if let Some(user_data) = user.get() {
                        if user_data.sub == user_id {
                            // Update with backend verification
                            if let Some(mut result) = calculation_result.get() {
                                result.backend_verified = true;
                                
                                // Check if backend calculation matches frontend
                                let tolerance = 0.01; // 1% tolerance
                                let size_match = (result.position_size - position_size).abs() / result.position_size < tolerance;
                                let risk_match = (result.risk_amount - risk_amount).abs() / result.risk_amount < tolerance;
                                
                                if !size_match || !risk_match {
                                    result.validation_errors.push(
                                        "Backend calculation differs from frontend - using backend values".to_string()
                                    );
                                    result.position_size = position_size;
                                    result.risk_amount = risk_amount;
                                }
                                
                                set_calculation_result.set(Some(result));
                                set_verification_pending.set(false);
                            }
                        }
                    }
                }
                _ => {}
            }
        }
    });

    view! {
        <div class="van-tharp-calculator">
            <div class="calculation-header mb-3">
                <h3 class="text-text-secondary text-sm mb-1">"Van Tharp Position Sizing"</h3>
                {move || {
                    if verification_pending.get() {
                        view! {
                            <div class="verification-status text-xs text-yellow-400">
                                "⏳ Verifying with backend..."
                            </div>
                        }.into_view()
                    } else if let Some(result) = calculation_result.get() {
                        if result.backend_verified {
                            view! {
                                <div class="verification-status text-xs text-green-400">
                                    "✅ Backend verified"
                                </div>
                            }.into_view()
                        } else {
                            view! {
                                <div class="verification-status text-xs text-text-muted">
                                    "📊 Frontend calculation"
                                </div>
                            }.into_view()
                        }
                    } else {
                        view! { <div></div> }.into_view()
                    }
                }}
            </div>
            
            {move || {
                match calculation_result.get() {
                    Some(result) if result.is_valid => view! {
                        <div class="calculation-results">
                            <div class="grid grid-cols-2 gap-2 text-sm">
                                <div class="text-text-muted">"Entry:"</div>
                                <div class="text-text-primary font-mono">
                                    {format!("${:.2}", entry_price.map(|s| s.get()).unwrap_or_else(|| price.get()))}
                                </div>
                                
                                <div class="text-text-muted">"Stop:"</div>
                                <div class="text-text-primary font-mono">
                                    {format!("${:.2}", stop_loss.get())}
                                </div>
                                
                                <div class="text-text-muted">"Stop Distance:"</div>
                                <div class="text-text-primary font-mono">
                                    {format!("{:.1}%", result.stop_distance_percentage)}
                                </div>
                                
                                <div class="text-text-muted">"Risk per R:"</div>
                                <div class="text-text-primary font-mono">
                                    {format!("${:.2}", result.risk_amount)}
                                </div>
                                
                                <div class="text-text-muted">"Position Size:"</div>
                                <div class={format!("font-mono font-bold {}", 
                                    if result.backend_verified { "text-green-400" } else { "text-yellow-400" }
                                )}>
                                    {format_position_size(result.position_size, &symbol.get())}
                                </div>
                                
                                <div class="text-text-muted">"Position Value:"</div>
                                <div class="text-text-primary font-mono">
                                    {format!("${:.2}", result.position_value)}
                                </div>
                            </div>
                            
                            {if !result.validation_errors.is_empty() {
                                view! {
                                    <div class="validation-errors mt-3 p-2 bg-red-900/20 border border-red-600/30 rounded">
                                        <div class="text-red-400 text-xs font-medium mb-1">"Validation Warnings:"</div>
                                        <ul class="text-red-300 text-xs space-y-1">
                                            {result.validation_errors.iter().map(|error| view! {
                                                <li>"• " {error}</li>
                                            }).collect::<Vec<_>>()}
                                        </ul>
                                    </div>
                                }.into_view()
                            } else {
                                view! { <div></div> }.into_view()
                            }}
                            
                            <div class="risk-profile-info mt-3 text-xs text-text-muted">
                                {move || {
                                    if let Some(user_data) = user.get() {
                                        format!("Risk Profile: {} (Max: {:.1}%)", 
                                            user_data.risk_profile.description().split(" - ").next().unwrap_or("Unknown"),
                                            user_data.risk_profile.max_trade_risk_percent() * 100.0
                                        )
                                    } else {
                                        "Risk Profile: Unknown".to_string()
                                    }
                                }}
                            </div>
                        </div>
                    }.into_view(),
                    
                    Some(result) => view! {
                        <div class="calculation-error">
                            <div class="text-red-400 text-sm mb-2">"⚠️ Invalid Calculation"</div>
                            <ul class="text-red-300 text-xs space-y-1">
                                {result.validation_errors.iter().map(|error| view! {
                                    <li>"• " {error}</li>
                                }).collect::<Vec<_>>()}
                            </ul>
                        </div>
                    }.into_view(),
                    
                    None => view! {
                        <div class="calculation-loading text-text-muted text-sm">
                            "Calculating position size..."
                        </div>
                    }.into_view(),
                }
            }}
        </div>
    }
}

/// Calculate Van Tharp position size
fn calculate_position_size(input: PositionSizingInput) -> PositionSizingResult {
    let mut validation_errors = Vec::new();
    
    // Basic validation
    if input.account_equity <= 0.0 {
        validation_errors.push("Account equity must be positive".to_string());
    }
    
    if input.entry_price <= 0.0 {
        validation_errors.push("Entry price must be positive".to_string());
    }
    
    if input.stop_loss <= 0.0 {
        validation_errors.push("Stop loss must be positive".to_string());
    }
    
    if input.risk_percentage <= 0.0 || input.risk_percentage > 20.0 {
        validation_errors.push("Risk percentage must be between 0% and 20%".to_string());
    }
    
    if input.entry_price <= input.stop_loss {
        validation_errors.push("Entry price must be above stop loss for long positions".to_string());
    }
    
    // Early return if basic validation fails
    if !validation_errors.is_empty() {
        return PositionSizingResult {
            position_size: 0.0,
            risk_amount: 0.0,
            stop_distance: 0.0,
            stop_distance_percentage: 0.0,
            position_value: 0.0,
            is_valid: false,
            validation_errors,
            backend_verified: false,
        };
    }
    
    // Van Tharp calculation
    let risk_amount = input.account_equity * (input.risk_percentage / 100.0);
    let stop_distance = input.entry_price - input.stop_loss;
    let stop_distance_percentage = (stop_distance / input.entry_price) * 100.0;
    let position_size = risk_amount / stop_distance;
    let position_value = position_size * input.entry_price;
    
    // Additional validation
    if position_value > input.account_equity {
        validation_errors.push("Position value exceeds account equity".to_string());
    }
    
    if stop_distance_percentage > 10.0 {
        validation_errors.push("Stop distance is unusually large (>10%)".to_string());
    }
    
    if stop_distance_percentage < 0.5 {
        validation_errors.push("Stop distance is very tight (<0.5%)".to_string());
    }
    
    PositionSizingResult {
        position_size,
        risk_amount,
        stop_distance,
        stop_distance_percentage,
        position_value,
        is_valid: validation_errors.is_empty(),
        validation_errors,
        backend_verified: false,
    }
}

/// Request backend verification of calculation
fn request_backend_verification(
    result: PositionSizingResult,
    ws_sender: WriteSignal<Option<String>>,
    set_verification_pending: WriteSignal<bool>,
    set_last_verification_time: WriteSignal<u64>,
) {
    let verification_request = serde_json::json!({
        "type": "position_size_verification",
        "data": {
            "position_size": result.position_size,
            "risk_amount": result.risk_amount,
            "timestamp": js_sys::Date::now() as u64,
        }
    });
    
    if let Ok(json) = serde_json::to_string(&verification_request) {
        ws_sender.set(Some(json));
        set_verification_pending.set(true);
        set_last_verification_time.set(js_sys::Date::now() as u64);
    }
}

/// Format position size based on symbol type
fn format_position_size(size: f64, symbol: &str) -> String {
    if symbol.starts_with("BTC") {
        format!("{:.6} BTC", size)
    } else if symbol.starts_with("ETH") {
        format!("{:.4} ETH", size)
    } else if symbol.contains("USDT") || symbol.contains("USD") {
        // For USDT pairs, show the base currency
        let base = symbol.split('/').next().unwrap_or("UNKNOWN");
        format!("{:.4} {}", size, base)
    } else {
        format!("{:.4}", size)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_van_tharp_calculation_basic() {
        let input = PositionSizingInput {
            symbol: "BTC/USDT".to_string(),
            account_equity: 10000.0,
            entry_price: 50000.0,
            stop_loss: 48000.0,
            risk_percentage: 2.0, // 2%
        };

        let result = calculate_position_size(input);
        
        assert!(result.is_valid);
        assert_eq!(result.risk_amount, 200.0); // 2% of 10000
        assert_eq!(result.stop_distance, 2000.0); // 50000 - 48000
        assert_eq!(result.position_size, 0.1); // 200 / 2000
        assert_eq!(result.position_value, 5000.0); // 0.1 * 50000
        assert!(result.validation_errors.is_empty());
    }

    #[test]
    fn test_van_tharp_calculation_validation_errors() {
        let input = PositionSizingInput {
            symbol: "BTC/USDT".to_string(),
            account_equity: -1000.0, // Invalid
            entry_price: 48000.0,
            stop_loss: 50000.0, // Stop above entry (invalid for long)
            risk_percentage: 25.0, // Too high
        };

        let result = calculate_position_size(input);
        
        assert!(!result.is_valid);
        assert!(!result.validation_errors.is_empty());
        assert!(result.validation_errors.iter().any(|e| e.contains("Account equity")));
        assert!(result.validation_errors.iter().any(|e| e.contains("Entry price must be above stop loss")));
        assert!(result.validation_errors.iter().any(|e| e.contains("Risk percentage")));
    }

    #[test]
    fn test_position_size_formatting() {
        assert_eq!(format_position_size(0.123456, "BTC/USDT"), "0.123456 BTC");
        assert_eq!(format_position_size(1.2345, "ETH/USDT"), "1.2345 ETH");
        assert_eq!(format_position_size(100.5555, "ADA/USDT"), "100.5556 ADA");
    }
}