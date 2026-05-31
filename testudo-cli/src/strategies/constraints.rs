// @anchor infra:cli:strategies:constraints
// @tags infra

//! Constraint merging — combines proof artifact constraints with user risk config.

/// Effective constraint set for the current strategy session.
/// Always picks the most conservative value (min for caps, max for floors).
#[derive(Debug, Clone)]
pub struct ConstraintSet {
    pub max_leverage: f64,
    pub max_account_risk_pct: f64,
    pub max_drawdown_pct: f64,
    pub min_kelly_fraction: f64,
    pub max_kelly_fraction: f64,
    pub min_samples: i64,
    pub min_required_win_rate: f64,
    pub stop_loss_required: bool,
}

impl ConstraintSet {
    /// Default values — permissive, tightened by artifacts and user config.
    pub fn defaults() -> Self {
        Self {
            max_leverage: 20.0,
            max_account_risk_pct: 10.0,
            max_drawdown_pct: 50.0,
            min_kelly_fraction: 0.001,
            max_kelly_fraction: 0.25,
            min_samples: 10,
            min_required_win_rate: 0.40,
            stop_loss_required: false,
        }
    }

    /// Apply constraints from a single proof artifact.
    /// Picks the most conservative value when artifacts overlap.
    pub fn apply_artifact(
        &mut self,
        _name: &str,
        max_leverage: f64,
        max_account_risk_pct: f64,
        max_drawdown_pct: f64,
    ) {
        // For caps (max values), the smaller number is more conservative
        self.max_leverage = self.max_leverage.min(max_leverage);
        self.max_account_risk_pct = self.max_account_risk_pct.min(max_account_risk_pct);
        self.max_drawdown_pct = self.max_drawdown_pct.min(max_drawdown_pct);
    }

    /// Apply a single constraint from a TOML value.
    pub fn apply_toml_constraint(
        &mut self,
        _name: &str,
        key: &str,
        value: &toml::Value,
    ) {
        match key {
            "max_leverage" => {
                if let Some(v) = value.as_integer() {
                    self.max_leverage = self.max_leverage.min(v as f64);
                } else if let Some(v) = value.as_float() {
                    self.max_leverage = self.max_leverage.min(v);
                }
            }
            "max_account_risk_pct" => {
                if let Some(v) = value.as_float() {
                    self.max_account_risk_pct = self.max_account_risk_pct.min(v);
                } else if let Some(v) = value.as_integer() {
                    self.max_account_risk_pct = self.max_account_risk_pct.min(v as f64);
                }
            }
            "max_drawdown_pct" => {
                if let Some(v) = value.as_float() {
                    self.max_drawdown_pct = self.max_drawdown_pct.min(v);
                } else if let Some(v) = value.as_integer() {
                    self.max_drawdown_pct = self.max_drawdown_pct.min(v as f64);
                }
            }
            "min_kelly_fraction" => {
                if let Some(v) = value.as_float() {
                    // For floors, the larger number is more conservative
                    self.min_kelly_fraction = self.min_kelly_fraction.max(v);
                }
            }
            "max_kelly_fraction" => {
                if let Some(v) = value.as_float() {
                    self.max_kelly_fraction = self.max_kelly_fraction.min(v);
                }
            }
            "min_samples" => {
                if let Some(v) = value.as_integer() {
                    self.min_samples = self.min_samples.max(v);
                }
            }
            "min_required_win_rate" => {
                if let Some(v) = value.as_float() {
                    // Higher win rate = more conservative
                    self.min_required_win_rate = self.min_required_win_rate.max(v);
                }
            }
            "stop_loss_required" => {
                if let Some(v) = value.as_bool() {
                    self.stop_loss_required = self.stop_loss_required || v;
                }
            }
            _ => {}
        }
    }

    /// Intersect with user's risk config. User can only tighten bounds, never loosen.
    pub fn intersect_user(
        &mut self,
        max_leverage: f64,
        max_account_risk_pct: f64,
        max_drawdown_pct: f64,
    ) {
        self.max_leverage = self.max_leverage.min(max_leverage);
        self.max_account_risk_pct = self.max_account_risk_pct.min(max_account_risk_pct);
        self.max_drawdown_pct = self.max_drawdown_pct.min(max_drawdown_pct);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn defaults_are_permissive() {
        let cs = ConstraintSet::defaults();
        assert_eq!(cs.max_leverage, 20.0);
        assert_eq!(cs.max_drawdown_pct, 50.0);
    }

    #[test]
    fn merge_picks_min_for_caps() {
        let mut cs = ConstraintSet::defaults();
        cs.apply_artifact("a", 5.0, 2.0, 15.0);
        cs.apply_artifact("b", 3.0, 1.0, 20.0);
        assert_eq!(cs.max_leverage, 3.0);
        assert_eq!(cs.max_account_risk_pct, 1.0);
        assert_eq!(cs.max_drawdown_pct, 15.0);
    }

    #[test]
    fn user_cannot_loosen() {
        let mut cs = ConstraintSet::defaults();
        cs.apply_artifact("a", 5.0, 2.0, 15.0);
        cs.intersect_user(10.0, 5.0, 30.0);
        assert_eq!(cs.max_leverage, 5.0);
    }

    #[test]
    fn user_can_tighten() {
        let mut cs = ConstraintSet::defaults();
        cs.apply_artifact("a", 5.0, 2.0, 15.0);
        cs.intersect_user(2.0, 1.0, 5.0);
        assert_eq!(cs.max_leverage, 2.0);
        assert_eq!(cs.max_account_risk_pct, 1.0);
    }
}
