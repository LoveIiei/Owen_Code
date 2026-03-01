use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Config {
    pub default_backend: Backend,
    pub ollama: OllamaConfig,
    pub nim: NimConfig,
    pub ui: UiConfig,
    pub planner: PlannerConfig,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum Backend {
    Ollama,
    Nim,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OllamaConfig {
    pub base_url: String,
    pub default_model: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NimConfig {
    pub base_url: String,
    pub api_key: String,
    pub default_model: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UiConfig {
    pub show_file_tree: bool,
    pub syntax_highlight: bool,
    pub mouse_enabled: bool,
}

/// Which backend + model to use for the lightweight planning step.
/// Defaults to the same Ollama instance with a small fast model.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PlannerConfig {
    /// Backend to use for planning: "ollama" or "nim"
    pub backend: Backend,
    /// Model name — pick something small and fast, e.g. "qwen2.5:1.5b" or "llama3.2:1b"
    pub model: String,
    /// Max parallel tool calls the planner may schedule per batch
    pub max_tools_per_batch: usize,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            default_backend: Backend::Ollama,
            ollama: OllamaConfig {
                base_url: "http://localhost:11434".to_string(),
                default_model: "llama3.2".to_string(),
            },
            nim: NimConfig {
                base_url: "https://integrate.api.nvidia.com/v1".to_string(),
                api_key: String::new(),
                default_model: "meta/llama-3.1-70b-instruct".to_string(),
            },
            ui: UiConfig {
                show_file_tree: true,
                syntax_highlight: true,
                mouse_enabled: true,
            },
            planner: PlannerConfig {
                backend: Backend::Ollama,
                // A small fast model; user should change to whatever they have pulled
                model: "qwen2.5:1.5b".to_string(),
                max_tools_per_batch: 5,
            },
        }
    }
}

impl Config {
    pub fn load() -> Result<Self> {
        let config_path = Self::config_path();

        if !config_path.exists() {
            let config = Config::default();
            config.save()?;
            return Ok(config);
        }

        let contents = std::fs::read_to_string(&config_path)?;
        // Use a permissive deserialize so old configs without [planner] still load
        let config: Config = toml::from_str(&contents).unwrap_or_else(|_| {
            let mut c = Config::default();
            // Try to at least recover the fields we know about
            if let Ok(partial) = toml::from_str::<toml::Value>(&contents) {
                if let Some(model) = partial
                    .get("ollama")
                    .and_then(|o| o.get("default_model"))
                    .and_then(|m| m.as_str())
                {
                    c.ollama.default_model = model.to_string();
                }
            }
            c
        });
        Ok(config)
    }

    pub fn save(&self) -> Result<()> {
        let config_path = Self::config_path();
        if let Some(parent) = config_path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let contents = toml::to_string_pretty(self)?;
        std::fs::write(&config_path, contents)?;
        Ok(())
    }

    pub fn config_path() -> PathBuf {
        dirs::config_dir()
            .unwrap_or_else(|| PathBuf::from("."))
            .join("ocode")
            .join("config.toml")
    }

    pub fn active_model(&self) -> &str {
        match self.default_backend {
            Backend::Ollama => &self.ollama.default_model,
            Backend::Nim => &self.nim.default_model,
        }
    }

    /// Build a backend instance for the planner model.
    pub fn planner_backend_url(&self) -> &str {
        match self.planner.backend {
            Backend::Ollama => &self.ollama.base_url,
            Backend::Nim => &self.nim.base_url,
        }
    }
}
