use super::roots::{RootEntry, RootSet};
use super::{CallContext, Provider, ProviderError, ToolFacts, ToolOutput};
use crate::config::{FilesystemConfig, RootAccess};
use crate::protocol::ToolDescriptor;
use async_trait::async_trait;
use cap_std::ambient_authority;
use cap_std::fs::{Dir, OpenOptions};
use serde::Deserialize;
use serde_json::{json, Value};
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use tracing::warn;

pub const READ_FILE: &str = "nook:read_local_file";
pub const LIST_DIRECTORY: &str = "nook:list_local_directory";
pub const WRITE_FILE: &str = "nook:write_local_file";

const MAX_LISTED_ENTRIES: usize = 1000;

const READ_ONLY: &str = "The folder is read-only";
const WRITE_TOO_LARGE: &str = "Content is larger than the write limit";

#[derive(Debug)]
pub struct FilesystemProvider {
    roots: RootSet,
    dirs: Vec<Arc<Dir>>,
    max_read_bytes: usize,
    max_write_bytes: usize,
}

#[derive(Deserialize)]
struct PathArgs {
    path: String,
}

#[derive(Deserialize)]
struct WriteArgs {
    path: String,
    content: String,
    #[serde(default)]
    overwrite: bool,
}

impl FilesystemProvider {
    /// Returns `None` when no configured root is usable.
    pub fn from_config(config: &FilesystemConfig) -> Option<Self> {
        let candidates = RootSet::from_config(&config.roots);
        let mut entries: Vec<RootEntry> = Vec::new();
        let mut dirs = Vec::new();
        for entry in candidates.entries() {
            match Dir::open_ambient_dir(&entry.canonical, ambient_authority()) {
                Ok(dir) => {
                    entries.push(entry.clone());
                    dirs.push(Arc::new(dir));
                }
                Err(error) => warn!(
                    "Ignored filesystem root {}: {error}",
                    entry.configured.display()
                ),
            }
        }
        if entries.is_empty() {
            return None;
        }
        Some(Self {
            roots: RootSet::from_entries(entries),
            dirs,
            max_read_bytes: config.max_read_bytes,
            max_write_bytes: config.max_write_bytes,
        })
    }

    fn descriptor(id: &str, name: &str, description: &str, schema: Value) -> ToolDescriptor {
        ToolDescriptor {
            id: id.to_string(),
            name: name.to_string(),
            description: description.to_string(),
            input_schema: schema,
        }
    }
}

fn path_schema() -> Value {
    json!({
        "type": "object",
        "properties": {
            "path": {
                "type": "string",
                "description": "Absolute path inside an allowed folder."
            }
        },
        "required": ["path"],
        "additionalProperties": false
    })
}

fn parse<T: for<'de> Deserialize<'de>>(arguments: &Value) -> Result<T, ProviderError> {
    serde_json::from_value(arguments.clone())
        .map_err(|_| ProviderError::InvalidArguments("Invalid arguments".to_string()))
}

fn relative_or_dot(relative: &Path) -> &Path {
    if relative.as_os_str().is_empty() {
        Path::new(".")
    } else {
        relative
    }
}

fn io_error(error: &std::io::Error) -> String {
    use std::io::ErrorKind;
    match error.kind() {
        ErrorKind::NotFound => "Error: path not found".to_string(),
        ErrorKind::PermissionDenied => {
            "Error: access denied. The path may leave the allowed folder through a link".to_string()
        }
        ErrorKind::AlreadyExists => {
            "Error: file already exists. Set overwrite to true to replace it".to_string()
        }
        _ => "Error: the operation failed".to_string(),
    }
}

#[async_trait]
impl Provider for FilesystemProvider {
    fn descriptors(&self) -> Vec<ToolDescriptor> {
        let allowed = self.roots.describe();
        let mut tools = vec![
            Self::descriptor(
                READ_FILE,
                "read_local_file",
                &format!(
                    "Read a UTF-8 text file from the user's computer. The path must be inside a folder the user allowed. {}",
                    allowed
                ),
                path_schema(),
            ),
            Self::descriptor(
                LIST_DIRECTORY,
                "list_local_directory",
                &format!(
                    "List the entries of a folder on the user's computer. The path must be inside a folder the user allowed. {}",
                    allowed
                ),
                path_schema(),
            ),
        ];
        if self.roots.has_write_access() {
            tools.push(Self::descriptor(
                WRITE_FILE,
                "write_local_file",
                &format!(
                    "Write a UTF-8 text file on the user's computer. The path must be inside a folder the user allowed to change. An existing file is replaced only when overwrite is true. {}",
                    allowed
                ),
                json!({
                    "type": "object",
                    "properties": {
                        "path": {
                            "type": "string",
                            "description": "Absolute path inside an allowed folder."
                        },
                        "content": { "type": "string" },
                        "overwrite": { "type": "boolean", "default": false }
                    },
                    "required": ["path", "content"],
                    "additionalProperties": false
                }),
            ));
        }
        tools
    }

