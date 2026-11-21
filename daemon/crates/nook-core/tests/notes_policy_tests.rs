use nook_core::authorization::AuthorizationRequest;
use nook_core::policy::ScopedGrant;
use nook_core::policy_assets::PolicyAssets;
use serde_json::json;
use std::collections::BTreeMap;
use std::path::PathBuf;
use uuid::Uuid;

const CREATE_NOTE: &str = "nook:notes_create_note";
const LIST_NOTES: &str = "nook:notes_list_notes";
const READ_NOTE: &str = "nook:notes_read_note";
const APPEND_NOTE: &str = "nook:notes_append_note";
const EDIT_NOTE: &str = "nook:notes_edit_note";
const DELETE_NOTE: &str = "nook:notes_delete_note";

const ALL_NOTES_TOOLS: &[&str] = &[
    CREATE_NOTE,
    LIST_NOTES,
    READ_NOTE,
    APPEND_NOTE,
    EDIT_NOTE,
    DELETE_NOTE,
];

const NON_DELETE_NOTES_TOOLS: &[&str] =
    &[CREATE_NOTE, LIST_NOTES, READ_NOTE, APPEND_NOTE, EDIT_NOTE];

fn policy_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../../policy")
}

fn load_policy() -> PolicyAssets {
    PolicyAssets::load(&policy_dir()).expect("real policy files must load and validate")
}

fn notes_request(tool: &str) -> AuthorizationRequest {
    let mut facts = BTreeMap::new();
    facts.insert("folder".to_string(), "Nook".to_string());
    facts.insert("argument_digest".to_string(), "test-digest".to_string());

    AuthorizationRequest {
        chat_id: Uuid::new_v4(),
        call_id: "call-1".to_string(),
        tool_name: tool.to_string(),
        arguments: json!({}),
        target_host: None,
        target_path: None,
        target_dir: None,
        facts,
        summary: Some("Notes operation in Nook".to_string()),
        approval_tier: "ask".to_string(),
        argument_digest: "test-digest".to_string(),
    }
}

#[test]
fn tools_yaml_contains_all_six_notes_tools() {
    let policy = load_policy();
    for tool_id in ALL_NOTES_TOOLS {
        assert!(
            policy.catalog.tool(tool_id).is_some(),
            "tools.yaml must contain {tool_id}"
        );
    }
}

#[test]
fn non_delete_notes_tools_offer_allow_once_allow_for_chat_and_deny_in_tools_yaml() {
    let policy = load_policy();
    for tool_id in NON_DELETE_NOTES_TOOLS {
        let spec = policy
            .catalog
            .tool(tool_id)
            .unwrap_or_else(|| panic!("missing {tool_id} in tools.yaml"));
        assert_eq!(spec.approval.tier, "ask", "tool {tool_id} tier must be ask");
        let action_ids: Vec<&str> = spec
            .approval
            .actions
            .iter()
            .map(|a| a.id.as_str())
            .collect();
        assert_eq!(
            action_ids,
            ["allow_once", "allow_for_chat", "deny"],
            "tool {tool_id} must offer allow_once, allow_for_chat, and deny"
        );
        for action in &spec.approval.actions {
            assert!(
                !action.label.trim().is_empty(),
                "action label must not be empty"
            );
            assert!(
                !action.description.trim().is_empty(),
                "action description must not be empty"
            );
        }
    }
}

#[test]
fn delete_note_offers_allow_once_and_deny_only_in_tools_yaml() {
    let policy = load_policy();
    let spec = policy
        .catalog
        .tool(DELETE_NOTE)
        .unwrap_or_else(|| panic!("missing {DELETE_NOTE} in tools.yaml"));
    assert_eq!(spec.approval.tier, "ask", "delete_note tier must be ask");
    let action_ids: Vec<&str> = spec
        .approval
        .actions
        .iter()
        .map(|a| a.id.as_str())
        .collect();
    assert_eq!(
        action_ids,
        ["allow_once", "deny"],
        "delete_note must offer allow_once and deny only"
    );
    for action in &spec.approval.actions {
        assert!(
            !action.label.trim().is_empty(),
            "action label must not be empty"
        );
        assert!(
            !action.description.trim().is_empty(),
            "action description must not be empty"
        );
    }
}

#[test]
fn no_notes_tool_offers_always_allow_in_tools_yaml() {
    let policy = load_policy();
    for tool_id in ALL_NOTES_TOOLS {
        let spec = policy
            .catalog
            .tool(tool_id)
            .unwrap_or_else(|| panic!("missing {tool_id} in tools.yaml"));
        for action in &spec.approval.actions {
            assert_ne!(
                action.id, "always_allow",
                "{tool_id} must not offer always_allow"
            );
            assert_ne!(
                action.id, "always_allow_host",
                "{tool_id} must not offer always_allow_host"
            );
        }
    }
}

