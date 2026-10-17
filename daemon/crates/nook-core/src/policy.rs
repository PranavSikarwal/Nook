use crate::authorization::AuthorizationRequest;
use crate::policy_assets::{MCP_ACTION, MCP_PREFIX, TOOL_NAMESPACE};
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
    pub path: Option<String>,
    pub argument_digest: Option<String>,
}

pub fn cedar_action_name(tool_name: &str) -> &str {
    if tool_name.starts_with(MCP_PREFIX) {
        return MCP_ACTION;
    }
    tool_name.strip_prefix(TOOL_NAMESPACE).unwrap_or(tool_name)
}

/// The name a grant is stored under. MCP grants name the exact tool.
pub fn is_persistent_scope(scope: &str) -> bool {
    scope.starts_with("persistent")
}

pub fn grant_key(tool_name: &str) -> &str {
    if tool_name.starts_with(MCP_PREFIX) {
        return tool_name;
    }
    cedar_action_name(tool_name)
}

fn path_is_inside(directory: &str, target: &str) -> bool {
    Path::new(target).starts_with(directory)
}

impl ScopedGrant {
    pub fn for_request(request: &AuthorizationRequest, scope: &str) -> Option<Self> {
        let (host, path, argument_digest) = match scope {
            "chat" | "persistent" => (None, None, None),
            "chat_host" | "persistent_host" => (Some(request.target_host.clone()?), None, None),
            "chat_path" => (None, Some(request.target_path.clone()?), None),
            "chat_dir" => (None, Some(request.target_dir.clone()?), None),
            "once" => (None, None, Some(request.argument_digest.clone())),
            _ => return None,
        };
        Some(Self {
            chat_id: if is_persistent_scope(scope) {
                Uuid::nil()
            } else {
                request.chat_id
            },
            tool_name: grant_key(&request.tool_name).to_string(),
            scope: scope.to_string(),
            host,
            path,
            argument_digest,
        })
    }

    /// A stable id for a grant, so the Panel can name it and the database can store it.
    pub fn id(&self) -> String {
        use sha2::{Digest, Sha256};
        let key = format!(
            "{}\0{}\0{}",
            self.tool_name,
            self.scope,
            self.host.as_deref().unwrap_or_default()
        );
        format!("{:x}", Sha256::digest(key.as_bytes()))[..32].to_string()
    }