    fn facts(&self, tool_id: &str, arguments: &Value) -> Result<ToolFacts, ProviderError> {
        let (raw_path, write) = match tool_id {
            READ_FILE | LIST_DIRECTORY => (parse::<PathArgs>(arguments)?.path, None),
            WRITE_FILE => {
                let args = parse::<WriteArgs>(arguments)?;
                (args.path.clone(), Some(args))
            }
            _ => return Err(ProviderError::UnknownTool),
        };
        let resolved = self
            .roots
            .resolve(&raw_path)
            .map_err(ProviderError::Rejected)?;
        let root = &self.roots.entries()[resolved.index];
        let path_text = resolved.canonical_path.display().to_string();
        let directory = if tool_id == LIST_DIRECTORY {
            resolved.canonical_path.clone()
        } else {
            resolved
                .canonical_path
                .parent()
                .unwrap_or(&root.canonical)
                .to_path_buf()
        };
        let summary = match (&write, tool_id) {
            (Some(args), _) => {
                if root.access != RootAccess::ReadWrite {
                    return Err(ProviderError::Rejected(READ_ONLY));
                }
                if args.content.len() > self.max_write_bytes {
                    return Err(ProviderError::Rejected(WRITE_TOO_LARGE));
                }
                let mode = if args.overwrite {
                    "replace the file if it exists"
                } else {
                    "new file only"
                };
                format!("Write {} bytes to {path_text} ({mode})", args.content.len())
            }
            (None, LIST_DIRECTORY) => format!("List {path_text}"),
            (None, _) => format!("Read {path_text}"),
        };
        let mut context = std::collections::BTreeMap::new();
        context.insert("path".to_string(), path_text.clone());
        context.insert("root".to_string(), root.canonical.display().to_string());
        context.insert("access".to_string(), root.access.as_str().to_string());
        Ok(ToolFacts {
            summary,
            target_host: None,
            target_path: Some(path_text),
            target_dir: Some(directory.display().to_string()),
            context,
        })
    }

    async fn call(&self, tool_id: &str, arguments: Value, _context: CallContext) -> ToolOutput {
        let work = match self.prepare(tool_id, &arguments) {
            Ok(work) => work,
            Err(message) => return ToolOutput::error(format!("Error: {message}")),
        };
        match tokio::task::spawn_blocking(move || work.run()).await {
            Ok(output) => output,
            Err(_) => ToolOutput::error("Error: the operation failed"),
        }
    }
}

enum Work {
    Read {
        dir: Arc<Dir>,
        relative: PathBuf,
        max: usize,
    },
    List {
        dir: Arc<Dir>,
        relative: PathBuf,
    },
    Write {
        dir: Arc<Dir>,
        relative: PathBuf,
        content: String,
        overwrite: bool,
        shown: String,
    },
}

impl FilesystemProvider {
    fn prepare(&self, tool_id: &str, arguments: &Value) -> Result<Work, &'static str> {
        match tool_id {
            READ_FILE | LIST_DIRECTORY => {
                let args: PathArgs =
                    serde_json::from_value(arguments.clone()).map_err(|_| "invalid arguments")?;
                let resolved = self.roots.resolve(&args.path)?;
                let (dir, relative) = (self.dirs[resolved.index].clone(), resolved.relative);
                Ok(if tool_id == READ_FILE {
                    Work::Read {
                        dir,
                        relative,
                        max: self.max_read_bytes,
                    }
                } else {
                    Work::List { dir, relative }
                })
            }
            WRITE_FILE => {
                let args: WriteArgs =
                    serde_json::from_value(arguments.clone()).map_err(|_| "invalid arguments")?;
                let resolved = self.roots.resolve(&args.path)?;
                if self.roots.entries()[resolved.index].access != RootAccess::ReadWrite {
                    return Err(READ_ONLY);
                }
                if args.content.len() > self.max_write_bytes {
                    return Err(WRITE_TOO_LARGE);
                }
                Ok(Work::Write {
                    dir: self.dirs[resolved.index].clone(),
                    shown: resolved.canonical_path.display().to_string(),
                    relative: resolved.relative,
                    content: args.content,
                    overwrite: args.overwrite,
                })
            }
            _ => Err("unknown tool"),
        }
    }
}

