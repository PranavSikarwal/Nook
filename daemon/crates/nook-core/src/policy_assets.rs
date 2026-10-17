use crate::policy::{CedarAuthorizer, PolicyError};
use crate::protocol::ApprovalAction;
use serde::Deserialize;
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use thiserror::Error;

pub(crate) const TOOL_NAMESPACE: &str = "nook:";
pub(crate) const MCP_PREFIX: &str = "mcp:";
pub(crate) const MCP_ENTRY: &str = "mcp:*";
pub(crate) const MCP_ACTION: &str = "mcp_call";
const KNOWN_TIERS: &[&str] = &["allow", "ask", "ask_once_per_host", "deny"];
pub const APPROVAL_ACTIONS: &[(&str, Option<&str>)] = &[
    ("allow_once", Some("once")),
    ("allow_for_chat", Some("chat")),
    ("allow_for_chat_host", Some("chat_host")),
    ("allow_for_chat_path", Some("chat_path")),
    ("allow_for_chat_dir", Some("chat_dir")),
    ("always_allow", Some("persistent")),
    ("deny", None),
];

pub fn approval_action_scope(id: &str) -> Option<Option<&'static str>> {
    APPROVAL_ACTIONS
        .iter()
        .find(|(known, _)| *known == id)
        .map(|(_, scope)| *scope)
}

#[derive(Debug, Error)]
pub enum PolicyAssetsError {
    #[error("Policy directory not found. Set NOOK_POLICY_DIR or install the policy resources")]
    DirectoryNotFound,
    #[error("Failed to read {path}: {reason}")]
    Io { path: String, reason: String },
    #[error("Invalid tools.yaml: {0}")]
    ToolsParse(String),
    #[error("Invalid tool catalog: {0}")]
    ToolsInvalid(String),
    #[error(transparent)]
    Cedar(#[from] PolicyError),
}

#[derive(Debug, Clone, Deserialize)]
pub struct ApprovalSpec {
    pub tier: String,
    #[serde(default)]
    pub actions: Vec<ApprovalAction>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct ToolSpec {
    pub display_name: String,
    pub description: String,
    pub approval: ApprovalSpec,
}

#[derive(Debug, Clone, Deserialize)]
pub struct ToolCatalog {
    pub version: u32,
    pub tools: BTreeMap<String, ToolSpec>,
}

impl ToolCatalog {
    pub fn parse(source: &str) -> Result<Self, PolicyAssetsError> {
        let catalog: ToolCatalog = serde_norway::from_str(source)
            .map_err(|e| PolicyAssetsError::ToolsParse(e.to_string()))?;
        if catalog.version != 1 {
            return Err(PolicyAssetsError::ToolsInvalid(format!(
                "unsupported version {}",
                catalog.version
            )));
        }
        Ok(catalog)
    }

    pub fn tool(&self, name: &str) -> Option<&ToolSpec> {
        self.tools.get(name)
    }

    /// Finds the entry for a tool. All MCP tools share the `mcp:*` entry.
    pub fn resolve(&self, name: &str) -> Option<&ToolSpec> {
        if name.starts_with(MCP_PREFIX) {
            return self.tools.get(MCP_ENTRY);
        }
        self.tools.get(name)
    }

