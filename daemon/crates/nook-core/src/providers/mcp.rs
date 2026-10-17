use super::{
    is_valid_tool_name, sanitize_tool_name, CallContext, Provider, ProviderError, ToolFacts,
    ToolOutput, MAX_DESCRIPTION_LENGTH, RESERVED_TOOL_NAMES,
};
use crate::config::McpServerConfig;
use crate::protocol::ToolDescriptor;
use crate::sandbox::{Sandbox, SandboxProcess, SandboxSpec};
use async_trait::async_trait;
use serde_json::{json, Value};
use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::Arc;
use std::time::Duration;
use thiserror::Error;
use tokio::io::{AsyncBufReadExt, AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt, BufReader};
use tokio::sync::{oneshot, Mutex};
use tokio::task::JoinHandle;
use tracing::{debug, info, warn};

pub const MAX_MESSAGE_BYTES: usize = 4 * 1024 * 1024;
pub const MAX_TOOLS_PER_SERVER: usize = 64;
const MAX_LIST_PAGES: usize = 10;
const SUPPORTED_PROTOCOLS: &[&str] = &["2024-11-05", "2025-03-26", "2025-06-18"];
const CLIENT_PROTOCOL: &str = "2025-03-26";
const SUMMARY_ARGUMENT_CHARS: usize = 300;

#[derive(Debug, Error, Clone, PartialEq, Eq)]
pub enum McpError {
    #[error("The MCP server stopped")]
    Closed,
    #[error("The MCP server did not answer in time")]
    Timeout,
    #[error("The MCP server reported an error: {message}")]
    Rpc { code: i64, message: String },
    #[error("The MCP server broke the protocol: {0}")]
    Protocol(String),
    #[error("Could not talk to the MCP server: {0}")]
    Io(String),
}

#[derive(Debug, Clone, PartialEq)]
pub struct McpTool {
    pub name: String,
    pub description: Option<String>,
    pub input_schema: Value,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct McpCallResult {
    pub text: String,
    pub is_error: bool,
}

type Pending = Arc<Mutex<HashMap<u64, oneshot::Sender<Result<Value, McpError>>>>>;
type SharedWriter = Arc<Mutex<Box<dyn AsyncWrite + Send + Unpin>>>;

async fn write_message(writer: &SharedWriter, message: &Value) -> Result<(), McpError> {
    let mut line = serde_json::to_string(message).map_err(|e| McpError::Io(e.to_string()))?;
    line.push('\n');
    let mut writer = writer.lock().await;
    writer
        .write_all(line.as_bytes())
        .await
        .map_err(|e| McpError::Io(e.to_string()))?;
    writer
        .flush()
        .await
        .map_err(|e| McpError::Io(e.to_string()))
}

/// A JSON-RPC client over newline-delimited messages.
pub struct McpClient {
    writer: SharedWriter,
    pending: Pending,
    next_id: AtomicU64,
    closed: Arc<AtomicBool>,
    reader: JoinHandle<()>,
}

impl McpClient {
    pub fn new(
        reader: impl AsyncRead + Send + Unpin + 'static,
        writer: impl AsyncWrite + Send + Unpin + 'static,
    ) -> Self {
        let writer: SharedWriter = Arc::new(Mutex::new(Box::new(writer)));
        let pending: Pending = Arc::new(Mutex::new(HashMap::new()));
        let closed = Arc::new(AtomicBool::new(false));
        let task = tokio::spawn(read_loop(
            reader,
            writer.clone(),
            pending.clone(),
            closed.clone(),
        ));
        Self {
            writer,
            pending,
            next_id: AtomicU64::new(1),
            closed,
            reader: task,
        }
    }

    pub fn is_closed(&self) -> bool {
        self.closed.load(Ordering::SeqCst)
    }