impl Work {
    fn run(self) -> ToolOutput {
        match self {
            Work::Read { dir, relative, max } => read_file(&dir, &relative, max),
            Work::List { dir, relative } => list_directory(&dir, &relative),
            Work::Write {
                dir,
                relative,
                content,
                overwrite,
                shown,
            } => write_file(&dir, &relative, &content, overwrite, &shown),
        }
    }
}

fn read_file(dir: &Dir, relative: &Path, max: usize) -> ToolOutput {
    let relative = relative_or_dot(relative);
    let metadata = match dir.metadata(relative) {
        Ok(metadata) => metadata,
        Err(error) => return ToolOutput::error(io_error(&error)),
    };
    if !metadata.is_file() {
        return ToolOutput::error("Error: not a regular file");
    }
    if metadata.len() > max as u64 {
        return ToolOutput::error("Error: the file is larger than the read limit");
    }
    let mut file = match dir.open(relative) {
        Ok(file) => file,
        Err(error) => return ToolOutput::error(io_error(&error)),
    };
    let mut bytes = Vec::new();
    if let Err(error) = (&mut file).take(max as u64 + 1).read_to_end(&mut bytes) {
        return ToolOutput::error(io_error(&error));
    }
    if bytes.len() > max {
        return ToolOutput::error("Error: the file is larger than the read limit");
    }
    match String::from_utf8(bytes) {
        Ok(text) => ToolOutput::ok(text),
        Err(_) => ToolOutput::error("Error: the file is not valid UTF-8 text"),
    }
}

fn list_directory(dir: &Dir, relative: &Path) -> ToolOutput {
    let relative = relative_or_dot(relative);
    let metadata = match dir.metadata(relative) {
        Ok(metadata) => metadata,
        Err(error) => return ToolOutput::error(io_error(&error)),
    };
    if !metadata.is_dir() {
        return ToolOutput::error("Error: not a directory");
    }
    let entries = match dir.read_dir(relative) {
        Ok(entries) => entries,
        Err(error) => return ToolOutput::error(io_error(&error)),
    };
    let mut lines: Vec<String> = Vec::new();
    for entry in entries.flatten() {
        let name = entry.file_name().to_string_lossy().into_owned();
        let line = match entry.metadata() {
            Ok(meta) if meta.is_dir() => format!("{name}/"),
            Ok(meta) if meta.is_file() => format!("{name} ({} bytes)", meta.len()),
            Ok(meta) if meta.is_symlink() => format!("{name}@"),
            _ => name,
        };
        lines.push(line);
    }
    lines.sort();
    if lines.is_empty() {
        return ToolOutput::ok("(empty directory)");
    }
    let total = lines.len();
    lines.truncate(MAX_LISTED_ENTRIES);
    let mut text = lines.join("\n");
    if total > MAX_LISTED_ENTRIES {
        text.push_str(&format!(
            "\n[{} more entries not shown]",
            total - MAX_LISTED_ENTRIES
        ));
    }
    ToolOutput::ok(text)
}

fn write_file(
    dir: &Dir,
    relative: &Path,
    content: &str,
    overwrite: bool,
    shown: &str,
) -> ToolOutput {
    if relative.as_os_str().is_empty() {
        return ToolOutput::error("Error: not a file path");
    }
    let parent = relative
        .parent()
        .filter(|parent| !parent.as_os_str().is_empty())
        .unwrap_or(Path::new("."));
    match dir.metadata(parent) {
        Ok(meta) if meta.is_dir() => {}
        Ok(_) => return ToolOutput::error("Error: the parent is not a directory"),
        Err(error) => return ToolOutput::error(io_error(&error)),
    }
    let mut options = OpenOptions::new();
    options.write(true);
    if overwrite {
        options.create(true).truncate(true);
    } else {
        options.create_new(true);
    }
    let mut file = match dir.open_with(relative, &options) {
        Ok(file) => file,
        Err(error) => return ToolOutput::error(io_error(&error)),
    };
    match file.write_all(content.as_bytes()) {
        Ok(()) => ToolOutput::ok(format!("Wrote {} bytes to {shown}", content.len())),
        Err(error) => ToolOutput::error(io_error(&error)),
    }
}
