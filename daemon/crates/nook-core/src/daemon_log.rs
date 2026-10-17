use crate::config::Config;
use std::path::{Path, PathBuf};
use tokio::fs::File;
use tokio::io::{AsyncBufReadExt, AsyncRead, AsyncWriteExt, BufReader};

pub const MAX_LOG_BYTES: u64 = 1024 * 1024;
const MIN_SECRET_LENGTH: usize = 4;

/// Values that must not reach the log file: the database password, the user and
/// password pair of the database address, the API key from the environment, and
/// the `env` values of the MCP servers.
pub fn secrets_to_redact(config: &Config, environment_api_key: Option<&str>) -> Vec<String> {
    let mut secrets = Vec::new();
    let mut database_urls = vec![config.database_url.clone()];
    if let Ok(url) = std::env::var("NOOK_DATABASE_URL") {
        database_urls.push(url);
    }
    for raw in database_urls {
        let Ok(url) = url::Url::parse(&raw) else {
            continue;
        };
        if let Some(password) = url.password() {
            secrets.push(format!("{}:{password}", url.username()));
            secrets.push(password.to_string());
            if let Ok(decoded) = urlencoding_decode(password) {
                secrets.push(decoded);
            }
        }
    }
    if let Some(key) = environment_api_key {
        secrets.push(key.to_string());
    }
    for server in &config.tools.mcp_servers {
        secrets.extend(server.env.values().cloned());
    }
    secrets.retain(|secret| secret.contains(':') || secret.len() >= MIN_SECRET_LENGTH);
    secrets.sort_by_key(|secret| std::cmp::Reverse(secret.len()));
    secrets.dedup();
    secrets
}

fn urlencoding_decode(text: &str) -> Result<String, std::string::FromUtf8Error> {
    let bytes = text.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut index = 0;
    while index < bytes.len() {
        if bytes[index] == b'%' && index + 2 < bytes.len() {
            let digits = std::str::from_utf8(&bytes[index + 1..index + 3]).unwrap_or("");
            if let Ok(value) = u8::from_str_radix(digits, 16) {
                out.push(value);
                index += 3;
                continue;
            }
        }
        out.push(bytes[index]);
        index += 1;
    }
    String::from_utf8(out)
}

pub fn redact_line(line: &str, secrets: &[String]) -> String {
    secrets.iter().fold(line.to_owned(), |line, secret| {
        if secret.is_empty() {
            line
        } else {
            line.replace(secret, "***")
        }
    })
}

/// A log file that keeps one older file. When the file reaches `max_bytes`, it
/// becomes `<name>.1` and a new file starts. Only the user can read the files.
pub struct RotatingLog {
    path: PathBuf,
    file: File,
    written: u64,
    max_bytes: u64,
}

fn older_path(path: &Path) -> PathBuf {
    let mut name = path.file_name().unwrap_or_default().to_os_string();
    name.push(".1");
    path.with_file_name(name)
}

async fn open_private(path: &Path) -> std::io::Result<File> {
    let mut options = tokio::fs::OpenOptions::new();
    options.create(true).append(true);
    #[cfg(unix)]
    options.mode(0o600);
    let file = options.open(path).await?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        tokio::fs::set_permissions(path, std::fs::Permissions::from_mode(0o600)).await?;
    }
    Ok(file)
}

impl RotatingLog {
    pub async fn open(path: PathBuf, max_bytes: u64) -> std::io::Result<Self> {
        if let Some(parent) = path.parent() {
            tokio::fs::create_dir_all(parent).await?;
        }
        let written = tokio::fs::metadata(&path)
            .await
            .map(|m| m.len())
            .unwrap_or(0);
        let file = open_private(&path).await?;
        Ok(Self {
            path,
            file,
            written,
            max_bytes,
        })
    }

    pub async fn write_line(&mut self, line: &str) -> std::io::Result<()> {
        let size = line.len() as u64 + 1;
        if self.written > 0 && self.written + size > self.max_bytes {
            self.file.flush().await?;
            tokio::fs::rename(&self.path, older_path(&self.path)).await?;
            self.file = open_private(&self.path).await?;
            self.written = 0;
        }
        self.file.write_all(line.as_bytes()).await?;
        self.file.write_all(b"\n").await?;
        self.written += size;
        Ok(())
    }
}

