// @anchor infra:cli:strategies:registry
// @tags infra

//! Strategy registry — loads built-in and user-installed strategies.

use crate::strategies::template::{StrategyMeta, StrategyTemplate};
use std::collections::HashMap;
use std::path::{Path, PathBuf};

/// Error type for registry operations.
#[derive(Debug, thiserror::Error)]
pub enum RegistryError {
    #[error("Invalid TOML: {0}")]
    InvalidToml(String),

    #[error("Strategy name mismatch: expected '{expected}', got '{actual}'")]
    NameMismatch { expected: String, actual: String },

    #[error("Cannot remove built-in strategy: {0}")]
    CannotRemoveBuiltin(String),

    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
}

/// Registry of available trading strategies.
pub struct StrategyRegistry {
    builtins: HashMap<String, StrategyTemplate>,
    user_dir: PathBuf,
}

impl StrategyRegistry {
    /// Create a new registry. `config_dir` is typically `~/.config/testudo/`.
    pub fn new(config_dir: &Path) -> Self {
        let builtins = Self::load_builtins();
        Self {
            builtins,
            user_dir: config_dir.join("strategies"),
        }
    }

    fn load_builtins() -> HashMap<String, StrategyTemplate> {
        let mut map = HashMap::new();

        let entries: Vec<(&str, &str)> = vec![
            (
                "mean-reversion",
                include_str!("../../strategies/builtins/mean_reversion.toml"),
            ),
            (
                "momentum-breakout",
                include_str!("../../strategies/builtins/momentum_breakout.toml"),
            ),
            (
                "funding-arb",
                include_str!("../../strategies/builtins/funding_arb.toml"),
            ),
        ];

        for (name, content) in entries {
            match toml::from_str::<StrategyTemplate>(content) {
                Ok(tmpl) => {
                    map.insert(name.to_string(), tmpl);
                }
                Err(e) => {
                    tracing::warn!(
                        "Built-in strategy '{}' failed to parse: {}",
                        name,
                        e
                    );
                }
            }
        }

        map
    }

    /// Get a strategy by name. User strategies override builtins.
    pub fn get(&self, name: &str) -> Option<StrategyTemplate> {
        let user_path = self.user_dir.join(format!("{}.toml", name));
        if user_path.exists() {
            if let Ok(content) = std::fs::read_to_string(&user_path) {
                if let Ok(tmpl) = toml::from_str(&content) {
                    return Some(tmpl);
                }
            }
        }
        self.builtins.get(name).cloned()
    }

    /// List all available strategies (metadata only).
    pub fn list(&self) -> Vec<StrategyMeta> {
        let mut metas: Vec<StrategyMeta> = self
            .builtins
            .values()
            .map(|s| s.meta.clone())
            .collect();

        if let Ok(entries) = std::fs::read_dir(&self.user_dir) {
            for entry in entries.flatten() {
                let path = entry.path();
                if path.extension().is_some_and(|e| e == "toml") {
                    if let Ok(content) = std::fs::read_to_string(&path) {
                        if let Ok(tmpl) = toml::from_str::<StrategyTemplate>(&content) {
                            metas.push(tmpl.meta);
                        }
                    }
                }
            }
        }

        metas
    }

    /// Add a user strategy. Validates TOML structure and saves to user dir.
    pub fn add(&self, name: &str, content: &str) -> Result<(), RegistryError> {
        // Reject collision with builtins
        if self.builtins.contains_key(name) {
            return Err(RegistryError::CannotRemoveBuiltin(format!(
                "'{}' is a built-in strategy and cannot be overwritten",
                name
            )));
        }

        // Reject collision with existing user strategy
        let user_path = self.user_dir.join(format!("{}.toml", name));
        if user_path.exists() {
            return Err(RegistryError::InvalidToml(format!(
                "Strategy '{}' already exists. Remove it first with 'strategy remove'.",
                name
            )));
        }

        let tmpl: StrategyTemplate = toml::from_str(content)
            .map_err(|e| RegistryError::InvalidToml(e.to_string()))?;

        if tmpl.meta.name != name {
            return Err(RegistryError::NameMismatch {
                expected: name.to_string(),
                actual: tmpl.meta.name,
            });
        }

        std::fs::create_dir_all(&self.user_dir)?;
        let path = self.user_dir.join(format!("{}.toml", name));
        std::fs::write(&path, content)?;

        Ok(())
    }

    /// Remove a user strategy. Cannot remove builtins.
    pub fn remove(&self, name: &str) -> Result<(), RegistryError> {
        if self.builtins.contains_key(name) {
            return Err(RegistryError::CannotRemoveBuiltin(name.to_string()));
        }

        let path = self.user_dir.join(format!("{}.toml", name));
        if path.exists() {
            std::fs::remove_file(&path)?;
        }

        Ok(())
    }
}
