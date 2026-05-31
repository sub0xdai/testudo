// @anchor infra:cli:strategies:validator
// @tags infra

//! Strategy validator — cross-references strategies against proof artifacts.

use crate::strategies::loader::StrategyArtifact;
use crate::strategies::template::StrategyTemplate;
use std::collections::HashMap;

pub struct StrategyValidator;

#[derive(Debug, Clone)]
pub struct ValidationResult {
    pub valid: bool,
    pub errors: Vec<String>,
    pub warnings: Vec<String>,
}

impl StrategyValidator {
    /// Validate a strategy against loaded proof artifacts.
    pub fn validate(
        strategy: &StrategyTemplate,
        artifacts: &HashMap<String, StrategyArtifact>,
    ) -> ValidationResult {
        let mut result = ValidationResult {
            valid: true,
            errors: vec![],
            warnings: vec![],
        };

        // Check required proofs are present
        for required in &strategy.required_proofs {
            if !artifacts.contains_key(required) {
                result.errors.push(format!(
                    "Strategy requires proof artifact '{}' but it is not installed. \
                     Run `lake build` in testudo-proofs/ and ensure the .toml artifact exists.",
                    required
                ));
                result.valid = false;
            }
        }

        // Check strategy constraints don't violate proven bounds
        if let Some(ref strat_constraints) = strategy.constraints {
            for (name, artifact) in artifacts {
                for (key, value) in &artifact.constraints {
                    if key.as_str() == "max_leverage" {
                        if let Some(strat_lev) = strat_constraints.max_leverage {
                            if let Some(artifact_lev) = value.as_integer() {
                                if (strat_lev as i64) > artifact_lev {
                                    result.errors.push(format!(
                                        "Strategy max_leverage={} exceeds {}'s \
                                         proven bound of {}",
                                        strat_lev, name, artifact_lev
                                    ));
                                    result.valid = false;
                                }
                            } else if let Some(artifact_lev) = value.as_float() {
                                if (strat_lev as f64) > artifact_lev {
                                    result.errors.push(format!(
                                        "Strategy max_leverage={} exceeds {}'s \
                                         proven bound of {:.1}",
                                        strat_lev, name, artifact_lev
                                    ));
                                    result.valid = false;
                                }
                            }
                        }
                    }
                }
            }
        }

        result
    }
}
