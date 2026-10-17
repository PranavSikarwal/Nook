use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::time::Duration;
use thiserror::Error;

#[cfg(unix)]
use std::process::Stdio;
#[cfg(unix)]
use tokio::io::{AsyncRead, AsyncReadExt};
#[cfg(unix)]
use tokio::process::Command;

#[cfg(unix)]
const READER_DRAIN_TIMEOUT: Duration = Duration::from_secs(2);

#[derive(Debug, Clone)]
pub struct SandboxSpec {
    pub argv: Vec<String>,
    pub cwd: PathBuf,
    pub read_roots: Vec<PathBuf>,
    pub write_roots: Vec<PathBuf>,
    pub network: bool,
    pub env: BTreeMap<String, String>,
    pub timeout: Duration,
    pub max_output_bytes: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SandboxOutput {
    pub exit_code: Option<i32>,
    pub signal: Option<i32>,
    pub timed_out: bool,
    pub stdout: String,
    pub stderr: String,
    pub truncated: bool,
}

#[derive(Debug, Error)]
pub enum SandboxError {
    #[error("Sandbox unavailable: {0}")]
    Unavailable(String),
    #[error("Command could not start: {0}")]
    Spawn(String),
    #[error("Sandbox I/O error: {0}")]
    Io(String),
}

#[allow(dead_code)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Backend {
    Seatbelt,
    Bubblewrap { mount_proc: bool },
}

#[derive(Debug, Clone)]
pub struct Sandbox {
    backend: Backend,
}

