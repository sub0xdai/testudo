//! Risk profiles for trading behavior classification
//!
//! This module defines the risk profiles that classify traders based on their
//! risk tolerance and trading behavior. These profiles align with Van Tharp
//! methodology and drive different risk limits throughout the system.

use serde::{Deserialize, Serialize};

/// Risk profile classification for traders
/// 
/// This enum defines the three primary risk profiles used throughout the Testudo
/// platform to classify trader behavior and apply appropriate risk limits.
/// The profiles align with Van Tharp's position sizing methodology.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum RiskProfile {
    /// Conservative risk profile
    /// - Individual trade risk: 1-2% of account equity
    /// - Lower position sizes, higher safety margins
    /// - Suitable for risk-averse traders and smaller accounts
    Conservative,
    
    /// Standard risk profile (recommended default)
    /// - Individual trade risk: 2-6% of account equity
    /// - Balanced approach following Van Tharp guidelines
    /// - Suitable for most systematic traders
    Standard,
    
    /// Aggressive risk profile
    /// - Individual trade risk: 6-10% of account equity
    /// - Higher position sizes for experienced traders only
    /// - Requires larger account sizes and risk management experience
    Aggressive,
}

impl RiskProfile {
    /// Get the maximum individual trade risk percentage for this profile
    /// 
    /// Returns the maximum percentage of account equity that can be risked
    /// on a single trade for this risk profile.
    pub fn max_trade_risk_percent(self) -> rust_decimal::Decimal {
        use rust_decimal_macros::dec;
        
        match self {
            RiskProfile::Conservative => dec!(0.02), // 2%
            RiskProfile::Standard => dec!(0.06),     // 6%
            RiskProfile::Aggressive => dec!(0.10),   // 10%
        }
    }
    
    /// Get the recommended trade risk percentage for this profile
    /// 
    /// Returns the recommended percentage of account equity to risk
    /// on trades for optimal performance with this risk profile.
    pub fn recommended_trade_risk_percent(self) -> rust_decimal::Decimal {
        use rust_decimal_macros::dec;
        
        match self {
            RiskProfile::Conservative => dec!(0.01), // 1%
            RiskProfile::Standard => dec!(0.02),     // 2%
            RiskProfile::Aggressive => dec!(0.06),   // 6%
        }
    }
    
    /// Get the human-readable description of this risk profile
    pub fn description(self) -> &'static str {
        match self {
            RiskProfile::Conservative => "Conservative - Lower risk, smaller positions",
            RiskProfile::Standard => "Standard - Balanced Van Tharp methodology",
            RiskProfile::Aggressive => "Aggressive - Higher risk for experienced traders",
        }
    }
}

impl Default for RiskProfile {
    fn default() -> Self {
        RiskProfile::Standard
    }
}

impl std::fmt::Display for RiskProfile {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let name = match self {
            RiskProfile::Conservative => "Conservative",
            RiskProfile::Standard => "Standard", 
            RiskProfile::Aggressive => "Aggressive",
        };
        write!(f, "{}", name)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rust_decimal_macros::dec;

    #[test]
    fn test_risk_profile_max_trade_risk_percent() {
        assert_eq!(RiskProfile::Conservative.max_trade_risk_percent(), dec!(0.02));
        assert_eq!(RiskProfile::Standard.max_trade_risk_percent(), dec!(0.06));
        assert_eq!(RiskProfile::Aggressive.max_trade_risk_percent(), dec!(0.10));
    }

    #[test]
    fn test_risk_profile_recommended_trade_risk_percent() {
        assert_eq!(RiskProfile::Conservative.recommended_trade_risk_percent(), dec!(0.01));
        assert_eq!(RiskProfile::Standard.recommended_trade_risk_percent(), dec!(0.02));
        assert_eq!(RiskProfile::Aggressive.recommended_trade_risk_percent(), dec!(0.06));
    }

    #[test]
    fn test_risk_profile_display() {
        assert_eq!(format!("{}", RiskProfile::Conservative), "Conservative");
        assert_eq!(format!("{}", RiskProfile::Standard), "Standard");
        assert_eq!(format!("{}", RiskProfile::Aggressive), "Aggressive");
    }

    #[test]
    fn test_risk_profile_default() {
        assert_eq!(RiskProfile::default(), RiskProfile::Standard);
    }

    #[test]
    fn test_risk_profile_serialization() {
        let profile = RiskProfile::Standard;
        let json = serde_json::to_string(&profile).unwrap();
        assert_eq!(json, "\"standard\"");
        
        let deserialized: RiskProfile = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized, profile);
    }
}