/// Copies lines from `reader` to the log with the secrets removed. It reads to
/// the end of the stream even when the log cannot be written, so the process on
/// the other side never blocks on a full pipe.
pub async fn capture<R: AsyncRead + Unpin>(reader: R, mut log: RotatingLog, secrets: Vec<String>) {
    let mut lines = BufReader::new(reader).lines();
    let mut writable = true;
    while let Ok(Some(line)) = lines.next_line().await {
        if writable && log.write_line(&redact_line(&line, &secrets)).await.is_err() {
            writable = false;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::McpServerConfig;
    use std::collections::BTreeMap;
    use tokio::io::{duplex, AsyncWriteExt};

    fn config() -> Config {
        let mut config = Config {
            database_url: "postgres://nook:s3cr%40t!@db.example.com:5432/nook".to_string(),
            ..Config::default()
        };
        config.tools.mcp_servers.push(McpServerConfig {
            name: "notes".to_string(),
            command: "/bin/true".to_string(),
            args: vec![],
            env: BTreeMap::from([
                ("TOKEN".to_string(), "tok-12345".to_string()),
                ("MODE".to_string(), "on".to_string()),
            ]),
            network: false,
            read_paths: vec![],
            write_paths: vec![],
            timeout_seconds: 60,
        });
        config
    }

    #[test]
    fn secrets_cover_the_password_the_user_pair_the_key_and_server_values() {
        let secrets = secrets_to_redact(&config(), Some("sk-live-abcdef"));

        assert!(secrets.contains(&"nook:s3cr%40t!".to_string()));
        assert!(secrets.contains(&"s3cr%40t!".to_string()));
        assert!(secrets.contains(&"s3cr@t!".to_string()));
        assert!(secrets.contains(&"sk-live-abcdef".to_string()));
        assert!(secrets.contains(&"tok-12345".to_string()));
        assert!(
            !secrets.contains(&"on".to_string()),
            "a short value is not a secret"
        );
    }

    #[test]
    fn a_line_loses_every_secret_and_keeps_the_rest() {
        let secrets = secrets_to_redact(&config(), Some("sk-live-abcdef"));
        let line = "connect nook:s3cr%40t!@db.example.com failed, key sk-live-abcdef, tok-12345 and s3cr@t!";

        let clean = redact_line(line, &secrets);

        assert_eq!(
            clean,
            "connect ***@db.example.com failed, key ***, *** and ***"
        );
    }

    #[test]
    fn a_short_password_is_hidden_inside_the_user_pair() {
        let config = Config {
            database_url: "postgres://nook:pw@localhost/nook".to_string(),
            ..Config::default()
        };

        let secrets = secrets_to_redact(&config, None);

        assert_eq!(
            redact_line("error for nook:pw@localhost", &secrets),
            "error for ***@localhost"
        );
        assert_eq!(redact_line("a pw in a word", &secrets), "a pw in a word");
    }

    #[tokio::test]
    async fn the_log_rotates_and_keeps_one_older_file() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("daemon.log");
        let mut log = RotatingLog::open(path.clone(), 40).await.unwrap();

        for index in 0..6 {
            log.write_line(&format!("line number {index} here"))
                .await
                .unwrap();
        }

        let current = std::fs::read_to_string(&path).unwrap();
        let older = std::fs::read_to_string(dir.path().join("daemon.log.1")).unwrap();
        assert!(current.contains("line number 5 here"));
        assert!(!older.is_empty());
        assert!(current.len() <= 40 && older.len() <= 40);
        assert!(!dir.path().join("daemon.log.2").exists());
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn the_log_files_are_private() {
        use std::os::unix::fs::PermissionsExt;
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("daemon.log");
        std::fs::write(&path, "old\n").unwrap();
        std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o644)).unwrap();
        let mut log = RotatingLog::open(path.clone(), 10).await.unwrap();

        log.write_line("this line forces a rotation").await.unwrap();

        for name in ["daemon.log", "daemon.log.1"] {
            let mode = std::fs::metadata(dir.path().join(name))
                .unwrap()
                .permissions()
                .mode();
            assert_eq!(mode & 0o777, 0o600, "{name}");
        }
    }

    #[tokio::test]
    async fn capture_copies_redacted_lines_until_the_stream_ends() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("daemon.log");
        let log = RotatingLog::open(path.clone(), MAX_LOG_BYTES)
            .await
            .unwrap();
        let (mut writer, reader) = duplex(1024);
        let task = tokio::spawn(capture(reader, log, vec!["hunter22".to_string()]));

        writer
            .write_all(b"start\npassword hunter22 used\nend\n")
            .await
            .unwrap();
        drop(writer);
        task.await.unwrap();

        assert_eq!(
            std::fs::read_to_string(&path).unwrap(),
            "start\npassword *** used\nend\n"
        );
    }
}
