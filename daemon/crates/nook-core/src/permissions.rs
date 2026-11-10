use crate::db::{self, StoredGrant};
use crate::policy::{grant_key, ScopedGrant};
use crate::protocol::{GrantInfo, TierChoice, ToolPolicyInfo};
use crate::server::AppState;
use tracing::warn;
use uuid::Uuid;

pub const DEFAULT_CHOICE: &str = "default";

fn tier_rank(tier: &str) -> u8 {
    match tier {
        "allow" => 0,
        "deny" => 2,
        _ => 1,
    }
}

/// A preference can only make a tool stricter. A looser or equal one is ignored.
pub fn effective_tier(default_tier: &str, preference: Option<&str>) -> String {
    match preference {
        Some(choice) if tier_rank(choice) > tier_rank(default_tier) => choice.to_string(),
        _ => default_tier.to_string(),
    }
}

pub fn allowed_preferences(default_tier: &str) -> &'static [&'static str] {
    match tier_rank(default_tier) {
        0 => &["ask", "deny"],
        1 => &["deny"],
        _ => &[],
    }
}

fn default_effect(tier: &str) -> &'static str {
    match tier {
        "allow" => "Nook runs this tool without asking.",
        "deny" => "Nook never runs this tool.",
        "ask_once_per_host" => "Nook asks once for each website, then remembers your answer.",
        _ => "Nook asks before it uses this tool.",
    }
}

fn choices_for(default_tier: &str) -> Vec<TierChoice> {
    let mut choices = vec![TierChoice {
        tier: DEFAULT_CHOICE.to_string(),
        label: "Default".to_string(),
        effect: default_effect(default_tier).to_string(),
    }];
    for tier in allowed_preferences(default_tier) {
        let (label, effect) = match *tier {
            "ask" => (
                "Ask every time",
                "Nook asks before it uses this tool, unless you already granted it.",
            ),
            _ => (
                "Never allow",
                "Nook never runs this tool. Stored grants stay but have no effect.",
            ),
        };
        choices.push(TierChoice {
            tier: (*tier).to_string(),
            label: label.to_string(),
            effect: effect.to_string(),
        });
    }
    choices
}

/// The tools the user can see: the Worker tools and the Daemon tools that exist.
fn known_tools(state: &AppState) -> Vec<(String, String, String)> {
    let mut tools = Vec::new();
    for (id, spec) in &state.policy.catalog.tools {
        let available = crate::authorization::WORKER_TOOLS.contains(&id.as_str())
            || state.providers.contains(id);
        if available {
            tools.push((
                id.clone(),
                spec.display_name.clone(),
                spec.approval.tier.clone(),
            ));
        }
    }
    if let Some(shared) = state.policy.catalog.resolve("mcp:") {
        for descriptor in state.providers.descriptors() {
            if descriptor.id.starts_with("mcp:") {
                let label = descriptor.id.trim_start_matches("mcp:").replace(':', " / ");
                tools.push((
                    descriptor.id.clone(),
                    format!("MCP: {label}"),
                    shared.approval.tier.clone(),
                ));
            }
        }
    }
    tools
}

fn tool_name(state: &AppState, key: &str) -> String {
    if let Some(id) = key.strip_prefix("mcp:") {
        return format!("MCP: {}", id.replace(':', " / "));
    }
    let full = format!("nook:{key}");
    state
        .policy
        .catalog
        .tool(&full)
        .map(|spec| spec.display_name.clone())
        .unwrap_or_else(|| key.to_string())
}

fn describe(state: &AppState, grant: &ScopedGrant) -> GrantInfo {
    let name = tool_name(state, &grant.tool_name);
    let description = match (grant.scope.as_str(), grant.host.as_deref()) {
        ("persistent_host", Some(host)) => format!("Always allow {name} for {host}, in every chat"),
        _ => format!("Always allow {name}, in every chat"),
    };
    GrantInfo {
        grant_id: grant.id(),
        tool_id: grant.tool_name.clone(),
        tool_name: name,
        scope: grant.scope.clone(),
        description,
    }
}

pub async fn snapshot(state: &AppState) -> (Vec<GrantInfo>, Vec<ToolPolicyInfo>) {
    let grants = state
        .policy
        .authorizer
        .grant_store()
        .lock()
        .await
        .persistent_grants()
        .iter()
        .map(|grant| describe(state, grant))
        .collect();
    let tools = known_tools(state)
        .into_iter()
        .map(|(tool_id, display_name, default_tier)| {
            let preference = state.policy.preference(&tool_id);
            let current = preference
                .filter(|choice| allowed_preferences(&default_tier).contains(&choice.as_str()))
                .unwrap_or_else(|| DEFAULT_CHOICE.to_string());
            ToolPolicyInfo {
                choices: choices_for(&default_tier),
                tool_id,
                display_name,
                default_tier,
                tier: current,
            }
        })
        .collect();
    (grants, tools)
}

pub async fn revoke_grant(state: &AppState, grant_id: &str) -> Result<(), &'static str> {
    let grant = {
        let mut store = state.policy.authorizer.grant_store().lock_owned().await;
        let found = store
            .persistent_grants()
            .into_iter()
            .find(|grant| grant.id() == grant_id)
            .ok_or("Unknown grant")?;
        store.remove(&found);
        found
    };
    if let Some(pool) = state.pool.get() {
        if let Err(error) = db::delete_persistent_grant(&pool, &grant.id()).await {
            warn!("Failed to delete a revoked grant from the database: {error}");
        }
    }
    Ok(())
}

