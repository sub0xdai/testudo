// @anchor infra:cli:strategies:loader
// @tags infra

//! Proof artifact loader — parses STRAT-01 TOML artifacts.

use serde::Deserialize;
use std::collections::HashMap;
use std::path::PathBuf;

/// A loaded strategy artifact from a TOML file in testudo-proofs/Proofs/.
#[derive(Debug, Clone, Deserialize)]
pub struct StrategyArtifact {
    pub meta: ArtifactMeta,
    pub theorem: TheoremRef,
    #[serde(default)]
    pub constraints: HashMap<String, toml::Value>,
    #[serde(default)]
    pub prompt: PromptSection,
}

#[derive(Debug, Clone, Deserialize)]
pub struct ArtifactMeta {
    pub name: String,
    pub version: String,
    pub description: String,
    pub lean_file: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct TheoremRef {
    pub name: String,
    pub statement: String,
    pub formula: String,
    #[serde(default)]
    pub implications: Vec<String>,
    pub lean_line: u32,
}

#[derive(Debug, Clone, Default, Deserialize)]
pub struct PromptSection {
    pub system_prompt: String,
}

/// Loads strategy artifacts from the proofs directory.
pub struct StrategyLoader {
    proofs_dir: PathBuf,
}

impl StrategyLoader {
    pub fn new(proofs_dir: PathBuf) -> Self {
        Self { proofs_dir }
    }

    /// Load all `.toml` artifacts from the proofs directory.
    /// Returns a map of artifact name → parsed StrategyArtifact.
    /// Gracefully returns empty map if directory doesn't exist.
    pub fn load_all(&self) -> Result<HashMap<String, StrategyArtifact>, String> {
        if !self.proofs_dir.exists() {
            tracing::warn!(
                "Proofs directory not found: {}. Running without proof-backed constraints.",
                self.proofs_dir.display()
            );
            return Ok(HashMap::new());
        }

        let mut artifacts = HashMap::new();

        let entries = std::fs::read_dir(&self.proofs_dir).map_err(|e| {
            format!(
                "Failed to read proofs directory {}: {}",
                self.proofs_dir.display(),
                e
            )
        })?;

        for entry in entries {
            let entry = entry.map_err(|e| format!("Failed to read entry: {}", e))?;
            let path = entry.path();

            if path.extension().is_some_and(|e| e == "toml") {
                let content = std::fs::read_to_string(&path).map_err(|e| {
                    format!("Failed to read {}: {}", path.display(), e)
                })?;

                let artifact: StrategyArtifact = toml::from_str(&content).map_err(|e| {
                    format!("Failed to parse {}: {}", path.display(), e)
                })?;

                let name = artifact.meta.name.clone();
                artifacts.insert(name, artifact);
            }
        }

        if artifacts.is_empty() {
            tracing::warn!(
                "No .toml artifacts found in {}. Running without proof-backed constraints.",
                self.proofs_dir.display()
            );
        } else {
            tracing::info!(
                "Loaded {} proof artifacts from {}",
                artifacts.len(),
                self.proofs_dir.display()
            );
        }

        Ok(artifacts)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn proofs_path() -> PathBuf {
        PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("..")
            .join("testudo-proofs")
            .join("Proofs")
    }

    #[test]
    fn load_kelly_artifact() {
        let loader = StrategyLoader::new(proofs_path());
        let artifacts = loader.load_all().unwrap();
        assert!(artifacts.contains_key("kelly"));
        assert_eq!(artifacts["kelly"].meta.name, "kelly");
    }
}
