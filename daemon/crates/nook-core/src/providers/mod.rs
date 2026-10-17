use crate::protocol::ToolDescriptor;
use async_trait::async_trait;
use serde_json::Value;
use std::collections::{BTreeMap, BTreeSet};
use std::sync::Arc;
use thiserror::Error;
use uuid::Uuid;

pub const MAX_TOOL_NAME_LENGTH: usize = 64;
pub const MAX_DESCRIPTION_LENGTH: usize = 2048;

/// Names the model already sees from the Worker or from Deep Agents.
pub const RESERVED_TOOL_NAMES: &[&str] = &[
    "web_search",
    "web_fetch",
    "news_search",
    "image_search",
    "video_search",
    "book_search",
    "ls",
    "read_file",
    "write_file",
    "edit_file",
    "glob",
    "grep",
    "write_todos",
    "task",
    "execute",
];

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct ToolFacts {
    pub summary: String,
    pub target_host: Option<String>,
    pub target_path: Option<String>,
    pub target_dir: Option<String>,
    pub context: BTreeMap<String, String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ToolOutput {
    pub text: String,
    pub is_error: bool,
}

impl ToolOutput {
    pub fn ok(text: impl Into<String>) -> Self {
        Self {
            text: text.into(),
            is_error: false,
        }
    }

    pub fn error(text: impl Into<String>) -> Self {
        Self {
            text: text.into(),
            is_error: true,
        }
    }
}

#[derive(Debug, Clone)]
pub struct CallContext {
    pub chat_id: Uuid,
    pub call_id: String,
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum ProviderError {
    #[error("{0}")]
    InvalidArguments(String),
    /// A fixed message the model can act on, such as a path outside the roots.
    #[error("{0}")]
    Rejected(&'static str),
    #[error("Unknown tool")]
    UnknownTool,
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum RegistryError {
    #[error("Tool id {0} is registered twice")]
    DuplicateId(String),
    #[error("Tool name {0} is used by two tools")]
    DuplicateName(String),
    #[error("Tool name {0} is reserved")]
    ReservedName(String),
    #[error("Tool name {0} is not valid")]
    InvalidName(String),
    #[error("Tool {0} has an empty or too long description")]
    InvalidDescription(String),
    #[error("Tool {0} needs an object input schema")]
    InvalidSchema(String),
}

#[async_trait]
pub trait Provider: Send + Sync {
    fn descriptors(&self) -> Vec<ToolDescriptor>;

    /// Turns validated arguments into the facts Cedar and the user see.
    fn facts(&self, tool_id: &str, arguments: &Value) -> Result<ToolFacts, ProviderError>;

    /// Runs one authorized call. The Daemon aborts this future to cancel it.
    async fn call(&self, tool_id: &str, arguments: Value, context: CallContext) -> ToolOutput;
}

pub mod filesystem;
pub mod roots;
pub mod terminal;

pub fn is_valid_tool_name(name: &str) -> bool {
    !name.is_empty()
        && name.len() <= MAX_TOOL_NAME_LENGTH
        && name
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-')
}

/// Builds a model-safe name such as `mcp__files__read` from raw parts.
pub fn sanitize_tool_name(parts: &[&str]) -> String {
    let joined = parts
        .iter()
        .map(|part| {
            part.chars()
                .map(|c| {
                    if c.is_ascii_alphanumeric() || c == '-' {
                        c
                    } else {
                        '_'
                    }
                })
                .collect::<String>()
        })
        .collect::<Vec<_>>()
        .join("__");
    joined.chars().take(MAX_TOOL_NAME_LENGTH).collect()
}

fn validate_descriptor(descriptor: &ToolDescriptor) -> Result<(), RegistryError> {
    if !is_valid_tool_name(&descriptor.name) {
        return Err(RegistryError::InvalidName(descriptor.name.clone()));
    }
    if RESERVED_TOOL_NAMES.contains(&descriptor.name.as_str()) {
        return Err(RegistryError::ReservedName(descriptor.name.clone()));
    }
    let description = descriptor.description.trim();
    if description.is_empty() || description.len() > MAX_DESCRIPTION_LENGTH {
        return Err(RegistryError::InvalidDescription(descriptor.id.clone()));
    }
    let is_object_schema = descriptor
        .input_schema
        .get("type")
        .and_then(Value::as_str)
        .is_some_and(|kind| kind == "object");
    if !is_object_schema || jsonschema::validator_for(&descriptor.input_schema).is_err() {
        return Err(RegistryError::InvalidSchema(descriptor.id.clone()));
    }
    Ok(())
}

#[derive(Default)]
pub struct ProviderRegistry {
    providers: Vec<Arc<dyn Provider>>,
    index: BTreeMap<String, usize>,
    descriptors: Vec<ToolDescriptor>,
}

impl ProviderRegistry {
    pub fn empty() -> Self {
        Self::default()
    }

    pub fn new(providers: Vec<Arc<dyn Provider>>) -> Result<Self, RegistryError> {
        let mut registry = Self::default();
        let mut names: BTreeSet<String> = BTreeSet::new();
        for (position, provider) in providers.iter().enumerate() {
            for descriptor in provider.descriptors() {
                validate_descriptor(&descriptor)?;
                if registry.index.contains_key(&descriptor.id) {
                    return Err(RegistryError::DuplicateId(descriptor.id));
                }
                if !names.insert(descriptor.name.clone()) {
                    return Err(RegistryError::DuplicateName(descriptor.name));
                }
                registry.index.insert(descriptor.id.clone(), position);
                registry.descriptors.push(descriptor);
            }
        }
        registry.providers = providers;
        Ok(registry)
    }

    pub fn is_empty(&self) -> bool {
        self.descriptors.is_empty()
    }

    pub fn descriptors(&self) -> &[ToolDescriptor] {
        &self.descriptors
    }

    pub fn descriptor(&self, tool_id: &str) -> Option<&ToolDescriptor> {
        self.descriptors
            .iter()
            .find(|descriptor| descriptor.id == tool_id)
    }

    pub fn contains(&self, tool_id: &str) -> bool {
        self.index.contains_key(tool_id)
    }

    fn provider(&self, tool_id: &str) -> Result<&Arc<dyn Provider>, ProviderError> {
        self.index
            .get(tool_id)
            .map(|position| &self.providers[*position])
            .ok_or(ProviderError::UnknownTool)
    }

    pub fn facts(&self, tool_id: &str, arguments: &Value) -> Result<ToolFacts, ProviderError> {
        let descriptor = self.descriptor(tool_id).ok_or(ProviderError::UnknownTool)?;
        let validator = jsonschema::validator_for(&descriptor.input_schema)
            .map_err(|_| ProviderError::InvalidArguments("Tool schema is invalid".to_string()))?;
        if let Some(error) = validator.iter_errors(arguments).next() {
            return Err(ProviderError::InvalidArguments(format!(
                "Invalid arguments: {error}"
            )));
        }
        self.provider(tool_id)?.facts(tool_id, arguments)
    }

    pub async fn call(&self, tool_id: &str, arguments: Value, context: CallContext) -> ToolOutput {
        match self.provider(tool_id) {
            Ok(provider) => provider.call(tool_id, arguments, context).await,
            Err(_) => ToolOutput::error("Unknown tool"),
        }
    }
}

pub fn truncate_output(text: String, max_bytes: usize) -> String {
    if text.len() <= max_bytes {
        return text;
    }
    let mut end = max_bytes;
    while !text.is_char_boundary(end) {
        end -= 1;
    }
    format!("{}\n[output truncated]", &text[..end])
}

/// Builds the registry from the config. A provider that cannot start is left
/// out and the reason is logged.
pub async fn build_registry(tools: &crate::config::ToolsConfig) -> ProviderRegistry {
    let mut providers: Vec<Arc<dyn Provider>> = Vec::new();
    if cfg!(unix) {
        if let Some(filesystem) = filesystem::FilesystemProvider::from_config(&tools.filesystem) {
            providers.push(Arc::new(filesystem));
        }
    } else if !tools.filesystem.roots.is_empty() {
        tracing::warn!("File tools are off: this platform has no sandbox");
    }
    if tools.terminal.enabled {
        match crate::sandbox::Sandbox::detect().await {
            Ok(sandbox) => {
                tracing::info!(sandbox = sandbox.name(), "The command sandbox works");
                if let Some(terminal) = terminal::TerminalProvider::new(tools, sandbox) {
                    providers.push(Arc::new(terminal));
                }
            }
            Err(error) => tracing::warn!("The terminal tool is off: {error}"),
        }
    }
    match ProviderRegistry::new(providers) {
        Ok(registry) => {
            for descriptor in registry.descriptors() {
                tracing::info!(tool = %descriptor.id, "Registered tool");
            }
            registry
        }
        Err(error) => {
            tracing::error!("Tool providers are off: {error}");
            ProviderRegistry::empty()
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::{FilesystemConfig, RootAccess, RootConfig, ToolsConfig};
    use serde_json::json;

    struct Fixed(Vec<ToolDescriptor>);

    #[async_trait]
    impl Provider for Fixed {
        fn descriptors(&self) -> Vec<ToolDescriptor> {
            self.0.clone()
        }

        fn facts(&self, _id: &str, _arguments: &Value) -> Result<ToolFacts, ProviderError> {
            Ok(ToolFacts::default())
        }

        async fn call(&self, _id: &str, _arguments: Value, _context: CallContext) -> ToolOutput {
            ToolOutput::ok("done")
        }
    }

    fn descriptor(id: &str, name: &str) -> ToolDescriptor {
        ToolDescriptor {
            id: id.to_string(),
            name: name.to_string(),
            description: "A tool.".to_string(),
            input_schema: json!({ "type": "object", "properties": { "a": { "type": "string" } } }),
        }
    }

    fn registry(tools: Vec<ToolDescriptor>) -> Result<ProviderRegistry, RegistryError> {
        ProviderRegistry::new(vec![Arc::new(Fixed(tools))])
    }

    #[test]
    fn a_valid_tool_is_registered() {
        let registry = registry(vec![descriptor("nook:a", "a_tool")]).unwrap();

        assert!(registry.contains("nook:a"));
        assert_eq!(registry.descriptors().len(), 1);
    }

    #[test]
    fn duplicate_ids_and_names_are_rejected() {
        assert_eq!(
            registry(vec![
                descriptor("nook:a", "one"),
                descriptor("nook:a", "two")
            ])
            .err(),
            Some(RegistryError::DuplicateId("nook:a".to_string()))
        );
        assert_eq!(
            registry(vec![
                descriptor("nook:a", "same"),
                descriptor("nook:b", "same")
            ])
            .err(),
            Some(RegistryError::DuplicateName("same".to_string()))
        );
    }

    #[test]
    fn reserved_and_invalid_names_are_rejected() {
        for reserved in ["web_fetch", "read_file", "execute", "task"] {
            assert_eq!(
                registry(vec![descriptor("nook:a", reserved)]).err(),
                Some(RegistryError::ReservedName(reserved.to_string()))
            );
        }
        for invalid in ["has space", "dot.name", "", &"x".repeat(65)] {
            assert_eq!(
                registry(vec![descriptor("nook:a", invalid)]).err(),
                Some(RegistryError::InvalidName(invalid.to_string()))
            );
        }
    }

    #[test]
    fn an_empty_description_or_a_bad_schema_is_rejected() {
        let mut empty = descriptor("nook:a", "a_tool");
        empty.description = "  ".to_string();
        assert_eq!(
            registry(vec![empty]).err(),
            Some(RegistryError::InvalidDescription("nook:a".to_string()))
        );
        for schema in [
            json!({ "type": "string" }),
            json!({ "properties": {} }),
            json!({ "type": "object", "properties": { "a": { "type": "nonsense" } } }),
        ] {
            let mut bad = descriptor("nook:a", "a_tool");
            bad.input_schema = schema;
            assert_eq!(
                registry(vec![bad]).err(),
                Some(RegistryError::InvalidSchema("nook:a".to_string()))
            );
        }
    }

    #[test]
    fn arguments_are_checked_against_the_schema_before_the_provider_sees_them() {
        let mut strict = descriptor("nook:a", "a_tool");
        strict.input_schema = json!({
            "type": "object",
            "properties": { "a": { "type": "string" } },
            "required": ["a"],
            "additionalProperties": false
        });
        let registry = registry(vec![strict]).unwrap();

        assert!(registry.facts("nook:a", &json!({ "a": "x" })).is_ok());
        for bad in [json!({}), json!({ "a": 1 }), json!({ "a": "x", "b": 1 })] {
            assert!(matches!(
                registry.facts("nook:a", &bad),
                Err(ProviderError::InvalidArguments(_))
            ));
        }
        assert_eq!(
            registry.facts("nook:missing", &json!({})),
            Err(ProviderError::UnknownTool)
        );
    }

    #[test]
    fn names_are_made_safe_for_the_model() {
        assert_eq!(
            sanitize_tool_name(&["mcp", "my server", "read.file"]),
            "mcp__my_server__read_file"
        );
        assert_eq!(sanitize_tool_name(&[&"x".repeat(100)]).len(), 64);
        assert!(is_valid_tool_name(&sanitize_tool_name(&["a b", "c/d"])));
    }

    #[test]
    fn output_is_cut_on_a_character_boundary() {
        let cut = truncate_output("héllo wörld".to_string(), 2);

        assert!(cut.starts_with('h'));
        assert!(cut.ends_with("[output truncated]"));
        assert_eq!(truncate_output("short".to_string(), 100), "short");
    }

    #[tokio::test]
    async fn the_builder_offers_nothing_without_roots() {
        let registry = build_registry(&ToolsConfig::default()).await;

        assert!(registry.is_empty());
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn the_builder_offers_file_tools_for_a_root() {
        let dir = tempfile::tempdir().unwrap();
        let tools = ToolsConfig {
            filesystem: FilesystemConfig {
                roots: vec![RootConfig {
                    path: dir.path().display().to_string(),
                    access: RootAccess::Read,
                }],
                ..FilesystemConfig::default()
            },
            ..ToolsConfig::default()
        };

        let registry = build_registry(&tools).await;

        assert!(registry.contains("nook:read_local_file"));
        assert!(!registry.contains("nook:write_local_file"));
    }
}
