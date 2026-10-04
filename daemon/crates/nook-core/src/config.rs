use std::path::{Path, PathBuf};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Config {
    pub base_url: String,
    pub model: String,
    #[serde(default = "default_database_url")]
    pub database_url: String,
    #[serde(default = "default_max_input_tokens")]
    pub max_input_tokens: i64,
    #[serde(default = "default_summarize_at_tokens")]
    pub summarize_at_tokens: i64,
    #[serde(default)]
    pub worker_command: Option<String>,
}

fn default_database_url() -> String {
    "postgres://localhost/nook".to_string()
}

fn default_max_input_tokens() -> i64 {
    1_000_000
}

fn default_summarize_at_tokens() -> i64 {
    750_000
}

impl Default for Config {
    fn default() -> Self {
        Self {
            base_url: "https://models.example.internal/v1".to_string(),
            model: "default-model".to_string(),
            database_url: default_database_url(),
            max_input_tokens: default_max_input_tokens(),
            summarize_at_tokens: default_summarize_at_tokens(),
            worker_command: None,
        }
    }
}

impl Config {
    pub fn app_dir() -> PathBuf {
        if let Some(home) = std::env::var_os("HOME") {
            PathBuf::from(home)
                .join("Library")
                .join("Application Support")
                .join("Nook")
        } else {
            PathBuf::from("nook_data")
        }
    }

    pub fn default_config_path() -> PathBuf {
        Self::app_dir().join("config.toml")
    }

    pub fn default_socket_path() -> PathBuf {
        Self::app_dir().join("daemon.sock")
    }

    pub fn default_attachments_dir() -> PathBuf {
        Self::app_dir().join("attachments")
    }

    pub fn from_file(path: &Path) -> Result<Self, Box<dyn std::error::Error + Send + Sync>> {
        if !path.exists() {
            let default_cfg = Self::default();
            if let Some(parent) = path.parent() {
                std::fs::create_dir_all(parent)?;
            }
            let toml_str = toml::to_string_pretty(&default_cfg)?;
            std::fs::write(path, toml_str)?;
            return Ok(default_cfg);
        }
        let content = std::fs::read_to_string(path)?;
        let cfg: Self = toml::from_str(&content)?;
        Ok(cfg)
    }

    pub fn save_to_file(&self, path: &Path) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let toml_str = toml::to_string_pretty(self)?;
        std::fs::write(path, toml_str)?;
        Ok(())
    }
}