    pub async fn request(
        &self,
        method: &str,
        params: Value,
        timeout: Duration,
    ) -> Result<Value, McpError> {
        if self.is_closed() {
            return Err(McpError::Closed);
        }
        let id = self.next_id.fetch_add(1, Ordering::SeqCst);
        let (tx, rx) = oneshot::channel();
        self.pending.lock().await.insert(id, tx);
        let message = json!({ "jsonrpc": "2.0", "id": id, "method": method, "params": params });
        if let Err(error) = write_message(&self.writer, &message).await {
            self.pending.lock().await.remove(&id);
            return Err(error);
        }
        match tokio::time::timeout(timeout, rx).await {
            Ok(Ok(result)) => result,
            Ok(Err(_)) => Err(McpError::Closed),
            Err(_) => {
                self.pending.lock().await.remove(&id);
                let cancel = json!({
                    "jsonrpc": "2.0",
                    "method": "notifications/cancelled",
                    "params": { "requestId": id, "reason": "timeout" }
                });
                let _ = write_message(&self.writer, &cancel).await;
                Err(McpError::Timeout)
            }
        }
    }

    pub async fn notify(&self, method: &str, params: Value) -> Result<(), McpError> {
        let message = json!({ "jsonrpc": "2.0", "method": method, "params": params });
        write_message(&self.writer, &message).await
    }

    pub async fn initialize(&self, timeout: Duration) -> Result<(), McpError> {
        let result = self
            .request(
                "initialize",
                json!({
                    "protocolVersion": CLIENT_PROTOCOL,
                    "capabilities": {},
                    "clientInfo": { "name": "nook", "version": env!("CARGO_PKG_VERSION") }
                }),
                timeout,
            )
            .await?;
        let version = result
            .get("protocolVersion")
            .and_then(Value::as_str)
            .ok_or_else(|| McpError::Protocol("no protocol version".to_string()))?;
        if !SUPPORTED_PROTOCOLS.contains(&version) {
            return Err(McpError::Protocol(format!(
                "unsupported protocol version {version}"
            )));
        }
        self.notify("notifications/initialized", json!({})).await
    }

    pub async fn list_tools(&self, timeout: Duration) -> Result<Vec<McpTool>, McpError> {
        let mut tools = Vec::new();
        let mut cursor: Option<String> = None;
        for _ in 0..MAX_LIST_PAGES {
            let params = match &cursor {
                Some(cursor) => json!({ "cursor": cursor }),
                None => json!({}),
            };
            let result = self.request("tools/list", params, timeout).await?;
            let page = result
                .get("tools")
                .and_then(Value::as_array)
                .ok_or_else(|| McpError::Protocol("tools/list had no tools".to_string()))?;
            for entry in page {
                let Some(name) = entry.get("name").and_then(Value::as_str) else {
                    continue;
                };
                tools.push(McpTool {
                    name: name.to_string(),
                    description: entry
                        .get("description")
                        .and_then(Value::as_str)
                        .map(str::to_string),
                    input_schema: entry.get("inputSchema").cloned().unwrap_or(Value::Null),
                });
            }
            cursor = result
                .get("nextCursor")
                .and_then(Value::as_str)
                .map(str::to_string);
            if cursor.is_none() {
                break;
            }
        }
        Ok(tools)
    }