    fn validate(&self, authorizer: &CedarAuthorizer) -> Result<(), PolicyAssetsError> {
        for (name, spec) in &self.tools {
            let cedar_action = if name == MCP_ENTRY {
                MCP_ACTION
            } else {
                name.strip_prefix(TOOL_NAMESPACE).ok_or_else(|| {
                    PolicyAssetsError::ToolsInvalid(format!(
                        "tool {name} must start with {TOOL_NAMESPACE}"
                    ))
                })?
            };
            if !authorizer.has_action(cedar_action) {
                return Err(PolicyAssetsError::ToolsInvalid(format!(
                    "tool {name} has no Cedar action {cedar_action}"
                )));
            }
            if !KNOWN_TIERS.contains(&spec.approval.tier.as_str()) {
                return Err(PolicyAssetsError::ToolsInvalid(format!(
                    "tool {name} has unknown tier {}",
                    spec.approval.tier
                )));
            }
            if spec.display_name.trim().is_empty() {
                return Err(PolicyAssetsError::ToolsInvalid(format!(
                    "tool {name} has an empty display_name"
                )));
            }
            validate_actions(name, &spec.approval)?;
        }
        Ok(())
    }
}

fn validate_actions(tool: &str, approval: &ApprovalSpec) -> Result<(), PolicyAssetsError> {
    let invalid = |detail: String| PolicyAssetsError::ToolsInvalid(format!("tool {tool} {detail}"));
    let actions = &approval.actions;
    if actions.is_empty() && !matches!(approval.tier.as_str(), "allow" | "deny") {
        return Err(invalid(format!(
            "has tier {} but lists no approval actions",
            approval.tier
        )));
    }
    let mut seen: Vec<&str> = Vec::new();
    for action in actions {
        if approval_action_scope(&action.id).is_none() {
            return Err(invalid(format!(
                "has unknown approval action {}",
                action.id
            )));
        }
        if seen.contains(&action.id.as_str()) {
            return Err(invalid(format!(
                "has duplicate approval action {}",
                action.id
            )));
        }
        seen.push(&action.id);
        if action.label.trim().is_empty() {
            return Err(invalid(format!(
                "has an empty label for approval action {}",
                action.id
            )));
        }
        if action.description.trim().is_empty() {
            return Err(invalid(format!(
                "has an empty description for approval action {}",
                action.id
            )));
        }
    }
    Ok(())
}

pub struct PolicyAssets {
    pub catalog: ToolCatalog,
    pub authorizer: CedarAuthorizer,
}

impl PolicyAssets {
    pub fn load(dir: &Path) -> Result<Self, PolicyAssetsError> {
        let read = |relative: &str| {
            let path = dir.join(relative);
            std::fs::read_to_string(&path).map_err(|e| PolicyAssetsError::Io {
                path: path.display().to_string(),
                reason: e.to_string(),
            })
        };
        Self::from_sources(
            &read("tools.yaml")?,
            &read("cedar/schema.cedarschema")?,
            &read("cedar/policies.cedar")?,
        )
    }

    pub fn from_sources(
        tools_yaml: &str,
        cedar_schema: &str,
        cedar_policies: &str,
    ) -> Result<Self, PolicyAssetsError> {
        let catalog = ToolCatalog::parse(tools_yaml)?;
        let authorizer = CedarAuthorizer::from_str(cedar_schema, cedar_policies)?;
        catalog.validate(&authorizer)?;
        Ok(Self {
            catalog,
            authorizer,
        })
    }

