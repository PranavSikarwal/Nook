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

    pub fn load_env_file() {
        let candidates = [
            PathBuf::from(".env"),
            PathBuf::from("../.env"),
            PathBuf::from("../../.env"),
        ];
        for p in candidates {
            if p.exists() {
                if let Ok(content) = std::fs::read_to_string(&p) {
                    for line in content.lines() {
                        let trimmed = line.trim();
                        if trimmed.is_empty() || trimmed.starts_with('#') {
                            continue;
                        }
                        if let Some((k, v)) = trimmed.split_once('=') {
                            let k = k.trim();
                            let mut v = v.trim();
                            if (v.starts_with('"') && v.ends_with('"'))
                                || (v.starts_with('\'') && v.ends_with('\''))
                            {
                                v = &v[1..v.len() - 1];
                            }
                            if std::env::var_os(k).is_none() {
                                std::env::set_var(k, v);
                            }
                        }
                    }
                }
                break;
            }
        }
    }

    pub fn from_file(path: &Path) -> Result<Self, Box<dyn std::error::Error + Send + Sync>> {
        let mut cfg = if !path.exists() {
            let default_cfg = Self::default();
            if let Some(parent) = path.parent() {
                std::fs::create_dir_all(parent)?;
            }
            let toml_str = toml::to_string_pretty(&default_cfg)?;
            std::fs::write(path, toml_str)?;
            default_cfg
        } else {
            let content = std::fs::read_to_string(path)?;
            toml::from_str::<Self>(&content)?
        };

        if let Ok(val) = std::env::var("NOOK_BASE_URL") {
            if !val.trim().is_empty() {
                cfg.base_url = val;
            }
        }
        if let Ok(val) = std::env::var("NOOK_MODEL") {
            if !val.trim().is_empty() {
                cfg.model = val;
            }
        }
        if let Ok(val) = std::env::var("NOOK_DATABASE_URL") {
            if !val.trim().is_empty() {
                cfg.database_url = val;
            }
        }

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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_config() {
        let cfg = Config::default();
        assert_eq!(cfg.max_input_tokens, 1_000_000);
        assert_eq!(cfg.summarize_at_tokens, 750_000);
        assert_eq!(cfg.database_url, "postgres://localhost/nook");
    }

    #[test]
    fn test_save_and_load_config() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("config.toml");
        let cfg = Config {
            base_url: "https://example.com/v1".to_string(),
            model: "test-model".to_string(),
            database_url: "postgres://localhost/test_db".to_string(),
            max_input_tokens: 500_000,
            summarize_at_tokens: 350_000,
            worker_command: None,
        };
        cfg.save_to_file(&path).unwrap();

        let loaded = Config::from_file(&path).unwrap();
        assert_eq!(loaded.max_input_tokens, 500_000);
        assert_eq!(loaded.summarize_at_tokens, 350_000);
        if std::env::var("NOOK_BASE_URL").is_err() {
            assert_eq!(loaded.base_url, "https://example.com/v1");
        }
        if std::env::var("NOOK_MODEL").is_err() {
            assert_eq!(loaded.model, "test-model");
        }
        if std::env::var("NOOK_DATABASE_URL").is_err() {
            assert_eq!(loaded.database_url, "postgres://localhost/test_db");
        }
    }
}