    pub async fn call_tool(
        &self,
        name: &str,
        arguments: Value,
        timeout: Duration,
    ) -> Result<McpCallResult, McpError> {
        let result = self
            .request(
                "tools/call",
                json!({ "name": name, "arguments": arguments }),
                timeout,
            )
            .await?;
        Ok(convert_result(&result))
    }
}

impl Drop for McpClient {
    fn drop(&mut self) {
        self.reader.abort();
    }
}

fn convert_result(result: &Value) -> McpCallResult {
    let mut parts: Vec<String> = Vec::new();
    if let Some(content) = result.get("content").and_then(Value::as_array) {
        for item in content {
            let kind = item
                .get("type")
                .and_then(Value::as_str)
                .unwrap_or("unknown");
            let part = match kind {
                "text" => item
                    .get("text")
                    .and_then(Value::as_str)
                    .unwrap_or_default()
                    .to_string(),
                "resource" => item
                    .pointer("/resource/text")
                    .and_then(Value::as_str)
                    .map(str::to_string)
                    .unwrap_or_else(|| "[resource omitted]".to_string()),
                "resource_link" => format!(
                    "[resource link: {}]",
                    item.get("uri").and_then(Value::as_str).unwrap_or("unknown")
                ),
                other => format!("[{other} omitted]"),
            };
            parts.push(part);
        }
    }
    if parts.is_empty() {
        if let Some(structured) = result.get("structuredContent") {
            parts.push(structured.to_string());
        }
    }
    McpCallResult {
        text: parts.join("\n"),
        is_error: result
            .get("isError")
            .and_then(Value::as_bool)
            .unwrap_or(false),
    }
}

async fn read_loop(
    reader: impl AsyncRead + Send + Unpin + 'static,
    writer: SharedWriter,
    pending: Pending,
    closed: Arc<AtomicBool>,
) {
    let mut reader = BufReader::new(reader);
    let mut buffer: Vec<u8> = Vec::new();
    loop {
        buffer.clear();
        let read = (&mut reader)
            .take(MAX_MESSAGE_BYTES as u64 + 1)
            .read_until(b'\n', &mut buffer)
            .await;
        match read {
            Ok(0) | Err(_) => break,
            Ok(_) if buffer.len() > MAX_MESSAGE_BYTES => {
                warn!("An MCP server sent a message above the size limit");
                break;
            }
            Ok(_) => {}
        }
        let Ok(message) = serde_json::from_slice::<Value>(&buffer) else {
            debug!("An MCP server sent a line that is not JSON");
            continue;
        };
        let id = message.get("id").cloned();
        let has_method = message.get("method").is_some();
        match (id, has_method) {
            (Some(id), false) => {
                let Some(id) = id.as_u64() else { continue };
                let Some(sender) = pending.lock().await.remove(&id) else {
                    continue;
                };
                let outcome = if let Some(error) = message.get("error") {
                    Err(McpError::Rpc {
                        code: error.get("code").and_then(Value::as_i64).unwrap_or(0),
                        message: error
                            .get("message")
                            .and_then(Value::as_str)
                            .unwrap_or("unknown error")
                            .chars()
                            .take(200)
                            .collect(),
                    })
                } else {
                    Ok(message.get("result").cloned().unwrap_or(Value::Null))
                };
                let _ = sender.send(outcome);
            }
            (Some(id), true) => {
                let reply = json!({
                    "jsonrpc": "2.0",
                    "id": id,
                    "error": { "code": -32601, "message": "Method not found" }
                });
                let _ = write_message(&writer, &reply).await;
            }
            (None, _) => {}
        }
    }
    closed.store(true, Ordering::SeqCst);
    let mut pending = pending.lock().await;
    for (_, sender) in pending.drain() {
        let _ = sender.send(Err(McpError::Closed));
    }
}

#[derive(Debug, Clone, Copy)]
pub struct StartupLimits {
    pub initialize: Duration,
    pub list_tools: Duration,
}

impl Default for StartupLimits {
    fn default() -> Self {
        Self {
            initialize: Duration::from_secs(10),
            list_tools: Duration::from_secs(10),
        }
    }
}

struct McpServer {
    name: String,
    client: McpClient,
    call_timeout: Duration,
    _process: SandboxProcess,
    _home: tempfile::TempDir,
    _stderr: JoinHandle<()>,
}

struct ToolEntry {
    server: usize,
    remote_name: String,
    descriptor: ToolDescriptor,
}

pub struct McpProvider {
    servers: Vec<McpServer>,
    tools: BTreeMap<String, ToolEntry>,
}

fn valid_server_name(name: &str) -> bool {
    !name.is_empty()
        && name.len() <= 32
        && name
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-')
}

fn existing_paths(paths: &[String], server: &str) -> Vec<PathBuf> {
    paths
        .iter()
        .filter_map(|raw| match PathBuf::from(raw).canonicalize() {
            Ok(path) if PathBuf::from(raw).is_absolute() => Some(path),
            _ => {
                warn!("MCP server {server}: ignored path {raw}");
                None
            }
        })
        .collect()
}

fn sandbox_path() -> String {
    let mut path = String::from("/usr/local/bin:/usr/bin:/bin:/usr/sbin:/sbin");
    if cfg!(target_os = "macos") && std::path::Path::new("/opt/homebrew/bin").is_dir() {
        path = format!("/opt/homebrew/bin:{path}");
    }
    path
}

async fn start_server(
    config: &McpServerConfig,
    sandbox: &Sandbox,
    limits: StartupLimits,
) -> Result<(McpServer, Vec<McpTool>), String> {
    let home = tempfile::tempdir().map_err(|e| e.to_string())?;
    let home_path = home.path().canonicalize().map_err(|e| e.to_string())?;
    let home_text = home_path.display().to_string();
    let mut env = BTreeMap::from([
        ("PATH".to_string(), sandbox_path()),
        ("HOME".to_string(), home_text.clone()),
        ("TMPDIR".to_string(), home_text),
        ("LANG".to_string(), "C.UTF-8".to_string()),
    ]);
    env.extend(config.env.clone());
    let mut write_roots = vec![home_path.clone()];
    write_roots.extend(existing_paths(&config.write_paths, &config.name));
    let spec = SandboxSpec {
        argv: std::iter::once(config.command.clone())
            .chain(config.args.iter().cloned())
            .collect(),
        cwd: home_path,
        read_roots: existing_paths(&config.read_paths, &config.name),
        write_roots,
        network: config.network,
        env,
        timeout: Duration::from_secs(0),
        max_output_bytes: 0,
    };
    let mut process = sandbox.spawn(&spec).map_err(|e| e.to_string())?;
    let stdin = process.child.stdin.take().ok_or("no standard input")?;
    let stdout = process.child.stdout.take().ok_or("no standard output")?;
    let stderr = process.child.stderr.take().ok_or("no standard error")?;
    let name = config.name.clone();
    let stderr_task = tokio::spawn(async move {
        let mut lines = BufReader::new(stderr).lines();
        while let Ok(Some(line)) = lines.next_line().await {
            debug!(server = %name, "MCP stderr: {line}");
        }
    });
    let client = McpClient::new(stdout, stdin);
    client
        .initialize(limits.initialize)
        .await
        .map_err(|e| e.to_string())?;
    let tools = client
        .list_tools(limits.list_tools)
        .await
        .map_err(|e| e.to_string())?;
    Ok((
        McpServer {
            name: config.name.clone(),
            client,
            call_timeout: Duration::from_secs(config.timeout_seconds.clamp(1, 600)),
            _process: process,
            _home: home,
            _stderr: stderr_task,
        },
        tools,
    ))
}

fn compact_arguments(arguments: &Value) -> String {
    let text = arguments.to_string();
    if text.chars().count() <= SUMMARY_ARGUMENT_CHARS {
        return text;
    }
    let cut: String = text.chars().take(SUMMARY_ARGUMENT_CHARS).collect();
    format!("{cut}...")
}

impl McpProvider {
    /// Starts every configured server and keeps the ones that answer. Returns
    /// `None` when no server offers a usable tool.
    pub async fn start(
        configs: &[McpServerConfig],
        sandbox: &Sandbox,
        limits: StartupLimits,
    ) -> Option<Self> {
        let mut accepted: Vec<&McpServerConfig> = Vec::new();
        let mut seen: BTreeSet<&str> = BTreeSet::new();
        for config in configs {
            if !valid_server_name(&config.name) {
                warn!(
                    "Ignored MCP server {:?}: the name is not valid",
                    config.name
                );
            } else if !seen.insert(config.name.as_str()) {
                warn!("Ignored MCP server {}: the name is used twice", config.name);
            } else if config.command.trim().is_empty() {
                warn!("Ignored MCP server {}: the command is empty", config.name);
            } else {
                accepted.push(config);
            }
        }
        let mut starting = tokio::task::JoinSet::new();
        for (position, config) in accepted.iter().enumerate() {
            let config = (*config).clone();
            let sandbox = sandbox.clone();
            starting
                .spawn(async move { (position, start_server(&config, &sandbox, limits).await) });
        }
        let mut started: Vec<(usize, McpServer, Vec<McpTool>)> = Vec::new();
        while let Some(joined) = starting.join_next().await {
            let Ok((position, outcome)) = joined else {
                continue;
            };
            match outcome {
                Ok((server, tools)) => started.push((position, server, tools)),
                Err(error) => warn!("MCP server {} is off: {error}", accepted[position].name),
            }
        }
        started.sort_by_key(|(position, _, _)| *position);

        let mut servers = Vec::new();
        let mut tools = BTreeMap::new();
        let mut names: BTreeSet<String> = BTreeSet::new();
        for (_, server, listed) in started {
            let index = servers.len();
            let mut count = 0;
            for tool in listed {
                if count >= MAX_TOOLS_PER_SERVER {
                    warn!(
                        "MCP server {}: more than {MAX_TOOLS_PER_SERVER} tools, ignoring the rest",
                        server.name
                    );
                    break;
                }
                let Some(entry) = build_entry(&server.name, index, &tool, &mut names) else {
                    continue;
                };
                tools.insert(entry.descriptor.id.clone(), entry);
                count += 1;
            }
            info!(server = %server.name, tools = count, "MCP server is ready");
            servers.push(server);
        }
        if tools.is_empty() {
            return None;
        }
        Some(Self { servers, tools })
    }
}

fn build_entry(
    server: &str,
    index: usize,
    tool: &McpTool,
    names: &mut BTreeSet<String>,
) -> Option<ToolEntry> {
    let skip = |reason: &str| {
        warn!(
            "MCP server {server}: ignored tool {:?}: {reason}",
            tool.name
        );
        None
    };
    if tool.name.is_empty() || tool.name.len() > 128 || tool.name.contains(':') {
        return skip("the name is not valid");
    }
    let is_object_schema = tool
        .input_schema
        .get("type")
        .and_then(Value::as_str)
        .is_some_and(|kind| kind == "object");
    if !is_object_schema || jsonschema::validator_for(&tool.input_schema).is_err() {
        return skip("the input schema is not a valid object schema");
    }
    let model_name = sanitize_tool_name(&["mcp", server, &tool.name]);
    if !is_valid_tool_name(&model_name) || RESERVED_TOOL_NAMES.contains(&model_name.as_str()) {
        return skip("the model name is not valid");
    }
    if !names.insert(model_name.clone()) {
        return skip("its model name collides with another tool");
    }
    let description = tool
        .description
        .as_deref()
        .map(str::trim)
        .filter(|text| !text.is_empty())
        .unwrap_or(&tool.name);
    let description: String = format!("{description} (MCP server: {server})")
        .chars()
        .take(MAX_DESCRIPTION_LENGTH)
        .collect();
    Some(ToolEntry {
        server: index,
        remote_name: tool.name.clone(),
        descriptor: ToolDescriptor {
            id: format!("mcp:{server}:{}", tool.name),
            name: model_name,
            description,
            input_schema: tool.input_schema.clone(),
        },
    })
}

#[async_trait]
impl Provider for McpProvider {
    fn descriptors(&self) -> Vec<ToolDescriptor> {
        self.tools
            .values()
            .map(|entry| entry.descriptor.clone())
            .collect()
    }