impl Sandbox {
    pub fn name(&self) -> &'static str {
        match self.backend {
            Backend::Seatbelt => "seatbelt",
            Backend::Bubblewrap { .. } => "bubblewrap",
        }
    }

    #[cfg(target_os = "macos")]
    pub async fn detect() -> Result<Self, SandboxError> {
        let sandbox = Self {
            backend: Backend::Seatbelt,
        };
        sandbox.self_test().await?;
        Ok(sandbox)
    }

    #[cfg(target_os = "linux")]
    pub async fn detect() -> Result<Self, SandboxError> {
        let mut last_error = String::new();
        for mount_proc in [true, false] {
            let sandbox = Self {
                backend: Backend::Bubblewrap { mount_proc },
            };
            match sandbox.self_test().await {
                Ok(()) => return Ok(sandbox),
                Err(error) => last_error = error.to_string(),
            }
        }
        Err(SandboxError::Unavailable(last_error))
    }

    #[cfg(not(any(target_os = "macos", target_os = "linux")))]
    pub async fn detect() -> Result<Self, SandboxError> {
        Err(SandboxError::Unavailable(
            "no sandbox exists for this platform".to_string(),
        ))
    }

    #[cfg(unix)]
    async fn self_test(&self) -> Result<(), SandboxError> {
        let dir = tempfile::tempdir().map_err(|e| SandboxError::Io(e.to_string()))?;
        let cwd = dir
            .path()
            .canonicalize()
            .map_err(|e| SandboxError::Io(e.to_string()))?;
        let spec = SandboxSpec {
            argv: vec!["/usr/bin/true".to_string()],
            cwd: cwd.clone(),
            read_roots: vec![],
            write_roots: vec![cwd],
            network: false,
            env: BTreeMap::new(),
            timeout: Duration::from_secs(10),
            max_output_bytes: 4096,
        };
        let output = self.run(spec).await?;
        if output.exit_code == Some(0) {
            Ok(())
        } else {
            let detail = output.stderr.trim();
            Err(SandboxError::Unavailable(format!(
                "{} self-test failed: {}",
                self.name(),
                if detail.is_empty() {
                    "no output"
                } else {
                    detail
                }
            )))
        }
    }

    #[cfg(not(unix))]
    pub async fn run(&self, _spec: SandboxSpec) -> Result<SandboxOutput, SandboxError> {
        Err(SandboxError::Unavailable(
            "no sandbox exists for this platform".to_string(),
        ))
    }

    #[cfg(unix)]
    fn command(&self, spec: &SandboxSpec) -> Command {
        match self.backend {
            Backend::Seatbelt => {
                let mut command = Command::new("/usr/bin/sandbox-exec");
                command.arg("-p").arg(seatbelt_profile(spec));
                command.args(&spec.argv);
                command.current_dir(&spec.cwd);
                command.env_clear();
                command.envs(&spec.env);
                command
            }
            Backend::Bubblewrap { mount_proc } => {
                let mut command = Command::new("bwrap");
                command.args(bubblewrap_args(spec, mount_proc));
                command.env_clear();
                if let Some(path) = spec.env.get("PATH") {
                    command.env("PATH", path);
                }
                command
            }
        }
    }

    /// Starts a long-running process with piped standard streams. Dropping the
    /// handle kills the whole process group.
    #[cfg(unix)]
    pub fn spawn(&self, spec: &SandboxSpec) -> Result<SandboxProcess, SandboxError> {
        if spec.argv.is_empty() {
            return Err(SandboxError::Spawn("empty command".to_string()));
        }
        let mut command = self.command(spec);
        command
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .kill_on_drop(true)
            .process_group(0);
        let child = command
            .spawn()
            .map_err(|e| SandboxError::Spawn(e.to_string()))?;
        let group = GroupKill(child.id().map(|pid| pid as i32));
        Ok(SandboxProcess {
            child,
            _group: group,
        })
    }

    #[cfg(unix)]
    pub async fn run(&self, spec: SandboxSpec) -> Result<SandboxOutput, SandboxError> {
        if spec.argv.is_empty() {
            return Err(SandboxError::Spawn("empty command".to_string()));
        }
        let mut command = self.command(&spec);
        command
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .kill_on_drop(true)
            .process_group(0);

        let mut child = command
            .spawn()
            .map_err(|e| SandboxError::Spawn(e.to_string()))?;
        let pid = child.id().map(|pid| pid as i32);
        let _group = GroupKill(pid);
        let stdout = child.stdout.take().expect("stdout is piped");
        let stderr = child.stderr.take().expect("stderr is piped");
        let max = spec.max_output_bytes;
        let stdout_task = tokio::spawn(read_capped(stdout, max));
        let stderr_task = tokio::spawn(read_capped(stderr, max));

        let mut timed_out = false;
        let status = tokio::select! {
            status = child.wait() => Some(status.map_err(|e| SandboxError::Io(e.to_string()))?),
            _ = tokio::time::sleep(spec.timeout) => {
                timed_out = true;
                None
            }
        };
        if timed_out {
            kill_group(pid);
            let _ = child.kill().await;
        } else {
            kill_group(pid);
        }

        let (stdout, stdout_truncated) = join_reader(stdout_task).await;
        let (stderr, stderr_truncated) = join_reader(stderr_task).await;
        let (exit_code, signal) = match status {
            Some(status) => {
                use std::os::unix::process::ExitStatusExt;
                (status.code(), status.signal())
            }
            None => (None, None),
        };
        Ok(SandboxOutput {
            exit_code,
            signal,
            timed_out,
            stdout: String::from_utf8_lossy(&stdout).into_owned(),
            stderr: String::from_utf8_lossy(&stderr).into_owned(),
            truncated: stdout_truncated || stderr_truncated,
        })
    }
}

#[cfg(unix)]
pub struct SandboxProcess {
    pub child: tokio::process::Child,
    _group: GroupKill,
}

#[cfg(unix)]
struct GroupKill(Option<i32>);

#[cfg(unix)]
impl Drop for GroupKill {
    fn drop(&mut self) {
        kill_group(self.0);
    }
}

#[cfg(unix)]
fn kill_group(pid: Option<i32>) {
    if let Some(pid) = pid.filter(|pid| *pid > 1) {
        unsafe {
            libc::killpg(pid, libc::SIGKILL);
        }
    }
}

#[cfg(unix)]
async fn read_capped<R: AsyncRead + Unpin>(mut reader: R, max: usize) -> (Vec<u8>, bool) {
    let mut kept = Vec::new();
    let mut truncated = false;
    let mut buffer = [0u8; 8192];
    loop {
        match reader.read(&mut buffer).await {
            Ok(0) | Err(_) => break,
            Ok(count) => {
                let room = max.saturating_sub(kept.len());
                if count > room {
                    truncated = true;
                }
                kept.extend_from_slice(&buffer[..count.min(room)]);
            }
        }
    }
    (kept, truncated)
}

#[cfg(unix)]
async fn join_reader(task: tokio::task::JoinHandle<(Vec<u8>, bool)>) -> (Vec<u8>, bool) {
    match tokio::time::timeout(READER_DRAIN_TIMEOUT, task).await {
        Ok(Ok(result)) => result,
        _ => (Vec::new(), false),
    }
}

