use super::roots::RootSet;
use super::{CallContext, Provider, ProviderError, ToolFacts, ToolOutput};
use crate::config::{RootAccess, TerminalConfig, ToolsConfig};
use crate::protocol::ToolDescriptor;
use crate::sandbox::{Sandbox, SandboxError, SandboxOutput, SandboxSpec};
use async_trait::async_trait;
use serde::Deserialize;
use serde_json::{json, Value};
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::time::Duration;

pub const RUN_COMMAND: &str = "nook:run_command";

pub const MAX_ARGUMENTS: usize = 64;
pub const MAX_ARGUMENT_BYTES: usize = 4096;
pub const MAX_TIMEOUT_SECONDS: u64 = 600;

const EMPTY_COMMAND: &str = "The command is empty";
const BAD_ARGUMENT: &str = "A command argument is not valid";

#[derive(Deserialize)]
struct CommandArgs {
    argv: Vec<String>,
    cwd: Option<String>,
    timeout_seconds: Option<u64>,
}

pub struct TerminalProvider {
    sandbox: Sandbox,
    roots: RootSet,
    network: bool,
    default_timeout: u64,
    max_output_bytes: usize,
}

struct Plan {
    argv: Vec<String>,
    cwd: PathBuf,
    root_index: usize,
    timeout: Duration,
}

impl TerminalProvider {
    /// Returns `None` when the terminal is off or has no folder to run in.
    pub fn new(config: &ToolsConfig, sandbox: Sandbox) -> Option<Self> {
        let terminal: &TerminalConfig = &config.terminal;
        if !terminal.enabled {
            return None;
        }
        let roots = RootSet::from_config(&config.filesystem.roots);
        if roots.is_empty() {
            tracing::warn!("The terminal tool is off: no usable folder is configured");
            return None;
        }
        Some(Self {
            sandbox,
            roots,
            network: terminal.network,
            default_timeout: terminal.timeout_seconds.clamp(1, MAX_TIMEOUT_SECONDS),
            max_output_bytes: terminal.max_output_bytes,
        })
    }

    fn plan(&self, arguments: &Value) -> Result<Plan, ProviderError> {
        let args: CommandArgs = serde_json::from_value(arguments.clone())
            .map_err(|_| ProviderError::InvalidArguments("Invalid arguments".to_string()))?;
        if args.argv.is_empty() || args.argv[0].is_empty() {
            return Err(ProviderError::Rejected(EMPTY_COMMAND));
        }
        if args.argv.len() > MAX_ARGUMENTS
            || args
                .argv
                .iter()
                .any(|part| part.contains('\0') || part.len() > MAX_ARGUMENT_BYTES)
        {
            return Err(ProviderError::Rejected(BAD_ARGUMENT));
        }
        let (root_index, cwd) = match &args.cwd {
            Some(raw) => {
                let resolved = self.roots.resolve(raw).map_err(ProviderError::Rejected)?;
                (resolved.index, resolved.canonical_path)
            }
            None => {
                let index = self
                    .roots
                    .default_index()
                    .ok_or(ProviderError::Rejected(super::roots::OUTSIDE_ROOTS))?;
                (index, self.roots.entries()[index].canonical.clone())
            }
        };
        let seconds = args
            .timeout_seconds
            .unwrap_or(self.default_timeout)
            .clamp(1, MAX_TIMEOUT_SECONDS);
        Ok(Plan {
            argv: args.argv,
            cwd,
            root_index,
            timeout: Duration::from_secs(seconds),
        })
    }
}

fn shell_quote(part: &str) -> String {
    let safe = !part.is_empty()
        && part
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || "-_./:=@%+,".contains(c));
    if safe {
        part.to_string()
    } else {
        format!("'{}'", part.replace('\'', "'\\''"))
    }
}

pub fn command_line(argv: &[String]) -> String {
    argv.iter()
        .map(|part| shell_quote(part))
        .collect::<Vec<_>>()
        .join(" ")
}

fn executable_name(argv0: &str) -> String {
    Path::new(argv0)
        .file_name()
        .map(|name| name.to_string_lossy().into_owned())
        .unwrap_or_else(|| argv0.to_string())
}

fn sandbox_path() -> String {
    let mut path = String::from("/usr/local/bin:/usr/bin:/bin:/usr/sbin:/sbin");
    if cfg!(target_os = "macos") && Path::new("/opt/homebrew/bin").is_dir() {
        path = format!("/opt/homebrew/bin:{path}");
    }
    path
}

fn describe(output: &SandboxOutput, seconds: u64) -> String {
    let mut text = if output.timed_out {
        format!("timed out after {seconds} seconds\n")
    } else if let Some(code) = output.exit_code {
        format!("exit code: {code}\n")
    } else if let Some(signal) = output.signal {
        format!("killed by signal {signal}\n")
    } else {
        "ended without an exit code\n".to_string()
    };
    text.push_str("--- stdout ---\n");
    text.push_str(&output.stdout);
    if !output.stdout.ends_with('\n') && !output.stdout.is_empty() {
        text.push('\n');
    }
    text.push_str("--- stderr ---\n");
    text.push_str(&output.stderr);
    if !output.stderr.ends_with('\n') && !output.stderr.is_empty() {
        text.push('\n');
    }
    if output.truncated {
        text.push_str("[output truncated]\n");
    }
    text
}