    fn facts(&self, tool_id: &str, arguments: &Value) -> Result<ToolFacts, ProviderError> {
        let entry = self.tools.get(tool_id).ok_or(ProviderError::UnknownTool)?;
        let server = &self.servers[entry.server].name;
        let mut context = BTreeMap::new();
        context.insert("server".to_string(), server.clone());
        context.insert("tool".to_string(), entry.remote_name.clone());
        Ok(ToolFacts {
            summary: format!(
                "Call {} on MCP server {server} with {}",
                entry.remote_name,
                compact_arguments(arguments)
            ),
            context,
            ..ToolFacts::default()
        })
    }

    async fn call(&self, tool_id: &str, arguments: Value, _context: CallContext) -> ToolOutput {
        let Some(entry) = self.tools.get(tool_id) else {
            return ToolOutput::error("Error: unknown tool");
        };
        let server = &self.servers[entry.server];
        match server
            .client
            .call_tool(&entry.remote_name, arguments, server.call_timeout)
            .await
        {
            Ok(result) if result.is_error => ToolOutput::error(format!("Error: {}", result.text)),
            Ok(result) => ToolOutput::ok(result.text),
            Err(McpError::Timeout) => {
                ToolOutput::error("Error: the MCP server did not answer in time")
            }
            Err(McpError::Closed) => ToolOutput::error("Error: the MCP server stopped"),
            Err(McpError::Rpc { message, .. }) => ToolOutput::error(format!(
                "Error: the MCP server reported an error: {message}"
            )),
            Err(_) => ToolOutput::error("Error: the MCP call failed"),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tokio::io::{duplex, AsyncBufReadExt, AsyncWriteExt, BufReader, DuplexStream};

    const SHORT: Duration = Duration::from_millis(300);
    const LONG: Duration = Duration::from_secs(5);

    struct Peer {
        lines: tokio::io::Lines<BufReader<tokio::io::ReadHalf<DuplexStream>>>,
        writer: tokio::io::WriteHalf<DuplexStream>,
    }

    impl Peer {
        async fn next(&mut self) -> Value {
            let line = tokio::time::timeout(LONG, self.lines.next_line())
                .await
                .unwrap()
                .unwrap()
                .expect("the client closed the pipe");
            serde_json::from_str(&line).unwrap()
        }

        async fn send(&mut self, message: Value) {
            self.send_raw(&message.to_string()).await;
        }

        async fn send_raw(&mut self, text: &str) {
            self.writer.write_all(text.as_bytes()).await.unwrap();
            self.writer.write_all(b"\n").await.unwrap();
            self.writer.flush().await.unwrap();
        }
    }

    fn pair() -> (McpClient, Peer) {
        let (client_side, server_side) = duplex(1 << 20);
        let (client_read, client_write) = tokio::io::split(client_side);
        let (server_read, server_write) = tokio::io::split(server_side);
        (
            McpClient::new(client_read, client_write),
            Peer {
                lines: BufReader::new(server_read).lines(),
                writer: server_write,
            },
        )
    }

    #[tokio::test]
    async fn answers_are_matched_by_id_even_out_of_order() {
        let (client, mut peer) = pair();
        let client = Arc::new(client);
        let first = tokio::spawn({
            let client = client.clone();
            async move { client.request("a", json!({}), LONG).await }
        });
        let first_request = peer.next().await;
        let second = tokio::spawn({
            let client = client.clone();
            async move { client.request("b", json!({}), LONG).await }
        });
        let second_request = peer.next().await;

        peer.send(json!({ "jsonrpc": "2.0", "id": second_request["id"], "result": "second" }))
            .await;
        peer.send(json!({ "jsonrpc": "2.0", "id": first_request["id"], "result": "first" }))
            .await;

        assert_eq!(first.await.unwrap().unwrap(), json!("first"));
        assert_eq!(second.await.unwrap().unwrap(), json!("second"));
        assert_eq!(first_request["method"], "a");
        assert_eq!(second_request["method"], "b");
    }

    #[tokio::test]
    async fn a_request_that_gets_no_answer_times_out_and_is_cancelled() {
        let (client, mut peer) = pair();

        let result = client.request("slow", json!({}), SHORT).await;

        assert_eq!(result, Err(McpError::Timeout));
        let request = peer.next().await;
        let cancelled = peer.next().await;
        assert_eq!(cancelled["method"], "notifications/cancelled");
        assert_eq!(cancelled["params"]["requestId"], request["id"]);
        assert!(client.pending.lock().await.is_empty());
    }

    #[tokio::test]
    async fn a_late_answer_after_a_timeout_is_ignored() {
        let (client, mut peer) = pair();
        let result = client.request("slow", json!({}), SHORT).await;
        let request = peer.next().await;
        peer.send(json!({ "jsonrpc": "2.0", "id": request["id"], "result": 1 }))
            .await;

        let next = client.request("next", json!({}), LONG);
        let answer = async {
            let _cancelled = peer.next().await;
            let next_request = peer.next().await;
            peer.send(json!({ "jsonrpc": "2.0", "id": next_request["id"], "result": 2 }))
                .await;
        };
        let (next_result, ()) = tokio::join!(next, answer);

        assert_eq!(result, Err(McpError::Timeout));
        assert_eq!(next_result.unwrap(), json!(2));
    }

    #[tokio::test]
    async fn closing_the_pipe_fails_the_waiting_requests_and_all_later_ones() {
        let (client, peer) = pair();
        let client = Arc::new(client);
        let waiting = tokio::spawn({
            let client = client.clone();
            async move { client.request("a", json!({}), LONG).await }
        });
        tokio::time::sleep(Duration::from_millis(50)).await;

        drop(peer);

        assert_eq!(waiting.await.unwrap(), Err(McpError::Closed));
        assert!(client.is_closed());
        assert_eq!(
            client.request("b", json!({}), LONG).await,
            Err(McpError::Closed)
        );
    }

    #[tokio::test]
    async fn a_request_from_the_server_gets_method_not_found() {
        let (client, mut peer) = pair();
        peer.send(json!({ "jsonrpc": "2.0", "id": 7, "method": "sampling/createMessage" }))
            .await;

        let reply = peer.next().await;

        assert_eq!(reply["id"], 7);
        assert_eq!(reply["error"]["code"], -32601);
        assert!(!client.is_closed());
    }

    #[tokio::test]
    async fn text_that_is_not_json_and_notifications_are_ignored() {
        let (client, mut peer) = pair();
        peer.send_raw("starting up...").await;
        peer.send(json!({ "jsonrpc": "2.0", "method": "notifications/message", "params": {} }))
            .await;
        let call = client.request("a", json!({}), LONG);
        let answer = async {
            let request = peer.next().await;
            peer.send(json!({ "jsonrpc": "2.0", "id": request["id"], "result": "ok" }))
                .await;
        };

        let (result, ()) = tokio::join!(call, answer);

        assert_eq!(result.unwrap(), json!("ok"));
    }

    #[tokio::test]
    async fn an_error_answer_keeps_the_code_and_a_short_message() {
        let (client, mut peer) = pair();
        let call = client.request("a", json!({}), LONG);
        let answer = async {
            let request = peer.next().await;
            peer.send(json!({
                "jsonrpc": "2.0",
                "id": request["id"],
                "error": { "code": -32000, "message": "m".repeat(500) }
            }))
            .await;
        };

        let (result, ()) = tokio::join!(call, answer);

        let Err(McpError::Rpc { code, message }) = result else {
            panic!("Expected an RPC error");
        };
        assert_eq!(code, -32000);
        assert_eq!(message.len(), 200);
    }

    #[tokio::test]
    async fn a_message_above_the_size_limit_closes_the_connection() {
        let (client, mut peer) = pair();
        let call = client.request("a", json!({}), LONG);
        let flood = async {
            let _request = peer.next().await;
            let big = "x".repeat(MAX_MESSAGE_BYTES + 10);
            let _ = peer.writer.write_all(big.as_bytes()).await;
            let _ = peer.writer.flush().await;
            peer
        };

        let (result, _peer) = tokio::join!(call, flood);

        assert_eq!(result, Err(McpError::Closed));
        assert!(client.is_closed());
    }

    async fn serve_initialize(peer: &mut Peer, version: &str) -> Value {
        let request = peer.next().await;
        assert_eq!(request["method"], "initialize");
        peer.send(json!({
            "jsonrpc": "2.0",
            "id": request["id"],
            "result": { "protocolVersion": version, "capabilities": {} }
        }))
        .await;
        request
    }

    #[tokio::test]
    async fn initialize_sends_the_client_info_and_then_the_initialized_notice() {
        let (client, mut peer) = pair();
        let init = client.initialize(LONG);
        let serve = async {
            let request = serve_initialize(&mut peer, "2025-06-18").await;
            let notice = peer.next().await;
            (request, notice)
        };

        let (result, (request, notice)) = tokio::join!(init, serve);

        result.unwrap();
        assert_eq!(request["params"]["clientInfo"]["name"], "nook");
        assert_eq!(request["params"]["capabilities"], json!({}));
        assert_eq!(notice["method"], "notifications/initialized");
        assert!(notice.get("id").is_none());
    }

    #[tokio::test]
    async fn initialize_rejects_a_protocol_version_it_does_not_know() {
        let (client, mut peer) = pair();
        let init = client.initialize(LONG);
        let serve = serve_initialize(&mut peer, "1999-01-01");

        let (result, _) = tokio::join!(init, serve);

        assert!(matches!(result, Err(McpError::Protocol(_))));
    }

    #[tokio::test]
    async fn the_tool_list_follows_pages_and_skips_entries_without_a_name() {
        let (client, mut peer) = pair();
        let list = client.list_tools(LONG);
        let serve = async {
            let first = peer.next().await;
            assert!(first["params"].get("cursor").is_none());
            peer.send(json!({
                "jsonrpc": "2.0",
                "id": first["id"],
                "result": {
                    "tools": [
                        { "name": "one", "inputSchema": { "type": "object" } },
                        { "description": "no name" }
                    ],
                    "nextCursor": "page2"
                }
            }))
            .await;
            let second = peer.next().await;
            assert_eq!(second["params"]["cursor"], "page2");
            peer.send(json!({
                "jsonrpc": "2.0",
                "id": second["id"],
                "result": { "tools": [{ "name": "two", "description": "Second." }] }
            }))
            .await;
        };

        let (tools, ()) = tokio::join!(list, serve);

        let tools = tools.unwrap();
        assert_eq!(
            tools.iter().map(|t| t.name.as_str()).collect::<Vec<_>>(),
            ["one", "two"]
        );
        assert_eq!(tools[1].description.as_deref(), Some("Second."));
        assert_eq!(tools[1].input_schema, Value::Null);
    }

    #[tokio::test]
    async fn a_server_that_never_stops_paging_is_cut_off() {
        let (client, mut peer) = pair();
        let list = client.list_tools(LONG);
        let serve = async {
            for _ in 0..MAX_LIST_PAGES {
                let request = peer.next().await;
                peer.send(json!({
                    "jsonrpc": "2.0",
                    "id": request["id"],
                    "result": { "tools": [], "nextCursor": "again" }
                }))
                .await;
            }
        };

        let (tools, ()) = tokio::join!(list, serve);

        assert!(tools.unwrap().is_empty());
    }

    #[test]
    fn results_turn_into_plain_text() {
        let mixed = convert_result(&json!({
            "content": [
                { "type": "text", "text": "a" },
                { "type": "image", "data": "x" },
                { "type": "resource", "resource": { "text": "b" } },
                { "type": "resource", "resource": { "blob": "x" } },
                { "type": "resource_link", "uri": "file:///c" },
                { "type": "audio" }
            ]
        }));
        assert_eq!(
            mixed.text,
            "a\n[image omitted]\nb\n[resource omitted]\n[resource link: file:///c]\n[audio omitted]"
        );
        assert!(!mixed.is_error);

        let structured = convert_result(&json!({ "structuredContent": { "n": 1 } }));
        assert_eq!(structured.text, "{\"n\":1}");

        let failed = convert_result(
            &json!({ "isError": true, "content": [{ "type": "text", "text": "no" }] }),
        );
        assert!(failed.is_error);
        assert_eq!(failed.text, "no");

        assert_eq!(convert_result(&json!({})).text, "");
    }

    #[test]
    fn server_names_are_checked() {
        for good in ["files", "my-server_2", &"a".repeat(32)] {
            assert!(valid_server_name(good), "{good}");
        }
        for bad in ["", "has space", "dot.name", "colon:name", &"a".repeat(33)] {
            assert!(!valid_server_name(bad), "{bad}");
        }
    }

    #[test]
    fn long_arguments_are_shortened_in_the_summary() {
        let long = compact_arguments(&json!({ "text": "x".repeat(1000) }));

        assert!(long.ends_with("..."));
        assert_eq!(long.chars().count(), SUMMARY_ARGUMENT_CHARS + 3);
        assert_eq!(compact_arguments(&json!({ "a": 1 })), "{\"a\":1}");
    }
}