    pub fn matches(
        &self,
        chat_id: Uuid,
        tool_name: &str,
        host: Option<&str>,
        target_path: Option<&str>,
        argument_digest: Option<&str>,
    ) -> bool {
        if self.tool_name != tool_name {
            return false;
        }
        match self.scope.as_str() {
            "persistent" => true,
            "persistent_host" => host.is_some() && self.host.as_deref() == host,
            "chat" => self.chat_id == chat_id,
            "chat_host" => {
                self.chat_id == chat_id && host.is_some() && self.host.as_deref() == host
            }
            "chat_path" => {
                self.chat_id == chat_id
                    && target_path.is_some()
                    && self.path.as_deref() == target_path
            }
            "chat_dir" => {
                self.chat_id == chat_id
                    && matches!((self.path.as_deref(), target_path),
                        (Some(directory), Some(target)) if path_is_inside(directory, target))
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

    pub fn add_grant(&mut self, grant: ScopedGrant) -> bool {
        self.grants.insert(grant)
    }

    pub fn remove(&mut self, grant: &ScopedGrant) -> bool {
        self.grants.remove(grant)
    }

    pub fn persistent_grants(&self) -> Vec<ScopedGrant> {
        let mut grants: Vec<ScopedGrant> = self
            .grants
            .iter()
            .filter(|grant| is_persistent_scope(&grant.scope))
            .cloned()
            .collect();
        grants.sort_by_key(ScopedGrant::id);
        grants
    }

    pub fn remove_chat(&mut self, chat_id: Uuid) {
        self.grants
            .retain(|g| g.chat_id != chat_id || is_persistent_scope(&g.scope));
    }

    pub fn matching_scopes(
        &self,
        chat_id: Uuid,
        tool_name: &str,
        host: Option<&str>,
        target_path: Option<&str>,
        argument_digest: Option<&str>,
    ) -> Vec<String> {
        let mut scopes: Vec<String> = self
            .grants
            .iter()
            .filter(|g| g.matches(chat_id, tool_name, host, target_path, argument_digest))
            .map(|g| g.scope.clone())
            .collect();
        scopes.sort_by_key(|scope| match scope.as_str() {
            "chat_host" | "chat_path" | "persistent_host" => 0,
            "chat_dir" | "chat" => 1,
            "persistent" => 2,
            _ => 3,
        });
        scopes.dedup();
        scopes
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
    schema: Schema,
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
        let validator = Validator::new(schema.clone());
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
            schema,
            entities,
            grant_store: Arc::new(Mutex::new(GrantStore::new())),
        })
    }

    pub fn has_action(&self, action_name: &str) -> bool {
        self.schema
            .actions()
            .any(|uid| uid.id().unescaped() == action_name)
    }

    pub fn grant_store(&self) -> Arc<Mutex<GrantStore>> {
        self.grant_store.clone()
    }

    pub async fn authorize_tool(
        &self,
        request: &AuthorizationRequest,
    ) -> Result<bool, PolicyError> {
        let mut store = self.grant_store.lock().await;
        self.authorize_locked(&mut store, request)
    }

    pub async fn authorize_with_candidate_grant(
        &self,
        request: &AuthorizationRequest,
        candidate: ScopedGrant,
    ) -> Result<bool, PolicyError> {
        let mut store = self.grant_store.lock().await;
        let added = store.add_grant(candidate.clone());
        let outcome = self.authorize_locked(&mut store, request);
        if added && !matches!(outcome, Ok(true)) {
            store.remove(&candidate);
        }
        outcome
    }

    fn authorize_locked(
        &self,
        store: &mut GrantStore,
        request: &AuthorizationRequest,
    ) -> Result<bool, PolicyError> {
        if request.approval_tier == "deny" {
            return Ok(false);
        }
        let key = grant_key(&request.tool_name);
        let host = request.target_host.as_deref();
        let target_path = request.target_path.as_deref();
        let argument_digest = request.argument_digest.as_str();
        let mut scopes = store.matching_scopes(
            request.chat_id,
            key,
            host,
            target_path,
            Some(argument_digest),
        );
        scopes.push("none".to_string());
        for scope in scopes {
            if self.evaluate(request, &scope)? {
                if scope == "once" {
                    if let Some(grant) = ScopedGrant::for_request(request, "once") {
                        store.remove(&grant);
                    }
                }
                return Ok(true);
            }
        }
        Ok(false)
    }

    fn evaluate(
        &self,
        request: &AuthorizationRequest,
        grant_scope: &str,
    ) -> Result<bool, PolicyError> {
        let action_name = cedar_action_name(&request.tool_name);

        let principal = EntityUid::from_str("Nook::User::\"local_user\"")
            .map_err(|e| PolicyError::Evaluation(e.to_string()))?;
        let action_str = format!("Nook::Action::\"{action_name}\"");
        let action =
            EntityUid::from_str(&action_str).map_err(|e| PolicyError::Evaluation(e.to_string()))?;
        let resource = EntityUid::from_str("Nook::Tool::\"local_tool\"")
            .map_err(|e| PolicyError::Evaluation(e.to_string()))?;

        let mut context_map = HashMap::new();
        context_map.insert(
            "chat_id".to_string(),
            make_str_expr(&request.chat_id.to_string())?,
        );
        context_map.insert(
            "approval_tier".to_string(),
            make_str_expr(&request.approval_tier)?,
        );
        context_map.insert("grant_scope".to_string(), make_str_expr(grant_scope)?);

        for (name, value) in &request.facts {
            context_map.insert(name.clone(), make_str_expr(value)?);
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
    use crate::policy_assets::ToolCatalog;

    const SCHEMA_SRC: &str = include_str!("../../../../policy/cedar/schema.cedarschema");
    const POLICIES_SRC: &str = include_str!("../../../../policy/cedar/policies.cedar");

    fn create_test_authorizer() -> CedarAuthorizer {
        CedarAuthorizer::from_str(SCHEMA_SRC, POLICIES_SRC).expect("valid test policy files")
    }

    const TOOLS_SRC: &str = include_str!("../../../../policy/tools.yaml");

    fn request(chat_id: Uuid, tool: &str, arguments: &str) -> AuthorizationRequest {
        let catalog = ToolCatalog::parse(TOOLS_SRC).expect("valid tool catalog");
        AuthorizationRequest::normalize(&catalog, chat_id, "call-1", tool, arguments)
            .expect("valid request")
    }

    async fn check_fetch(authorizer: &CedarAuthorizer, chat_id: Uuid, host: &str) -> bool {
        let args = serde_json::json!({ "url": format!("https://{host}") }).to_string();
        authorizer
            .authorize_tool(&request(chat_id, "nook:web_fetch", &args))
            .await
            .expect("evaluation succeeds")
    }

    #[tokio::test]
    async fn test_web_search_allowed_by_default() {
        let auth = create_test_authorizer();
        let req = request(Uuid::new_v4(), "nook:web_search", r#"{"query":"rust"}"#);
        assert!(auth.authorize_tool(&req).await.unwrap());
    }

    #[tokio::test]
    async fn once_grant_matches_argument_digest() {
        let auth = create_test_authorizer();
        let chat_id = Uuid::new_v4();
        let req = request(
            chat_id,
            "nook:web_fetch",
            r#"{"url":"https://a.example/x"}"#,
        );
        assert!(!auth.authorize_tool(&req).await.unwrap());
        auth.grant_store().lock().await.add_grant(ScopedGrant {
            chat_id,
            tool_name: "web_fetch".to_string(),
            scope: "once".to_string(),
            host: None,
            path: None,
            argument_digest: Some(req.argument_digest.clone()),
        });
        assert!(auth.authorize_tool(&req).await.unwrap());
        let other = request(
            chat_id,
            "nook:web_fetch",
            r#"{"url":"https://a.example/y"}"#,
        );
        assert!(!auth.authorize_tool(&other).await.unwrap());
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
            path: None,
            argument_digest: None,
        });

        // 3. Approved host: allowed
        assert!(check_fetch(&auth, chat_id, "docs.example.com").await);

        // 4. Unapproved host: still denied
        assert!(!check_fetch(&auth, chat_id, "unapproved.example.com").await);
    }

    fn grant(chat_id: Uuid, tool: &str, scope: &str) -> ScopedGrant {
        ScopedGrant {
            chat_id,
            tool_name: tool.to_string(),
            scope: scope.to_string(),
            host: None,
            path: None,
            argument_digest: None,
        }
    }

    #[tokio::test]
    async fn once_grant_is_consumed_by_the_call_it_authorizes() {
        let auth = create_test_authorizer();
        let chat_id = Uuid::new_v4();
        let req = request(
            chat_id,
            "nook:web_fetch",
            r#"{"url":"https://a.example/x"}"#,
        );
        auth.grant_store()
            .lock()
            .await
            .add_grant(ScopedGrant::for_request(&req, "once").unwrap());
        assert!(auth.authorize_tool(&req).await.unwrap());
        assert!(!auth.authorize_tool(&req).await.unwrap());
    }

    #[tokio::test]
    async fn once_grant_is_kept_when_cedar_denies() {
        let auth = create_test_authorizer();
        let chat_id = Uuid::new_v4();
        let req = request(
            chat_id,
            "nook:web_fetch",
            r#"{"url":"https://a.example/x"}"#,
        );
        let store = auth.grant_store();
        let other = request(chat_id, "nook:web_fetch", r#"{"url":"https://b.example/"}"#);
        store
            .lock()
            .await
            .add_grant(ScopedGrant::for_request(&other, "once").unwrap());
        assert!(!auth.authorize_tool(&req).await.unwrap());
        assert!(auth.authorize_tool(&other).await.unwrap());
    }

    #[tokio::test]
    async fn deny_tier_is_denied_even_with_a_matching_grant() {
        let auth = create_test_authorizer();
        let chat_id = Uuid::new_v4();
        let mut req = request(chat_id, "nook:web_search", r#"{"query":"rust"}"#);
        req.approval_tier = "deny".to_string();
        auth.grant_store()
            .lock()
            .await
            .add_grant(grant(chat_id, "web_search", "persistent"));
        assert!(!auth.authorize_tool(&req).await.unwrap());
    }

    fn fetch_request(chat_id: Uuid, url: &str) -> AuthorizationRequest {
        request(
            chat_id,
            "nook:web_fetch",
            &serde_json::json!({ "url": url }).to_string(),
        )
    }

    async fn has_grant(auth: &CedarAuthorizer, grant: &ScopedGrant) -> bool {
        auth.grant_store().lock().await.remove(grant)
    }

    #[tokio::test]
    async fn candidate_grant_that_allows_is_kept() {
        let auth = create_test_authorizer();
        let req = fetch_request(Uuid::new_v4(), "https://a.example/x");
        let candidate = ScopedGrant::for_request(&req, "chat_host").unwrap();
        assert!(auth
            .authorize_with_candidate_grant(&req, candidate.clone())
            .await
            .unwrap());
        assert!(auth.authorize_tool(&req).await.unwrap());
        assert!(has_grant(&auth, &candidate).await);
    }

    #[tokio::test]
    async fn candidate_grant_that_is_denied_is_removed() {
        let auth = create_test_authorizer();
        let chat_id = Uuid::new_v4();
        let req = fetch_request(chat_id, "https://a.example/x");
        let other = fetch_request(chat_id, "https://b.example/x");
        let candidate = ScopedGrant::for_request(&other, "chat_host").unwrap();
        assert!(!auth
            .authorize_with_candidate_grant(&req, candidate.clone())
            .await
            .unwrap());
        assert!(!has_grant(&auth, &candidate).await);
    }

    #[tokio::test]
    async fn denied_candidate_does_not_delete_an_equal_existing_grant() {
        let auth = create_test_authorizer();
        let chat_id = Uuid::new_v4();
        let req = fetch_request(chat_id, "https://a.example/x");
        let other = fetch_request(chat_id, "https://b.example/x");
        let existing = ScopedGrant::for_request(&other, "chat_host").unwrap();
        auth.grant_store().lock().await.add_grant(existing.clone());
        assert!(!auth
            .authorize_with_candidate_grant(&req, existing.clone())
            .await
            .unwrap());
        assert!(has_grant(&auth, &existing).await);
    }

    #[tokio::test]
    async fn allowed_once_candidate_is_consumed() {
        let auth = create_test_authorizer();
        let req = fetch_request(Uuid::new_v4(), "https://a.example/x");
        let candidate = ScopedGrant::for_request(&req, "once").unwrap();
        assert!(auth
            .authorize_with_candidate_grant(&req, candidate.clone())
            .await
            .unwrap());
        assert!(!has_grant(&auth, &candidate).await);
        assert!(!auth.authorize_tool(&req).await.unwrap());
    }

    #[tokio::test]
    async fn concurrent_identical_once_candidates_each_use_their_own_grant() {
        let auth = Arc::new(create_test_authorizer());
        let req = fetch_request(Uuid::new_v4(), "https://a.example/x");
        let candidate = ScopedGrant::for_request(&req, "once").unwrap();
        let mut tasks = Vec::new();
        for _ in 0..8 {
            let (auth, req, candidate) = (auth.clone(), req.clone(), candidate.clone());
            tasks.push(tokio::spawn(async move {
                auth.authorize_with_candidate_grant(&req, candidate)
                    .await
                    .unwrap()
            }));
        }
        for task in tasks {
            assert!(task.await.unwrap());
        }
        assert!(!has_grant(&auth, &candidate).await);
        assert!(!auth.authorize_tool(&req).await.unwrap());
    }

    #[test]
    fn remove_deletes_only_the_given_grant() {
        let chat_id = Uuid::new_v4();
        let mut store = GrantStore::new();
        let chat = grant(chat_id, "web_search", "chat");
        let persistent = grant(chat_id, "web_search", "persistent");
        store.add_grant(chat.clone());
        store.add_grant(persistent.clone());
        assert!(store.remove(&chat));
        assert!(!store.remove(&chat));
        assert_eq!(
            store.matching_scopes(chat_id, "web_search", None, None, None),
            vec!["persistent".to_string()]
        );
    }

    #[test]
    fn remove_chat_drops_that_chats_grants_and_keeps_persistent_and_other_chats() {
        let chat_id = Uuid::new_v4();
        let other_chat = Uuid::new_v4();
        let mut store = GrantStore::new();
        store.add_grant(grant(chat_id, "web_search", "chat"));
        store.add_grant(ScopedGrant {
            host: Some("a.example".to_string()),
            ..grant(chat_id, "web_fetch", "chat_host")
        });
        store.add_grant(ScopedGrant {
            argument_digest: Some("d".to_string()),
            ..grant(chat_id, "web_fetch", "once")
        });
        store.add_grant(grant(chat_id, "web_search", "persistent"));
        store.add_grant(grant(other_chat, "web_search", "chat"));
        store.remove_chat(chat_id);
        assert_eq!(
            store.matching_scopes(chat_id, "web_search", None, None, None),
            vec!["persistent".to_string()]
        );
        assert!(store
            .matching_scopes(chat_id, "web_fetch", Some("a.example"), None, Some("d"))
            .is_empty());
        assert_eq!(
            store.matching_scopes(other_chat, "web_search", None, None, None),
            vec!["chat".to_string(), "persistent".to_string()]
        );
    }

    #[test]
    fn matching_scopes_put_once_last() {
        let chat_id = Uuid::new_v4();
        let mut store = GrantStore::new();
        store.add_grant(ScopedGrant {
            argument_digest: Some("d".to_string()),
            ..grant(chat_id, "web_search", "once")
        });
        store.add_grant(grant(chat_id, "web_search", "chat"));
        assert_eq!(
            store.matching_scopes(chat_id, "web_search", None, None, Some("d")),
            vec!["chat".to_string(), "once".to_string()]
        );
    }

    #[test]
    fn grant_for_request_carries_the_scope_fields() {
        let chat_id = Uuid::new_v4();
        let req = request(
            chat_id,
            "nook:web_fetch",
            r#"{"url":"https://A.example/x"}"#,
        );
        let once = ScopedGrant::for_request(&req, "once").unwrap();
        assert_eq!(once.tool_name, "web_fetch");
        assert_eq!(once.chat_id, chat_id);
        assert_eq!(
            once.argument_digest.as_deref(),
            Some(req.argument_digest.as_str())
        );
        let host = ScopedGrant::for_request(&req, "chat_host").unwrap();
        assert_eq!(host.host.as_deref(), Some("a.example"));
        assert_eq!(host.argument_digest, None);
        assert!(ScopedGrant::for_request(&req, "bogus").is_none());
        let search = request(chat_id, "nook:web_search", r#"{"query":"q"}"#);
        assert!(ScopedGrant::for_request(&search, "chat_host").is_none());
    }
}
