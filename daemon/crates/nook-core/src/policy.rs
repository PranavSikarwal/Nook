use cedar_policy::{
    Authorizer, Context, Decision, Entities, Entity, EntityUid, PolicySet, Request, Schema,
    ValidationMode, Validator,
};
use std::collections::{HashMap, HashSet};
use std::path::Path;
use std::str::FromStr;
use std::sync::Arc;
use thiserror::Error;
use tokio::sync::Mutex;
use uuid::Uuid;

#[derive(Debug, Error)]
pub enum PolicyError {
    #[error("Failed to read schema file: {0}")]
    SchemaIo(String),
    #[error("Invalid Cedar schema: {0}")]
    SchemaParse(String),
    #[error("Failed to read policy file: {0}")]
    PolicyIo(String),
    #[error("Invalid Cedar policy: {0}")]
    PolicyParse(String),
    #[error("Cedar policy validation error: {0}")]
    Validation(String),
    #[error("Cedar request evaluation error: {0}")]
    Evaluation(String),
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct ScopedGrant {
    pub chat_id: Uuid,
    pub tool_name: String,
    pub scope: String,
    pub host: Option<String>,
    pub argument_digest: Option<String>,
}

impl ScopedGrant {
    pub fn matches(
        &self,
        chat_id: Uuid,
        tool_name: &str,
        host: Option<&str>,
        argument_digest: Option<&str>,
    ) -> bool {
        if self.tool_name != tool_name {
            return false;
        }
        match self.scope.as_str() {
            "persistent" => true,
            "chat" => self.chat_id == chat_id,
            "chat_host" => {
                self.chat_id == chat_id && host.is_some() && self.host.as_deref() == host
            }
            "once" => {
                self.chat_id == chat_id
                    && argument_digest.is_some()
                    && self.argument_digest.as_deref() == argument_digest
            }
            _ => false,
        }
    }
}

#[derive(Debug, Default)]
pub struct GrantStore {
    grants: HashSet<ScopedGrant>,
}

impl GrantStore {
    pub fn new() -> Self {
        Self {
            grants: HashSet::new(),
        }
    }

    pub fn add_grant(&mut self, grant: ScopedGrant) {
        self.grants.insert(grant);
    }

    pub fn find_scope(
        &self,
        chat_id: Uuid,
        tool_name: &str,
        host: Option<&str>,
        argument_digest: Option<&str>,
    ) -> Option<String> {
        self.grants
            .iter()
            .find(|g| g.matches(chat_id, tool_name, host, argument_digest))
            .map(|g| g.scope.clone())
    }
}

fn make_str_expr(val: &str) -> Result<cedar_policy::RestrictedExpression, PolicyError> {
    let escaped = val.replace('\\', "\\\\").replace('"', "\\\"");
    cedar_policy::RestrictedExpression::from_str(&format!("\"{escaped}\""))
        .map_err(|e| PolicyError::Evaluation(e.to_string()))
}

pub struct CedarAuthorizer {
    authorizer: Authorizer,
    policies: PolicySet,
    entities: Entities,
    grant_store: Arc<Mutex<GrantStore>>,
}

impl CedarAuthorizer {
    pub fn from_files(schema_path: &Path, policy_path: &Path) -> Result<Self, PolicyError> {
        let schema_src = std::fs::read_to_string(schema_path)
            .map_err(|e| PolicyError::SchemaIo(e.to_string()))?;
        let policy_src = std::fs::read_to_string(policy_path)
            .map_err(|e| PolicyError::PolicyIo(e.to_string()))?;

        Self::from_str(&schema_src, &policy_src)
    }

    pub fn from_str(schema_src: &str, policy_src: &str) -> Result<Self, PolicyError> {
        let (schema, warnings) = Schema::from_cedarschema_str(schema_src)
            .map_err(|e| PolicyError::SchemaParse(e.to_string()))?;
        let _ = warnings;

        let policies =
            PolicySet::from_str(policy_src).map_err(|e| PolicyError::PolicyParse(e.to_string()))?;

        // Validate policies against schema
        let validator = Validator::new(schema);
        let val_res = validator.validate(&policies, ValidationMode::Strict);
        if !val_res.validation_passed() {
            let errs: Vec<String> = val_res.validation_errors().map(|e| e.to_string()).collect();
            if !errs.is_empty() {
                return Err(PolicyError::Validation(errs.join("; ")));
            }
        }

        let authorizer = Authorizer::new();

        // Create base entities (User and Tool)
        let mut entity_map = HashMap::new();
        let user_uid = EntityUid::from_str("Nook::User::\"local_user\"")
            .map_err(|e| PolicyError::Evaluation(e.to_string()))?;
        let user_entity = Entity::new_no_attrs(user_uid.clone(), HashSet::new());
        entity_map.insert(user_uid, user_entity);

        let tool_uid = EntityUid::from_str("Nook::Tool::\"local_tool\"")
            .map_err(|e| PolicyError::Evaluation(e.to_string()))?;
        let tool_entity = Entity::new_no_attrs(tool_uid.clone(), HashSet::new());
        entity_map.insert(tool_uid, tool_entity);

        let entities = Entities::from_entities(entity_map.into_values(), None)
            .map_err(|e| PolicyError::Evaluation(e.to_string()))?;

        Ok(Self {
            authorizer,
            policies,
            entities,
            grant_store: Arc::new(Mutex::new(GrantStore::new())),
        })
    }

