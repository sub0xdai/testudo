// @anchor infra:cli:config
// @tags infra

//! Config loading from ~/.config/testudo/config.toml.

use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;

/// Top-level configuration loaded from ~/.config/testudo/config.toml.
#[derive(Debug, Clone, Default, Deserialize, Serialize)]
pub struct Config {
    #[serde(default)]
    pub ui: UiConfig,
    #[serde(default)]
    pub api: ApiConfig,
    #[serde(default)]
    pub agent: AgentConfig,
    #[serde(default)]
    pub llm: LlmConfig,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct UiConfig {
    #[serde(default = "default_theme")]
    pub theme: String,
}

impl Default for UiConfig {
    fn default() -> Self {
        Self { theme: default_theme() }
    }
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct ApiConfig {
    #[serde(default = "default_base_url")]
    pub base_url: String,
    #[serde(default)]
    pub agent_key: String,
}

impl Default for ApiConfig {
    fn default() -> Self {
        Self {
            base_url: default_base_url(),
            agent_key: String::new(),
        }
    }
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct AgentConfig {
    #[serde(default = "default_loop_interval_secs")]
    pub loop_interval_secs: u64,
    #[serde(default = "default_true")]
    pub shadow_only: bool,
}

impl Default for AgentConfig {
    fn default() -> Self {
        Self {
            loop_interval_secs: default_loop_interval_secs(),
            shadow_only: true,
        }
    }
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct LlmConfig {
    #[serde(default = "default_provider")]
    pub provider: String,
    #[serde(default)]
    pub api_key: String,
    #[serde(default = "default_model")]
    pub model: String,
}

impl Default for LlmConfig {
    fn default() -> Self {
        Self {
            provider: default_provider(),
            api_key: String::new(),
            model: default_model(),
        }
    }
}

// ── Serde default functions ──

fn default_theme() -> String {
    "vanilla-amoled".into()
}

fn default_base_url() -> String {
    "http://localhost:8080/api/v1".into()
}

fn default_loop_interval_secs() -> u64 {
    60
}

fn default_true() -> bool {
    true
}

fn default_provider() -> String {
    "anthropic".into()
}

fn default_model() -> String {
    "claude-sonnet-4-20250514".into()
}

impl Config {
    /// Resolve the XDG config directory for testudo.
    fn config_dir() -> PathBuf {
        directories::ProjectDirs::from("com", "testudo", "tudo")
            .map(|proj_dirs| proj_dirs.config_dir().to_path_buf())
            .unwrap_or_else(|| {
                let fallback = dirs_fallback();
                fallback.join("testudo")
            })
    }

    /// Load config from disk. Creates default file if none exists.
    /// Exits the process on parse errors.
    pub fn load() -> Self {
        let config_dir = Self::config_dir();
        let config_path = config_dir.join("config.toml");

        if !config_path.exists() {
            return Self::create_default(&config_dir, &config_path);
        }

        let contents = fs::read_to_string(&config_path).unwrap_or_else(|e| {
            eprintln!(
                "Failed to read config at {}: {}",
                config_path.display(),
                e
            );
            std::process::exit(1);
        });

        toml::from_str(&contents).unwrap_or_else(|e| {
            eprintln!(
                "Failed to parse config at {}: {}",
                config_path.display(),
                e
            );
            std::process::exit(1);
        })
    }

    /// Create the default config file on disk and return defaults.
    fn create_default(config_dir: &PathBuf, config_path: &PathBuf) -> Self {
        let default_config = Config::default();

        // Build TOML with comments
        let default_toml = format!(
            r#"# testudo configuration — auto-generated on first run
# Edit this file to customize your trading harness.

[ui]
# Theme: "vanilla-amoled" (only option for now)
theme = "{}"

[api]
# Base URL of the Testudo backend
base_url = "{}"
# Agent key from AGENT-07 (testudo_sk_...)
agent_key = "{}"

[agent]
# Seconds between agent loop iterations
loop_interval_secs = {}
# Run in shadow mode (signals logged but not executed)
shadow_only = {}

[llm]
# LLM provider: "anthropic" or "openai"
provider = "{}"
# API key for the LLM provider
api_key = "{}"
# Model name
model = "{}"
"#,
            default_config.ui.theme,
            default_config.api.base_url,
            default_config.api.agent_key,
            default_config.agent.loop_interval_secs,
            default_config.agent.shadow_only,
            default_config.llm.provider,
            default_config.llm.api_key,
            default_config.llm.model,
        );

        fs::create_dir_all(config_dir).unwrap_or_else(|e| {
            eprintln!(
                "Failed to create config directory {}: {}",
                config_dir.display(),
                e
            );
            std::process::exit(1);
        });

        fs::write(config_path, &default_toml).unwrap_or_else(|e| {
            eprintln!(
                "Failed to write default config to {}: {}",
                config_path.display(),
                e
            );
            std::process::exit(1);
        });

        eprintln!(
            "Created default config at {}",
            config_path.display()
        );

        default_config
    }
}

/// Fallback config directory when XDG resolution fails.
fn dirs_fallback() -> PathBuf {
    let home = std::env::var("HOME").unwrap_or_else(|_| "/tmp".into());
    PathBuf::from(home).join(".config")
}
