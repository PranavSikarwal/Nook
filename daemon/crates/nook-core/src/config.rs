use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RootAccess {
    Read,
    ReadWrite,
}

impl RootAccess {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Read => "read",
            Self::ReadWrite => "read_write",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RootConfig {
    pub path: String,
    pub access: RootAccess,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FilesystemConfig {
    #[serde(default)]
    pub roots: Vec<RootConfig>,
    #[serde(default = "default_max_file_bytes")]
    pub max_read_bytes: usize,
    #[serde(default = "default_max_file_bytes")]
    pub max_write_bytes: usize,
}

fn default_max_file_bytes() -> usize {
    1024 * 1024
}

impl Default for FilesystemConfig {
    fn default() -> Self {
        Self {
            roots: Vec::new(),
            max_read_bytes: default_max_file_bytes(),
            max_write_bytes: default_max_file_bytes(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TerminalConfig {
    #[serde(default)]
    pub enabled: bool,
    #[serde(default)]
    pub network: bool,
    #[serde(default = "default_command_timeout_seconds")]
    pub timeout_seconds: u64,
    #[serde(default = "default_command_output_bytes")]
    pub max_output_bytes: usize,
}

fn default_command_timeout_seconds() -> u64 {
    30
}

fn default_command_output_bytes() -> usize {
    64 * 1024
}

impl Default for TerminalConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            network: false,
            timeout_seconds: default_command_timeout_seconds(),
            max_output_bytes: default_command_output_bytes(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct McpServerConfig {
    pub name: String,
    pub command: String,
    #[serde(default)]
    pub args: Vec<String>,
    #[serde(default)]
    pub env: BTreeMap<String, String>,
    #[serde(default)]
    pub network: bool,
    #[serde(default)]
    pub read_paths: Vec<String>,
    #[serde(default)]
    pub write_paths: Vec<String>,
    #[serde(default = "default_mcp_timeout_seconds")]
    pub timeout_seconds: u64,
}

fn default_mcp_timeout_seconds() -> u64 {
    60
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct ToolsConfig {
    #[serde(default)]
    pub filesystem: FilesystemConfig,
    #[serde(default)]
    pub terminal: TerminalConfig,
    #[serde(default)]
    pub mcp_servers: Vec<McpServerConfig>,
}

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
    #[serde(default)]
    pub tools: ToolsConfig,
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
            tools: ToolsConfig::default(),
        }
    }
}

impl Config {
    pub fn app_dir() -> PathBuf {
        Self::app_dir_from(
            std::env::var_os("NOOK_APP_DIR").as_deref(),
            std::env::var_os("HOME").as_deref(),
            std::env::var_os("APPDATA").as_deref(),
            std::env::var_os("USERPROFILE").as_deref(),
            std::env::var_os("XDG_CONFIG_HOME").as_deref(),
        )
    }

    fn app_dir_from(
        app_dir: Option<&std::ffi::OsStr>,
        home: Option<&std::ffi::OsStr>,
        _appdata: Option<&std::ffi::OsStr>,
        _userprofile: Option<&std::ffi::OsStr>,
        _xdg_config_home: Option<&std::ffi::OsStr>,
    ) -> PathBuf {
        if let Some(path) = app_dir.filter(|path| !path.is_empty()) {
            return PathBuf::from(path);
        }

        #[cfg(target_os = "macos")]
        {
            if let Some(home) = home {
                PathBuf::from(home)
                    .join("Library")
                    .join("Application Support")
                    .join("Nook")
            } else {
                PathBuf::from("nook_data")
            }
        }
        #[cfg(target_os = "windows")]
        {
            if let Some(appdata) = _appdata {
                PathBuf::from(appdata).join("Nook")
            } else if let Some(userprofile) = _userprofile {
                PathBuf::from(userprofile)
                    .join("AppData")
                    .join("Roaming")
                    .join("Nook")
            } else {
                PathBuf::from("nook_data")
            }
        }
        #[cfg(all(not(target_os = "macos"), not(target_os = "windows")))]
        {
            if let Some(xdg) = _xdg_config_home {
                PathBuf::from(xdg).join("nook")
            } else if let Some(home) = home {
                PathBuf::from(home).join(".config").join("nook")
            } else {
                PathBuf::from("nook_data")
            }
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
        Self::load_env_file();

        let mut cfg = if !path.exists() {
            let default_cfg = Self::default();
            if let Some(parent) = path.parent() {
                std::fs::create_dir_all(parent)?;
            }
            let toml_str = toml::to_string_pretty(&default_cfg)?;
            write_private(path, &toml_str)?;
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

    /// Sets `base_url` and `model` in the file and leaves every other line, comment,
    /// and table as it is. Creates the file when it is missing.
    pub fn update_model_settings(
        path: &Path,
        base_url: &str,
        model: &str,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let text = match std::fs::read_to_string(path) {
            Ok(text) => text,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => String::new(),
            Err(error) => return Err(error.into()),
        };
        let mut document = text.parse::<toml_edit::DocumentMut>()?;
        document["base_url"] = toml_edit::value(base_url);
        document["model"] = toml_edit::value(model);
        write_private(path, &document.to_string())
    }
}

/// Writes the file so that only the user can read it. The file can hold secrets
/// in the `env` of an MCP server.
fn write_private(
    path: &Path,
    contents: &str,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    use std::io::Write;
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let temporary = path.with_extension("toml.tmp");
    let mut options = std::fs::OpenOptions::new();
    options.write(true).create(true).truncate(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600);
    }
    let mut file = options.open(&temporary)?;
    file.write_all(contents.as_bytes())?;
    file.sync_all()?;
    drop(file);
    std::fs::rename(&temporary, path)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn app_dir_uses_override() {
        let path = Config::app_dir_from(
            Some(std::ffi::OsStr::new("/tmp/nook-e2e")),
            None,
            None,
            None,
            None,
        );

        assert_eq!(path, PathBuf::from("/tmp/nook-e2e"));
    }

    #[test]
    fn blank_app_dir_uses_platform_default() {
        let path = Config::app_dir_from(
            Some(std::ffi::OsStr::new("")),
            Some(std::ffi::OsStr::new("/tmp/home")),
            Some(std::ffi::OsStr::new("/tmp/appdata")),
            Some(std::ffi::OsStr::new("/tmp/profile")),
            Some(std::ffi::OsStr::new("/tmp/xdg")),
        );

        #[cfg(target_os = "macos")]
        assert_eq!(
            path,
            PathBuf::from("/tmp/home")
                .join("Library")
                .join("Application Support")
                .join("Nook")
        );
        #[cfg(target_os = "windows")]
        assert_eq!(path, PathBuf::from("/tmp/appdata").join("Nook"));
        #[cfg(all(not(target_os = "macos"), not(target_os = "windows")))]
        assert_eq!(path, PathBuf::from("/tmp/xdg").join("nook"));
    }

    #[test]
    fn test_default_config() {
        let cfg = Config::default();
        assert_eq!(cfg.max_input_tokens, 1_000_000);
        assert_eq!(cfg.summarize_at_tokens, 750_000);
        assert_eq!(cfg.database_url, "postgres://localhost/nook");
    }

    const HAND_EDITED: &str = r#"# My Nook settings
base_url = "https://old.example.com/v1" # the proxy
model = "old-model"
database_url = "postgres://localhost/mine"

# Files the assistant may read
[tools.filesystem]
roots = [{ path = "/tmp/docs", access = "read" }]

[[tools.mcp_servers]]
name = "notes"
command = "/opt/tools/notes"
env = { TOKEN = "secret-value" }
"#;

    #[test]
    fn updating_the_model_keeps_comments_and_tool_blocks() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("config.toml");
        std::fs::write(&path, HAND_EDITED).unwrap();

        Config::update_model_settings(&path, "https://new.example.com/v1", "new-model").unwrap();

        let text = std::fs::read_to_string(&path).unwrap();
        assert!(text.contains("# My Nook settings"));
        assert!(text.contains("# Files the assistant may read"));
        assert!(text.contains("env = { TOKEN = \"secret-value\" }"));
        assert!(text.contains("database_url = \"postgres://localhost/mine\""));
        let loaded: Config = toml::from_str(&text).unwrap();
        assert_eq!(loaded.base_url, "https://new.example.com/v1");
        assert_eq!(loaded.model, "new-model");
        assert_eq!(loaded.tools.mcp_servers[0].name, "notes");
    }

    #[test]
    fn updating_a_missing_file_creates_it() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("nested").join("config.toml");

        Config::update_model_settings(&path, "https://a.example.com/v1", "m").unwrap();

        let loaded: Config = toml::from_str(&std::fs::read_to_string(&path).unwrap()).unwrap();
        assert_eq!(loaded.base_url, "https://a.example.com/v1");
        assert_eq!(loaded.model, "m");
    }

    #[test]
    fn a_file_that_is_not_toml_is_left_alone() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("config.toml");
        std::fs::write(&path, "base_url = [unclosed").unwrap();

        assert!(Config::update_model_settings(&path, "u", "m").is_err());

        assert_eq!(
            std::fs::read_to_string(&path).unwrap(),
            "base_url = [unclosed"
        );
    }

    #[cfg(unix)]
    #[test]
    fn the_config_file_is_private_to_the_user() {
        use std::os::unix::fs::PermissionsExt;
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("config.toml");
        std::fs::write(&path, HAND_EDITED).unwrap();
        std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o644)).unwrap();

        Config::update_model_settings(&path, "u", "m").unwrap();

        let mode = std::fs::metadata(&path).unwrap().permissions().mode() & 0o777;
        assert_eq!(mode, 0o600);
        let created = dir.path().join("created.toml");
        Config::update_model_settings(&created, "u", "m").unwrap();
        let mode = std::fs::metadata(&created).unwrap().permissions().mode() & 0o777;
        assert_eq!(mode, 0o600);
    }

    #[cfg(unix)]
    #[test]
    fn the_first_start_writes_a_private_default_file() {
        use std::os::unix::fs::PermissionsExt;
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("config.toml");

        Config::from_file(&path).unwrap();

        let mode = std::fs::metadata(&path).unwrap().permissions().mode() & 0o777;
        assert_eq!(mode, 0o600);
    }

    #[test]
    fn tools_default_to_off_and_empty() {
        let cfg: Config = toml::from_str("base_url = \"u\"\nmodel = \"m\"\n").unwrap();

        assert!(cfg.tools.filesystem.roots.is_empty());
        assert!(!cfg.tools.terminal.enabled);
        assert!(cfg.tools.mcp_servers.is_empty());
        assert_eq!(cfg.tools.filesystem.max_read_bytes, 1024 * 1024);
        assert_eq!(cfg.tools.terminal.timeout_seconds, 30);
    }

    #[test]
    fn tools_section_round_trips() {
        let text = r#"
base_url = "u"
model = "m"

[tools.filesystem]
roots = [
  { path = "/work/a", access = "read" },
  { path = "/work/b", access = "read_write" },
]

[tools.terminal]
enabled = true
network = true
timeout_seconds = 10

[[tools.mcp_servers]]
name = "files"
command = "npx"
args = ["-y", "server"]
env = { TOKEN = "x" }
read_paths = ["/opt/tools"]
write_paths = ["/work/cache"]
timeout_seconds = 20
"#;
        let cfg: Config = toml::from_str(text).unwrap();

        assert_eq!(cfg.tools.filesystem.roots[0].access, RootAccess::Read);
        assert_eq!(cfg.tools.filesystem.roots[1].access, RootAccess::ReadWrite);
        assert!(cfg.tools.terminal.enabled && cfg.tools.terminal.network);
        assert_eq!(cfg.tools.mcp_servers[0].args, ["-y", "server"]);
        assert_eq!(cfg.tools.mcp_servers[0].read_paths, ["/opt/tools"]);
        assert_eq!(cfg.tools.mcp_servers[0].timeout_seconds, 20);
        assert!(!cfg.tools.mcp_servers[0].network);
        let again: Config = toml::from_str(&toml::to_string_pretty(&cfg).unwrap()).unwrap();
        assert_eq!(again, cfg);
    }

    #[test]
    fn an_unknown_root_access_is_rejected() {
        let text = "base_url = \"u\"\nmodel = \"m\"\n[tools.filesystem]\nroots = [{ path = \"/a\", access = \"admin\" }]\n";

        assert!(toml::from_str::<Config>(text).is_err());
    }
}