#[async_trait]
impl Provider for TerminalProvider {
    fn descriptors(&self) -> Vec<ToolDescriptor> {
        vec![ToolDescriptor {
            id: RUN_COMMAND.to_string(),
            name: "run_command".to_string(),
            description: format!(
                "Run one command on the user's computer inside a sandbox. Give the program and its arguments as a list. No shell runs unless you start one yourself. The command sees only the folder it runs in{}. The default time limit is {} seconds. {}",
                if self.network { ", and it can use the network" } else { ", and it cannot use the network" },
                self.default_timeout,
                self.roots.describe(),
            ),
            input_schema: json!({
                "type": "object",
                "properties": {
                    "argv": {
                        "type": "array",
                        "items": { "type": "string", "maxLength": MAX_ARGUMENT_BYTES },
                        "minItems": 1,
                        "maxItems": MAX_ARGUMENTS,
                        "description": "The program, then its arguments."
                    },
                    "cwd": {
                        "type": "string",
                        "description": "Absolute folder to run in. It must be inside an allowed folder. Defaults to the first writable allowed folder."
                    },
                    "timeout_seconds": {
                        "type": "integer",
                        "minimum": 1,
                        "maximum": MAX_TIMEOUT_SECONDS
                    }
                },
                "required": ["argv"],
                "additionalProperties": false
            }),
        }]
    }

    fn facts(&self, tool_id: &str, arguments: &Value) -> Result<ToolFacts, ProviderError> {
        if tool_id != RUN_COMMAND {
            return Err(ProviderError::UnknownTool);
        }
        let plan = self.plan(arguments)?;
        let root = &self.roots.entries()[plan.root_index];
        let line = command_line(&plan.argv);
        let network_note = if self.network { " (network on)" } else { "" };
        let mut context = BTreeMap::new();
        context.insert("executable".to_string(), plan.argv[0].clone());
        context.insert(
            "executable_name".to_string(),
            executable_name(&plan.argv[0]),
        );
        context.insert("command_line".to_string(), line.clone());
        context.insert("cwd".to_string(), plan.cwd.display().to_string());
        context.insert("root".to_string(), root.canonical.display().to_string());
        context.insert("access".to_string(), root.access.as_str().to_string());
        context.insert("network".to_string(), self.network.to_string());
        Ok(ToolFacts {
            summary: format!("Run `{line}` in {}{network_note}", plan.cwd.display()),
            target_host: None,
            target_path: None,
            target_dir: None,
            context,
        })
    }

    async fn call(&self, tool_id: &str, arguments: Value, _context: CallContext) -> ToolOutput {
        if tool_id != RUN_COMMAND {
            return ToolOutput::error("Error: unknown tool");
        }
        let Ok(plan) = self.plan(&arguments) else {
            return ToolOutput::error("Error: the command is not valid");
        };
        let root = &self.roots.entries()[plan.root_index];
        let home = match tempfile::tempdir() {
            Ok(dir) => dir,
            Err(_) => return ToolOutput::error("Error: could not prepare the command"),
        };
        let home_path = match home.path().canonicalize() {
            Ok(path) => path,
            Err(_) => return ToolOutput::error("Error: could not prepare the command"),
        };
        let mut write_roots = vec![home_path.clone()];
        let mut read_roots = Vec::new();
        match root.access {
            RootAccess::ReadWrite => write_roots.push(root.canonical.clone()),
            RootAccess::Read => read_roots.push(root.canonical.clone()),
        }
        let home_text = home_path.display().to_string();
        let env = BTreeMap::from([
            ("PATH".to_string(), sandbox_path()),
            ("HOME".to_string(), home_text.clone()),
            ("TMPDIR".to_string(), home_text),
            ("LANG".to_string(), "C.UTF-8".to_string()),
            ("TERM".to_string(), "dumb".to_string()),
            ("NO_COLOR".to_string(), "1".to_string()),
        ]);
        let seconds = plan.timeout.as_secs();
        let spec = SandboxSpec {
            argv: plan.argv,
            cwd: plan.cwd,
            read_roots,
            write_roots,
            network: self.network,
            env,
            timeout: plan.timeout,
            max_output_bytes: (self.max_output_bytes / 2).max(1024),
        };
        let result = self.sandbox.run(spec).await;
        drop(home);
        match result {
            Ok(output) => {
                let text = describe(&output, seconds);
                if output.timed_out || output.exit_code.is_none() {
                    ToolOutput::error(text)
                } else {
                    ToolOutput::ok(text)
                }
            }
            Err(SandboxError::Spawn(_)) => ToolOutput::error("Error: the command could not start"),
            Err(_) => ToolOutput::error("Error: the sandbox failed"),
        }
    }
}