    pub fn grant_store(&self) -> Arc<Mutex<GrantStore>> {
        self.grant_store.clone()
    }

    pub async fn authorize_tool(
        &self,
        chat_id: Uuid,
        action_name: &str, // "web_search" or "web_fetch"
        approval_tier: &str,
        host: Option<&str>,
        url: Option<&str>,
        argument_digest: Option<&str>,
    ) -> Result<bool, PolicyError> {
        let grant_scope = {
            let store = self.grant_store.lock().await;
            store
                .find_scope(chat_id, action_name, host, argument_digest)
                .unwrap_or_else(|| "none".to_string())
        };

        let principal = EntityUid::from_str("Nook::User::\"local_user\"")
            .map_err(|e| PolicyError::Evaluation(e.to_string()))?;
        let action_str = format!("Nook::Action::\"{action_name}\"");
        let action =
            EntityUid::from_str(&action_str).map_err(|e| PolicyError::Evaluation(e.to_string()))?;
        let resource = EntityUid::from_str("Nook::Tool::\"local_tool\"")
            .map_err(|e| PolicyError::Evaluation(e.to_string()))?;

        let mut context_map = HashMap::new();
        context_map.insert("chat_id".to_string(), make_str_expr(&chat_id.to_string())?);
        context_map.insert("approval_tier".to_string(), make_str_expr(approval_tier)?);
        context_map.insert("grant_scope".to_string(), make_str_expr(&grant_scope)?);

        if action_name == "web_fetch" {
            context_map.insert("host".to_string(), make_str_expr(host.unwrap_or(""))?);
            context_map.insert("url".to_string(), make_str_expr(url.unwrap_or(""))?);
            context_map.insert(
                "argument_digest".to_string(),
                make_str_expr(argument_digest.unwrap_or(""))?,
            );
        }

        let context =
            Context::from_pairs(context_map).map_err(|e| PolicyError::Evaluation(e.to_string()))?;

        let request = Request::new(principal, action, resource, context, None)
            .map_err(|e| PolicyError::Evaluation(e.to_string()))?;

        let response = self
            .authorizer
            .is_authorized(&request, &self.policies, &self.entities);

        Ok(response.decision() == Decision::Allow)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const SCHEMA_SRC: &str = include_str!("../../../../policy/cedar/schema.cedarschema");
    const POLICIES_SRC: &str = include_str!("../../../../policy/cedar/policies.cedar");

    fn create_test_authorizer() -> CedarAuthorizer {
        CedarAuthorizer::from_str(SCHEMA_SRC, POLICIES_SRC).expect("valid test policy files")
    }

    async fn check_fetch(authorizer: &CedarAuthorizer, chat_id: Uuid, host: &str) -> bool {
        authorizer
            .authorize_tool(
                chat_id,
                "web_fetch",
                "ask_once_per_host",
                Some(host),
                Some(&format!("https://{host}")),
                Some("digest_xyz"),
            )
            .await
            .expect("evaluation succeeds")
    }

    #[tokio::test]
    async fn test_web_search_allowed_by_default() {
        let auth = create_test_authorizer();
        assert!(auth
            .authorize_tool(Uuid::new_v4(), "web_search", "allow", None, None, None)
            .await
            .unwrap());
    }

    #[tokio::test]
    async fn test_web_fetch_authorization_workflow() {
        let auth = create_test_authorizer();
        let chat_id = Uuid::new_v4();

        // 1. Initial attempt without grant: denied
        assert!(!check_fetch(&auth, chat_id, "docs.example.com").await);

        // 2. Add host grant
        auth.grant_store().lock().await.add_grant(ScopedGrant {
            chat_id,
            tool_name: "web_fetch".to_string(),
            scope: "chat_host".to_string(),
            host: Some("docs.example.com".to_string()),
            argument_digest: None,
        });

        // 3. Approved host: allowed
        assert!(check_fetch(&auth, chat_id, "docs.example.com").await);

        // 4. Unapproved host: still denied
        assert!(!check_fetch(&auth, chat_id, "unapproved.example.com").await);
    }
}
