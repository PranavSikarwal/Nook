use crate::policy::{CedarAuthorizer, PolicyError};
use serde::Deserialize;
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use thiserror::Error;

const TOOL_NAMESPACE: &str = "nook:";
const KNOWN_TIERS: &[&str] = &["allow", "ask_once_per_host", "deny"];
const KNOWN_ACTIONS: &[&str] = &[
    "allow_once",
    "allow_for_chat",
    "allow_for_chat_host",
    "always_allow",
    "deny",
];

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
    pub actions: Vec<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct ToolSpec {
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

    fn validate(&self, authorizer: &CedarAuthorizer) -> Result<(), PolicyAssetsError> {
        for (name, spec) in &self.tools {
            let cedar_action = name.strip_prefix(TOOL_NAMESPACE).ok_or_else(|| {
                PolicyAssetsError::ToolsInvalid(format!(
                    "tool {name} must start with {TOOL_NAMESPACE}"
                ))
            })?;
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
            for action in &spec.approval.actions {
                if !KNOWN_ACTIONS.contains(&action.as_str()) {
                    return Err(PolicyAssetsError::ToolsInvalid(format!(
                        "tool {name} has unknown approval action {action}"
                    )));
                }
            }
        }
        Ok(())
    }
}

pub struct PolicyAssets {
    pub catalog: ToolCatalog,
    pub authorizer: CedarAuthorizer,
}

impl PolicyAssets {
    pub fn load(dir: &Path) -> Result<Self, PolicyAssetsError> {
        let tools_path = dir.join("tools.yaml");
        let tools_source =
            std::fs::read_to_string(&tools_path).map_err(|e| PolicyAssetsError::Io {
                path: tools_path.display().to_string(),
                reason: e.to_string(),
            })?;
        let catalog = ToolCatalog::parse(&tools_source)?;
        let authorizer = CedarAuthorizer::from_files(
            &dir.join("cedar/schema.cedarschema"),
            &dir.join("cedar/policies.cedar"),
        )?;
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

    #[test]
    fn rejects_unknown_approval_action() {
        let tools = TOOLS.replace("- deny", "- grant_forever");
        let dir = write_assets(&tools, SCHEMA, POLICIES);
        assert!(matches!(
            PolicyAssets::load(dir.path()),
            Err(PolicyAssetsError::ToolsInvalid(_))
        ));
    }

    #[test]
    fn rejects_tool_without_cedar_action() {
        let tools = format!(
            "{TOOLS}\n  nook:other:\n    description: x\n    approval:\n      tier: allow\n"
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