    pub fn load_default() -> Result<Self, PolicyAssetsError> {
        let dir = find_policy_directory().ok_or(PolicyAssetsError::DirectoryNotFound)?;
        Self::load(&dir)
    }
}

fn is_policy_directory(path: &Path) -> bool {
    path.join("tools.yaml").is_file() && path.join("cedar").is_dir()
}

pub fn find_policy_directory() -> Option<PathBuf> {
    if let Ok(dir) = std::env::var("NOOK_POLICY_DIR") {
        let path = PathBuf::from(dir);
        return is_policy_directory(&path).then_some(path);
    }

    let mut candidates: Vec<PathBuf> = Vec::new();
    if let Ok(current_exe) = std::env::current_exe() {
        if let Some(exe_dir) = current_exe.parent() {
            candidates.extend([
                exe_dir.join("../policy"),
                exe_dir.join("policy"),
                exe_dir.join("resources/policy"),
                exe_dir.join("../Resources/policy"),
            ]);
        }
    }
    candidates.extend(["policy", "../policy", "../../policy"].map(PathBuf::from));
    candidates
        .into_iter()
        .find(|path| is_policy_directory(path))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::TempDir;

    const SCHEMA: &str = include_str!("../../../../policy/cedar/schema.cedarschema");
    const POLICIES: &str = include_str!("../../../../policy/cedar/policies.cedar");
    const TOOLS: &str = include_str!("../../../../policy/tools.yaml");

    fn write_assets(tools: &str, schema: &str, policies: &str) -> TempDir {
        let dir = TempDir::new().unwrap();
        fs::create_dir(dir.path().join("cedar")).unwrap();
        fs::write(dir.path().join("tools.yaml"), tools).unwrap();
        fs::write(dir.path().join("cedar/schema.cedarschema"), schema).unwrap();
        fs::write(dir.path().join("cedar/policies.cedar"), policies).unwrap();
        dir
    }

    #[test]
    fn loads_repository_assets() {
        let dir = write_assets(TOOLS, SCHEMA, POLICIES);
        let assets = PolicyAssets::load(dir.path()).unwrap();
        assert!(assets.catalog.tool("nook:web_fetch").is_some());
        assert_eq!(
            assets
                .catalog
                .tool("nook:web_search")
                .unwrap()
                .approval
                .tier,
            "allow"
        );
    }

    #[test]
    fn rejects_invalid_yaml() {
        let dir = write_assets("tools: [", SCHEMA, POLICIES);
        assert!(matches!(
            PolicyAssets::load(dir.path()),
            Err(PolicyAssetsError::ToolsParse(_))
        ));
    }

    #[test]
    fn rejects_invalid_cedar_schema() {
        let dir = write_assets(TOOLS, "namespace Nook {", POLICIES);
        assert!(matches!(
            PolicyAssets::load(dir.path()),
            Err(PolicyAssetsError::Cedar(PolicyError::SchemaParse(_)))
        ));
    }

    #[test]
    fn rejects_invalid_cedar_policy() {
        let dir = write_assets(TOOLS, SCHEMA, "permit (");
        assert!(matches!(
            PolicyAssets::load(dir.path()),
            Err(PolicyAssetsError::Cedar(PolicyError::PolicyParse(_)))
        ));
    }

    fn expect_invalid(tools: &str) -> String {
        let dir = write_assets(tools, SCHEMA, POLICIES);
        match PolicyAssets::load(dir.path()) {
            Err(PolicyAssetsError::ToolsInvalid(message)) => message,
            Err(other) => panic!("Expected ToolsInvalid, got {other:?}"),
            Ok(_) => panic!("Expected the catalog to be rejected"),
        }
    }

    #[test]
    fn loads_display_names_and_action_objects() {
        let dir = write_assets(TOOLS, SCHEMA, POLICIES);
        let assets = PolicyAssets::load(dir.path()).unwrap();
        let fetch = assets.catalog.tool("nook:web_fetch").unwrap();
        assert_eq!(fetch.display_name, "Web fetch");
        let ids: Vec<&str> = fetch
            .approval
            .actions
            .iter()
            .map(|action| action.id.as_str())
            .collect();
        assert_eq!(ids, ["allow_once", "allow_for_chat_host", "deny"]);
        assert_eq!(
            fetch.approval.actions[1].label,
            "Allow for this chat & host"
        );
        assert!(!fetch.approval.actions[1].description.is_empty());
    }

    #[test]
    fn rejects_unknown_approval_action() {
        let tools = TOOLS.replace("id: deny", "id: grant_forever");
        assert!(expect_invalid(&tools).contains("grant_forever"));
    }

    #[test]
    fn rejects_empty_action_label() {
        let tools = TOOLS.replace("label: \"Deny\"", "label: \"  \"");
        assert!(expect_invalid(&tools).contains("label"));
    }

    #[test]
    fn rejects_empty_action_description() {
        let tools = TOOLS.replace(
            "description: \"Reject this request. The tool does not run.\"",
            "description: \"\"",
        );
        assert!(expect_invalid(&tools).contains("description"));
    }

    #[test]
    fn rejects_duplicate_action_id() {
        let tools = TOOLS.replace("id: deny", "id: allow_once");
        assert!(expect_invalid(&tools).contains("duplicate"));
    }

    #[test]
    fn rejects_action_without_label() {
        let tools = TOOLS.replace("          label: \"Deny\"\n", "");
        let dir = write_assets(&tools, SCHEMA, POLICIES);
        assert!(matches!(
            PolicyAssets::load(dir.path()),
            Err(PolicyAssetsError::ToolsParse(_))
        ));
    }

    #[test]
    fn rejects_legacy_string_actions() {
        let tools = "version: 1\ntools:\n  nook:web_search:\n    display_name: Web search\n    description: x\n    approval:\n      tier: allow\n      actions:\n        - allow_once\n";
        let dir = write_assets(tools, SCHEMA, POLICIES);
        assert!(matches!(
            PolicyAssets::load(dir.path()),
            Err(PolicyAssetsError::ToolsParse(_))
        ));
    }

    const PANEL_SCHEMA: &str = include_str!("../../../../contracts/panel-daemon.schema.json");

    fn schema_enum(definition: &str, property: &str) -> Vec<String> {
        let schema: serde_json::Value = serde_json::from_str(PANEL_SCHEMA).unwrap();
        schema["definitions"][definition]["properties"][property]["enum"]
            .as_array()
            .unwrap_or_else(|| panic!("{definition}.{property} has no enum"))
            .iter()
            .map(|value| value.as_str().unwrap().to_string())
            .collect()
    }

    fn table_ids() -> Vec<String> {
        let mut ids: Vec<String> = APPROVAL_ACTIONS
            .iter()
            .map(|(id, _)| id.to_string())
            .collect();
        ids.sort();
        ids
    }

    #[test]
    fn approval_actions_match_the_panel_schema_enums() {
        for (definition, property) in [
            ("ApprovalAction", "id"),
            ("ApprovalDecisionRequest", "action"),
        ] {
            let mut ids = schema_enum(definition, property);
            ids.sort();
            assert_eq!(ids, table_ids(), "{definition}.{property}");
        }
    }

    #[test]
    fn approval_actions_map_ids_to_grant_scopes() {
        let scope = |id: &str| APPROVAL_ACTIONS.iter().find(|(known, _)| *known == id);
        assert_eq!(scope("allow_once"), Some(&("allow_once", Some("once"))));
        assert_eq!(
            scope("allow_for_chat_host"),
            Some(&("allow_for_chat_host", Some("chat_host")))
        );
        assert_eq!(
            scope("allow_for_chat"),
            Some(&("allow_for_chat", Some("chat")))
        );
        assert_eq!(
            scope("always_allow"),
            Some(&("always_allow", Some("persistent")))
        );
        assert_eq!(scope("deny"), Some(&("deny", None)));
    }

    #[test]
    fn rejects_asking_tier_without_actions() {
        let tools = "version: 1\ntools:\n  nook:web_search:\n    display_name: Web search\n    description: x\n    approval:\n      tier: ask_once_per_host\n";
        assert!(expect_invalid(tools).contains("actions"));
    }

    #[test]
    fn rejects_asking_tier_with_empty_actions_list() {
        let tools = "version: 1\ntools:\n  nook:web_search:\n    display_name: Web search\n    description: x\n    approval:\n      tier: ask_once_per_host\n      actions: []\n";
        assert!(expect_invalid(tools).contains("actions"));
    }

    #[test]
    fn loads_allow_and_deny_tiers_without_actions() {
        for tier in ["allow", "deny"] {
            let tools = format!("version: 1\ntools:\n  nook:web_search:\n    display_name: Web search\n    description: x\n    approval:\n      tier: {tier}\n");
            let dir = write_assets(&tools, SCHEMA, POLICIES);
            PolicyAssets::load(dir.path()).unwrap();
        }
    }

    #[test]
    fn repository_web_search_lists_no_actions() {
        let dir = write_assets(TOOLS, SCHEMA, POLICIES);
        let assets = PolicyAssets::load(dir.path()).unwrap();
        assert!(assets
            .catalog
            .tool("nook:web_search")
            .unwrap()
            .approval
            .actions
            .is_empty());
    }

    #[test]
    fn rejects_empty_display_name() {
        let tools = TOOLS.replace("display_name: \"Web fetch\"", "display_name: \"\"");
        assert!(expect_invalid(&tools).contains("display_name"));
    }

    #[test]
    fn rejects_missing_display_name() {
        let tools = TOOLS.replace("    display_name: \"Web fetch\"\n", "");
        let dir = write_assets(&tools, SCHEMA, POLICIES);
        assert!(matches!(
            PolicyAssets::load(dir.path()),
            Err(PolicyAssetsError::ToolsParse(_))
        ));
    }

    #[test]
    fn rejects_tool_without_cedar_action() {
        let tools = format!(
            "{TOOLS}\n  nook:other:\n    display_name: Other\n    description: x\n    approval:\n      tier: allow\n"
        );
        let dir = write_assets(&tools, SCHEMA, POLICIES);
        assert!(matches!(
            PolicyAssets::load(dir.path()),
            Err(PolicyAssetsError::ToolsInvalid(_))
        ));
    }

    #[test]
    fn rejects_unknown_tier() {
        let tools = TOOLS.replace("tier: allow\n", "tier: sometimes\n");
        let dir = write_assets(&tools, SCHEMA, POLICIES);
        assert!(matches!(
            PolicyAssets::load(dir.path()),
            Err(PolicyAssetsError::ToolsInvalid(_))
        ));
    }

    #[test]
    fn rejects_missing_directory() {
        assert!(matches!(
            PolicyAssets::load(Path::new("/nonexistent/nook-policy")),
            Err(PolicyAssetsError::Io { .. })
        ));
    }

    #[test]
    fn detects_policy_directory_layout() {
        let dir = write_assets(TOOLS, SCHEMA, POLICIES);
        assert!(is_policy_directory(dir.path()));
        assert!(!is_policy_directory(&dir.path().join("cedar")));
    }
}