#[test]
fn cedar_schema_defines_all_six_notes_actions() {
    let policy = load_policy();
    for tool_id in ALL_NOTES_TOOLS {
        let action_name = tool_id.strip_prefix("nook:").unwrap();
        assert!(
            policy.authorizer.has_action(action_name),
            "Cedar schema must define action {action_name}"
        );
    }
}

#[tokio::test]
async fn non_delete_notes_tools_are_permitted_with_once_grant() {
    let policy = load_policy();
    for tool_id in NON_DELETE_NOTES_TOOLS {
        let req = notes_request(tool_id);
        let candidate = ScopedGrant::for_request(&req, "once").expect("once grant");
        let allowed = policy
            .authorizer
            .authorize_with_candidate_grant(&req, candidate)
            .await
            .expect("evaluation succeeds");
        assert!(allowed, "{tool_id} must be permitted with once grant");
    }
}

#[tokio::test]
async fn non_delete_notes_tools_are_permitted_with_chat_grant() {
    let policy = load_policy();
    for tool_id in NON_DELETE_NOTES_TOOLS {
        let req = notes_request(tool_id);
        let candidate = ScopedGrant::for_request(&req, "chat").expect("chat grant");
        let allowed = policy
            .authorizer
            .authorize_with_candidate_grant(&req, candidate)
            .await
            .expect("evaluation succeeds");
        assert!(allowed, "{tool_id} must be permitted with chat grant");
    }
}

#[tokio::test]
async fn non_delete_notes_tools_are_denied_without_a_grant() {
    let policy = load_policy();
    for tool_id in NON_DELETE_NOTES_TOOLS {
        let req = notes_request(tool_id);
        let allowed = policy
            .authorizer
            .authorize_tool(&req)
            .await
            .expect("evaluation succeeds");
        assert!(!allowed, "{tool_id} must be denied without a grant");
    }
}

#[tokio::test]
async fn non_delete_notes_tools_are_denied_with_a_persistent_grant() {
    let policy = load_policy();
    for tool_id in NON_DELETE_NOTES_TOOLS {
        let req = notes_request(tool_id);
        let candidate = ScopedGrant::for_request(&req, "persistent").expect("persistent grant");
        let allowed = policy
            .authorizer
            .authorize_with_candidate_grant(&req, candidate)
            .await
            .expect("evaluation succeeds");
        assert!(!allowed, "{tool_id} must be denied with a persistent grant");
    }
}

#[tokio::test]
async fn delete_note_is_permitted_with_once_grant() {
    let policy = load_policy();
    let req = notes_request(DELETE_NOTE);
    let candidate = ScopedGrant::for_request(&req, "once").expect("once grant");
    let allowed = policy
        .authorizer
        .authorize_with_candidate_grant(&req, candidate)
        .await
        .expect("evaluation succeeds");
    assert!(allowed, "{DELETE_NOTE} must be permitted with once grant");
}

#[tokio::test]
async fn delete_note_is_denied_with_chat_grant() {
    let policy = load_policy();
    let req = notes_request(DELETE_NOTE);
    let candidate = ScopedGrant::for_request(&req, "chat").expect("chat grant");
    let allowed = policy
        .authorizer
        .authorize_with_candidate_grant(&req, candidate)
        .await
        .expect("evaluation succeeds");
    assert!(!allowed, "{DELETE_NOTE} must be denied with chat grant");
}

#[tokio::test]
async fn delete_note_is_denied_without_a_grant() {
    let policy = load_policy();
    let req = notes_request(DELETE_NOTE);
    let allowed = policy
        .authorizer
        .authorize_tool(&req)
        .await
        .expect("evaluation succeeds");
    assert!(!allowed, "{DELETE_NOTE} must be denied without a grant");
}

#[tokio::test]
async fn delete_note_is_denied_with_a_persistent_grant() {
    let policy = load_policy();
    let req = notes_request(DELETE_NOTE);
    let candidate = ScopedGrant::for_request(&req, "persistent").expect("persistent grant");
    let allowed = policy
        .authorizer
        .authorize_with_candidate_grant(&req, candidate)
        .await
        .expect("evaluation succeeds");
    assert!(
        !allowed,
        "{DELETE_NOTE} must be denied with a persistent grant"
    );
}
