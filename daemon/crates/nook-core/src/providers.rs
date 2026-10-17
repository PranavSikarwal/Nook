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