fn sbpl_string(path: &Path) -> String {
    let text = path.to_string_lossy();
    format!("\"{}\"", text.replace('\\', "\\\\").replace('"', "\\\""))
}

const SEATBELT_SYSTEM_READ: &[&str] = &[
    "/usr",
    "/bin",
    "/sbin",
    "/System",
    "/Library",
    "/private/etc",
    "/private/var/db/dyld",
    "/private/var/db/timezone",
    "/opt/homebrew",
    "/dev",
];

pub fn seatbelt_profile(spec: &SandboxSpec) -> String {
    let mut profile = String::from(
        "(version 1)\n(deny default)\n(import \"/System/Library/Sandbox/Profiles/bsd.sb\")\n(allow process-exec)\n(allow process-fork)\n",
    );
    let mut read_paths: Vec<&Path> = SEATBELT_SYSTEM_READ.iter().map(Path::new).collect();
    read_paths.extend(spec.read_roots.iter().map(PathBuf::as_path));
    read_paths.extend(spec.write_roots.iter().map(PathBuf::as_path));
    read_paths.push(&spec.cwd);
    let mut ancestors: Vec<PathBuf> = Vec::new();
    for path in &read_paths {
        for ancestor in path.ancestors() {
            if !ancestors.iter().any(|known| known == ancestor) {
                ancestors.push(ancestor.to_path_buf());
            }
        }
    }
    profile.push_str("(allow file-read-metadata");
    for ancestor in &ancestors {
        profile.push_str(&format!(" (literal {})", sbpl_string(ancestor)));
    }
    profile.push_str(")\n(allow file-read*");
    for path in &read_paths {
        profile.push_str(&format!(" (subpath {})", sbpl_string(path)));
    }
    profile.push_str(")\n");
    if !spec.write_roots.is_empty() {
        profile.push_str("(allow file-write*");
        for root in &spec.write_roots {
            profile.push_str(&format!(" (subpath {})", sbpl_string(root)));
        }
        profile.push_str(")\n");
    }
    if spec.network {
        profile.push_str("(allow network*)\n");
    } else {
        profile.push_str("(deny network*)\n");
    }
    profile
}

pub fn bubblewrap_args(spec: &SandboxSpec, mount_proc: bool) -> Vec<String> {
    let mut args: Vec<String> = ["--die-with-parent", "--new-session", "--unshare-all"]
        .map(String::from)
        .to_vec();
    if spec.network {
        args.push("--share-net".to_string());
    }
    args.extend(["--ro-bind", "/usr", "/usr"].map(String::from));
    for link_or_dir in ["/bin", "/sbin", "/lib", "/lib32", "/lib64"] {
        match std::fs::read_link(link_or_dir) {
            Ok(target) => {
                args.extend(["--symlink".to_string(), target.display().to_string()]);
                args.push(link_or_dir.to_string());
            }
            Err(_) if Path::new(link_or_dir).is_dir() => {
                args.extend(["--ro-bind", link_or_dir, link_or_dir].map(String::from));
            }
            Err(_) => {}
        }
    }
    for etc in [
        "/etc/alternatives",
        "/etc/ld.so.cache",
        "/etc/ssl",
        "/etc/ca-certificates",
        "/etc/resolv.conf",
        "/etc/hosts",
        "/etc/passwd",
        "/etc/group",
        "/etc/nsswitch.conf",
        "/etc/localtime",
    ] {
        args.extend(["--ro-bind-try", etc, etc].map(String::from));
    }
    if mount_proc {
        args.extend(["--proc", "/proc"].map(String::from));
    }
    args.extend(["--dev", "/dev", "--tmpfs", "/tmp"].map(String::from));
    for root in &spec.read_roots {
        let root = root.display().to_string();
        args.extend(["--ro-bind".to_string(), root.clone(), root]);
    }
    for root in &spec.write_roots {
        let root = root.display().to_string();
        args.extend(["--bind".to_string(), root.clone(), root]);
    }
    args.extend(["--chdir".to_string(), spec.cwd.display().to_string()]);
    args.push("--clearenv".to_string());
    for (key, value) in &spec.env {
        args.extend(["--setenv".to_string(), key.clone(), value.clone()]);
    }
    args.push("--".to_string());
    args.extend(spec.argv.iter().cloned());
    args
}