pub async fn set_preference(
    state: &AppState,
    tool_id: &str,
    choice: &str,
) -> Result<(), &'static str> {
    let (_, _, default_tier) = known_tools(state)
        .into_iter()
        .find(|(id, _, _)| id == tool_id)
        .ok_or("Unknown tool")?;
    if choice == DEFAULT_CHOICE {
        state.policy.set_preference(tool_id, None);
        if let Some(pool) = state.pool.get() {
            if let Err(error) = db::delete_tool_preference(&pool, tool_id).await {
                warn!("Failed to delete a tool preference: {error}");
            }
        }
        return Ok(());
    }
    if !allowed_preferences(&default_tier).contains(&choice) {
        return Err("That choice is not allowed for this tool");
    }
    state.policy.set_preference(tool_id, Some(choice));
    if let Some(pool) = state.pool.get() {
        if let Err(error) = db::save_tool_preference(&pool, tool_id, choice).await {
            warn!("Failed to save a tool preference: {error}");
        }
    }
    Ok(())
}

pub fn stored_grant(grant: &ScopedGrant) -> StoredGrant {
    StoredGrant {
        id: grant.id(),
        tool_key: grant.tool_name.clone(),
        scope: grant.scope.clone(),
        host: grant.host.clone(),
    }
}

pub fn grant_from_stored(stored: &StoredGrant) -> Option<ScopedGrant> {
    if !crate::policy::is_persistent_scope(&stored.scope) {
        return None;
    }
    let grant = ScopedGrant {
        chat_id: Uuid::nil(),
        tool_name: stored.tool_key.clone(),
        scope: stored.scope.clone(),
        host: stored.host.clone(),
        path: None,
        argument_digest: None,
    };
    (grant.id() == stored.id && grant_key(&stored.tool_key) == stored.tool_key).then_some(grant)
}

/// Loads the stored grants and preferences. Rows that fail a check are ignored.
pub async fn restore(state: &AppState) {
    let Some(pool) = state.pool.get() else { return };
    match db::load_persistent_grants(&pool).await {
        Ok(rows) => {
            let store = state.policy.authorizer.grant_store();
            let mut store = store.lock().await;
            for row in rows {
                match grant_from_stored(&row) {
                    Some(grant) => {
                        store.add_grant(grant);
                    }
                    None => warn!("Ignored a stored grant that does not check out: {}", row.id),
                }
            }
        }
        Err(error) => warn!("Failed to load persistent grants: {error}"),
    }
    match db::load_tool_preferences(&pool).await {
        Ok(rows) => {
            for (tool_id, tier) in rows {
                state.policy.set_preference(&tool_id, Some(&tier));
            }
        }
        Err(error) => warn!("Failed to load tool preferences: {error}"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_preference_only_makes_a_tool_stricter() {
        assert_eq!(effective_tier("allow", Some("ask")), "ask");
        assert_eq!(effective_tier("allow", Some("deny")), "deny");
        assert_eq!(effective_tier("ask", Some("deny")), "deny");
        assert_eq!(effective_tier("ask_once_per_host", Some("deny")), "deny");
    }

    #[test]
    fn a_looser_or_equal_preference_is_ignored() {
        assert_eq!(effective_tier("ask", Some("ask")), "ask");
        assert_eq!(
            effective_tier("ask_once_per_host", Some("ask")),
            "ask_once_per_host"
        );
        assert_eq!(effective_tier("deny", Some("ask")), "deny");
        assert_eq!(effective_tier("deny", Some("allow")), "deny");
        assert_eq!(effective_tier("ask", Some("allow")), "ask");
        assert_eq!(effective_tier("ask", None), "ask");
        assert_eq!(effective_tier("ask", Some("nonsense")), "ask");
    }

    #[test]
    fn the_choices_match_what_a_tool_may_do() {
        assert_eq!(allowed_preferences("allow"), ["ask", "deny"]);
        assert_eq!(allowed_preferences("ask"), ["deny"]);
        assert_eq!(allowed_preferences("ask_once_per_host"), ["deny"]);
        assert!(allowed_preferences("deny").is_empty());
        let tiers: Vec<String> = choices_for("allow").into_iter().map(|c| c.tier).collect();
        assert_eq!(tiers, ["default", "ask", "deny"]);
        assert!(choices_for("deny").len() == 1);
    }

    #[test]
    fn a_grant_has_one_stable_id() {
        let grant = ScopedGrant {
            chat_id: Uuid::nil(),
            tool_name: "web_fetch".to_string(),
            scope: "persistent_host".to_string(),
            host: Some("example.com".to_string()),
            path: None,
            argument_digest: None,
        };
        let other_host = ScopedGrant {
            host: Some("example.org".to_string()),
            ..grant.clone()
        };

        assert_eq!(grant.id(), grant.clone().id());
        assert_ne!(grant.id(), other_host.id());
        assert_eq!(grant.id().len(), 32);
    }

    #[test]
    fn a_stored_grant_loads_only_if_its_id_matches() {
        let grant = ScopedGrant {
            chat_id: Uuid::nil(),
            tool_name: "mcp:fake:echo".to_string(),
            scope: "persistent".to_string(),
            host: None,
            path: None,
            argument_digest: None,
        };
        let stored = stored_grant(&grant);

        assert_eq!(grant_from_stored(&stored), Some(grant));
        let mut tampered = stored.clone();
        tampered.host = Some("evil.example".to_string());
        assert_eq!(grant_from_stored(&tampered), None);
        let mut wrong_scope = stored;
        wrong_scope.scope = "chat".to_string();
        assert_eq!(grant_from_stored(&wrong_scope), None);
    }
}
