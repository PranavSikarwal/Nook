use crate::authorization::{AuthorizationError, AuthorizationRequest};
use crate::policy::ScopedGrant;
use crate::policy_assets::approval_action_scope;
use crate::protocol::{
    AuthorizationDecision, DaemonMessage, DaemonWorkerRequest, ErrorCode, ErrorInfo, ToolStatus,
};
use crate::providers::{
    roots::OUTSIDE_ROOTS, truncate_output, CallContext, ProviderError, ProviderRegistry,
};
use crate::server::{
    claim_pending_approval, forward_unless_cancelled, AppState, PendingApprovalEntry,
};
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::mpsc;
use tracing::warn;
use uuid::Uuid;

pub const APPROVAL_TIMEOUT: Duration = Duration::from_secs(300);
pub const MAX_TOOL_OUTPUT_BYTES: usize = 64 * 1024;
pub const PANEL_RESULT_CHARS: usize = 2000;

pub struct AuthorizeContext {
    pub client_request_id: Uuid,
    pub worker_request_id: Uuid,
    pub chat_id: Uuid,
}

pub struct AuthorizeToolCall {
    pub message_id: Uuid,
    pub call_id: String,
    pub chat_id: Uuid,
    pub tool_name: String,
    pub arguments: String,
    pub execute: bool,
}

/// What the Daemon does with an allowed call that it runs itself.
#[derive(Debug, Clone)]
pub struct Execution {
    pub client_request_id: Uuid,
    pub message_id: Uuid,
    pub chat_id: Uuid,
    pub tool_name: String,
    pub arguments: String,
    pub tx_out: mpsc::Sender<DaemonMessage>,
    pub registry: Arc<ProviderRegistry>,
}

/// Where the verdict for one call goes.
#[derive(Debug, Clone)]
pub struct Delivery {
    pub worker_request_id: Uuid,
    pub call_id: String,
    pub execution: Option<Execution>,
}

type Verdict = (AuthorizationDecision, Option<String>);

fn deny(reason: impl Into<String>) -> Verdict {
    (AuthorizationDecision::Deny, Some(reason.into()))
}

fn argument_denial(error: &AuthorizationError) -> &'static str {
    match error {
        AuthorizationError::UnknownTool(_) => "Unknown tool",
        AuthorizationError::Rejected(reason) => reason,
        _ => "Invalid arguments",
    }
}

fn policy_failure(error: impl std::fmt::Display) -> Verdict {
    warn!("Authorization evaluation failed: {error}");
    deny("Authorization failed")
}

async fn send_to_worker(state: &AppState, request: DaemonWorkerRequest) -> Result<(), String> {
    let supervisor = state.supervisor.lock().await.clone();
    let supervisor = supervisor.ok_or_else(|| "Worker supervisor is not available".to_string())?;
    supervisor
        .send_request(request)
        .await
        .map(|_| ())
        .map_err(|error| error.to_string())
}

async fn send_result(
    state: &AppState,
    worker_request_id: Uuid,
    call_id: &str,
    verdict: Verdict,
) -> Result<(), String> {
    let (decision, reason) = verdict;
    send_to_worker(
        state,
        DaemonWorkerRequest::AuthorizationResult {
            request_id: worker_request_id,
            call_id: call_id.to_string(),
            decision,
            reason,
        },
    )
    .await
}

async fn send_tool_result(
    state: &AppState,
    worker_request_id: Uuid,
    call_id: &str,
    status: ToolStatus,
    output: String,
) -> Result<(), String> {
    send_to_worker(
        state,
        DaemonWorkerRequest::ToolResult {
            request_id: worker_request_id,
            call_id: call_id.to_string(),
            status,
            output,
        },
    )
    .await
}

/// Sends a verdict to the Worker, or starts the call when the Daemon runs it.
pub async fn deliver(
    state: &Arc<AppState>,
    delivery: &Delivery,
    verdict: Verdict,
) -> Result<(), String> {
    match (&delivery.execution, verdict) {
        (None, verdict) => {
            send_result(
                state,
                delivery.worker_request_id,
                &delivery.call_id,
                verdict,
            )
            .await
        }
        (Some(execution), (AuthorizationDecision::Allow, _)) => {
            start_execution(state, delivery, execution).await;
            Ok(())
        }
        (Some(_), (AuthorizationDecision::Deny, reason)) => {
            send_tool_result(
                state,
                delivery.worker_request_id,
                &delivery.call_id,
                ToolStatus::Denied,
                reason.unwrap_or_default(),
            )
            .await
        }
    }
}

async fn deliver_logged(state: &Arc<AppState>, delivery: &Delivery, verdict: Verdict) {
    if let Err(error) = deliver(state, delivery, verdict).await {
        warn!(
            "Failed to deliver the result for {}: {error}",
            delivery.call_id
        );
    }
}

async fn start_execution(state: &Arc<AppState>, delivery: &Delivery, execution: &Execution) {
    let client_id = execution.client_request_id;
    forward_unless_cancelled(
        state,
        client_id,
        DaemonMessage::ToolCallStarted {
            id: client_id,
            chat_id: execution.chat_id,
            message_id: execution.message_id,
            call_id: delivery.call_id.clone(),
            name: execution.tool_name.clone(),
            arguments: execution.arguments.clone(),
        },
        &execution.tx_out,
    )
    .await;

    let task_state = state.clone();
    let task_delivery = delivery.clone();
    let task_execution = execution.clone();
    let handle = tokio::spawn(async move {
        let arguments = serde_json::from_str(&task_execution.arguments).unwrap_or_default();
        let output = task_execution
            .registry
            .call(
                &task_execution.tool_name,
                arguments,
                CallContext {
                    chat_id: task_execution.chat_id,
                    call_id: task_delivery.call_id.clone(),
                },
            )
            .await;
        let text = truncate_output(output.text, MAX_TOOL_OUTPUT_BYTES);
        let panel_text: String = text.chars().take(PANEL_RESULT_CHARS).collect();
        forward_unless_cancelled(
            &task_state,
            task_execution.client_request_id,
            DaemonMessage::ToolCallFinished {
                id: task_execution.client_request_id,
                chat_id: task_execution.chat_id,
                message_id: task_execution.message_id,
                call_id: task_delivery.call_id.clone(),
                result: panel_text,
            },
            &task_execution.tx_out,
        )
        .await;
        let status = if output.is_error {
            ToolStatus::Error
        } else {
            ToolStatus::Ok
        };
        if let Err(error) = send_tool_result(
            &task_state,
            task_delivery.worker_request_id,
            &task_delivery.call_id,
            status,
            text,
        )
        .await
        {
            warn!(
                "Failed to send the tool result for {}: {error}",
                task_delivery.call_id
            );
        }
        let mut running = task_state.running_tools.lock().await;
        if let Some(calls) = running.get_mut(&task_execution.client_request_id) {
            calls.retain(|(running_call, _)| running_call != &task_delivery.call_id);
        }
    });

    let cancelled = state.cancelled_requests.lock().await.contains(&client_id);
    if cancelled {
        handle.abort();
        return;
    }
    state
        .running_tools
        .lock()
        .await
        .entry(client_id)
        .or_default()
        .push((delivery.call_id.clone(), handle.abort_handle()));
}

pub async fn abort_running_tools(state: &AppState, client_request_id: Uuid) {
    let calls = state.running_tools.lock().await.remove(&client_request_id);
    for (_, handle) in calls.unwrap_or_default() {
        handle.abort();
    }
}

/// Aborts every running tool task and waits for each task to finish.
pub async fn abort_all_running_tools(state: &AppState) {
    let handles = {
        let mut running = state.running_tools.lock().await;
        running
            .drain()
            .flat_map(|(_, calls)| calls.into_iter().map(|(_, handle)| handle))
            .collect::<Vec<_>>()
    };
    for handle in &handles {
        handle.abort();
    }
    while handles.iter().any(|handle| !handle.is_finished()) {
        tokio::time::sleep(Duration::from_millis(10)).await;
    }
}

fn delivery_for(call_id: &str, entry: &PendingApprovalEntry) -> Delivery {
    Delivery {
        worker_request_id: entry.worker_request_id,
        call_id: call_id.to_string(),
        execution: entry.execution.clone(),
    }
}

pub async fn expire_pending_approval(
    state: &Arc<AppState>,
    call_id: &str,
    worker_request_id: Uuid,
    after: Duration,
) {
    tokio::time::sleep(after).await;
    let expired = {
        let mut approvals = state.pending_approvals.lock().await;
        let still_pending = approvals.get(call_id).is_some_and(|entry| {
            entry.worker_request_id == worker_request_id && !entry.decision_in_flight
        });
        if still_pending {
            approvals.remove(call_id)
        } else {
            None
        }
    };
    if let Some(entry) = expired {
        let delivery = delivery_for(call_id, &entry);
        deliver_logged(state, &delivery, deny("Approval timed out")).await;
    }
}

fn stale_reason(error: &ProviderError) -> &'static str {
    match error {
        ProviderError::Rejected(reason) => reason,
        _ => OUTSIDE_ROOTS,
    }
}

/// Denies each waiting approval whose call the current tools no longer accept,
/// for example a write in a folder that the user just removed.
pub async fn deny_stale_approvals(state: &Arc<AppState>) {
    let registry = state.providers.current();
    let stale: Vec<(String, PendingApprovalEntry, &'static str)> = {
        let mut approvals = state.pending_approvals.lock().await;
        let stale_calls: Vec<(String, &'static str)> = approvals
            .iter()
            .filter_map(|(call_id, entry)| {
                if entry.execution.is_none() || entry.decision_in_flight {
                    return None;
                }
                let error = registry
                    .facts(&entry.request.tool_name, &entry.request.arguments)
                    .err()?;
                Some((call_id.clone(), stale_reason(&error)))
            })
            .collect();
        stale_calls
            .into_iter()
            .filter_map(|(call_id, reason)| {
                approvals
                    .remove(&call_id)
                    .map(|entry| (call_id, entry, reason))
            })
            .collect()
    };
    for (call_id, entry, reason) in stale {
        let delivery = delivery_for(&call_id, &entry);
        deliver_logged(state, &delivery, deny(reason)).await;
    }
}

pub async fn handle_authorize_tool(
    state: &Arc<AppState>,
    ctx: &AuthorizeContext,
    call: AuthorizeToolCall,
    tx_out: &mpsc::Sender<DaemonMessage>,
) {
    let call_id = call.call_id.clone();
    let cfg = state.config.read().await;
    let registry = state.providers.current();
    let runs_in_daemon = registry.contains(&call.tool_name);
    let execution = call.execute.then(|| Execution {
        client_request_id: ctx.client_request_id,
        message_id: call.message_id,
        chat_id: ctx.chat_id,
        tool_name: call.tool_name.clone(),
        arguments: call.arguments.clone(),
        tx_out: tx_out.clone(),
        registry: registry.clone(),
    });
    let delivery = Delivery {
        worker_request_id: ctx.worker_request_id,
        call_id: call_id.clone(),
        execution,
    };
    if call.chat_id != ctx.chat_id {
        drop(cfg);
        let verdict = deny("Request does not match the running chat");
        return deliver_logged(state, &delivery, verdict).await;
    }
    if call.execute != runs_in_daemon && state.policy.catalog.resolve(&call.tool_name).is_some() {
        drop(cfg);
        let reason = if runs_in_daemon {
            "Tool runs in the Daemon"
        } else {
            "Tool runs in the Worker"
        };
        return deliver_logged(state, &delivery, deny(reason)).await;
    }
    let mut request = match AuthorizationRequest::normalize_with(
        &state.policy.catalog,
        Some(&registry),
        ctx.chat_id,
        &call.call_id,
        &call.tool_name,
        &call.arguments,
    ) {
        Ok(request) => request,
        Err(error) => {
            drop(cfg);
            warn!("Tool arguments rejected: {error}");
            let verdict = deny(argument_denial(&error));
            return deliver_logged(state, &delivery, verdict).await;
        }
    };

    if is_notes_tool(&request.tool_name) {
        request
            .facts
            .insert("folder".to_string(), cfg.tools.notes.folder.clone());
    }

    request.approval_tier = state
        .policy
        .effective_tier(&request.tool_name, &request.approval_tier);

    match state.policy.authorizer.authorize_tool(&request).await {
        Ok(true) => {
            drop(cfg);
            let verdict = (AuthorizationDecision::Allow, None);
            return deliver_logged(state, &delivery, verdict).await;
        }
        Ok(false) => {}
        Err(error) => {
            drop(cfg);
            let verdict = policy_failure(error);
            return deliver_logged(state, &delivery, verdict).await;
        }
    }

    if request.approval_tier == "deny" {
        drop(cfg);
        let verdict = deny("The tool is denied by policy");
        return deliver_logged(state, &delivery, verdict).await;
    }
    let spec = state.policy.catalog.resolve(&request.tool_name);
    let (explanation, actions) = match spec {
        Some(spec) if !spec.approval.actions.is_empty() => {
            (spec.description.clone(), spec.approval.actions.clone())
        }
        _ => {
            drop(cfg);
            let verdict = deny("No approval action is available for this tool");
            return deliver_logged(state, &delivery, verdict).await;
        }
    };

    let resource_summary = request.resource_summary();
    {
        let mut approvals = state.pending_approvals.lock().await;
        if approvals.contains_key(&call_id) {
            drop(approvals);
            drop(cfg);
            let verdict = deny("An approval for this call is already pending");
            return deliver_logged(state, &delivery, verdict).await;
        }
        approvals.insert(
            call_id.clone(),
            PendingApprovalEntry {
                chat_id: ctx.chat_id,
                client_request_id: ctx.client_request_id,
                worker_request_id: ctx.worker_request_id,
                decision_in_flight: false,
                request,
                offered_actions: actions.iter().map(|action| action.id.clone()).collect(),
                execution: delivery.execution.clone(),
            },
        );
    }
    drop(cfg);

    let prompt = DaemonMessage::ApprovalRequested {
        id: ctx.client_request_id,
        chat_id: ctx.chat_id,
        message_id: call.message_id,
        call_id: call_id.clone(),
        tool_name: call.tool_name,
        arguments: call.arguments,
        explanation,
        resource_summary,
        actions,
    };
    if tx_out.send(prompt).await.is_err() {
        state.pending_approvals.lock().await.remove(&call_id);
        let verdict = deny("Approval could not be shown");
        return deliver_logged(state, &delivery, verdict).await;
    }
    let weak_state = Arc::downgrade(state);
    let (timeout, worker_request_id) = (state.approval_timeout, ctx.worker_request_id);
    tokio::spawn(async move {
        tokio::time::sleep(timeout).await;
        if let Some(state) = weak_state.upgrade() {
            expire_pending_approval(&state, &call_id, worker_request_id, Duration::ZERO).await;
        }
    });
}

async fn resolve_decision(state: &AppState, entry: &PendingApprovalEntry, action: &str) -> Verdict {
    let scope = match approval_action_scope(action) {
        None => return deny("Unsupported approval action"),
        Some(None) => return deny("Denied by the user"),
        Some(Some(scope)) => scope,
    };
    let Some(grant) = ScopedGrant::for_request(&entry.request, scope) else {
        return deny("The approval action does not apply to this request");
    };
    match state
        .policy
        .authorizer
        .authorize_with_candidate_grant(&entry.request, grant)
        .await
    {
        Ok(true) => (AuthorizationDecision::Allow, None),
        Ok(false) => deny("Policy does not allow this request"),
        Err(error) => policy_failure(error),
    }
}

async fn send_error(
    tx_out: &mpsc::Sender<DaemonMessage>,
    id: Uuid,
    message: String,
    retryable: bool,
) {
    let _ = tx_out
        .send(DaemonMessage::Error {
            id,
            error: ErrorInfo {
                code: ErrorCode::InvalidRequest,
                message,
                retryable,
            },
        })
        .await;
}

async fn store_persistent_grant(state: &AppState, entry: &PendingApprovalEntry, action: &str) {
    let Some(Some(scope)) = approval_action_scope(action) else {
        return;
    };
    if !crate::policy::is_persistent_scope(scope) {
        return;
    }
    let (Some(grant), Some(pool)) = (
        ScopedGrant::for_request(&entry.request, scope),
        state.pool.get(),
    ) else {
        return;
    };
    if let Err(error) =
        crate::db::save_persistent_grant(&pool, &crate::permissions::stored_grant(&grant)).await
    {
        warn!("Failed to store a persistent grant: {error}");
    }
}

pub async fn handle_approval_decision(
    state: &Arc<AppState>,
    id: Uuid,
    chat_id: Uuid,
    call_id: String,
    action: String,
    tx_out: &mpsc::Sender<DaemonMessage>,
) {
    let claimed = {
        let mut approvals = state.pending_approvals.lock().await;
        claim_pending_approval(&mut approvals, &call_id, chat_id, &action)
    };
    let entry = match claimed {
        Ok(entry) => entry,
        Err(message) => return send_error(tx_out, id, message.to_string(), false).await,
    };

    let (verdict, execution) = {
        let cfg = state.config.read().await;
        let registry = state.providers.current();
        if is_notes_tool(&entry.request.tool_name)
            && entry.folder() != Some(cfg.tools.notes.folder.as_str())
        {
            (deny(NOTES_FOLDER_CHANGED), entry.execution.clone())
        } else {
            let mut execution = entry.execution.clone();
            if let Some(ref mut exec) = execution {
                exec.registry = registry;
            }
            let verdict = resolve_decision(state, &entry, &action).await;
            (verdict, execution)
        }
    };

    if verdict.0 == AuthorizationDecision::Allow {
        store_persistent_grant(state, &entry, &action).await;
    }
    let delivery = Delivery {
        worker_request_id: entry.worker_request_id,
        call_id: call_id.clone(),
        execution,
    };
    match deliver(state, &delivery, verdict).await {
        Ok(()) => {
            state.pending_approvals.lock().await.remove(&call_id);
        }
        Err(error) => {
            if let Some(pending) = state.pending_approvals.lock().await.get_mut(&call_id) {
                pending.decision_in_flight = false;
            }
            let message = format!("Failed to deliver the authorization result: {error}");
            send_error(tx_out, id, message, true).await;
        }
    }
}

pub async fn deny_pending_for_request(
    state: &Arc<AppState>,
    client_request_id: Uuid,
    reason: &str,
) {
    let dropped: Vec<(String, PendingApprovalEntry)> = {
        let mut approvals = state.pending_approvals.lock().await;
        let call_ids: Vec<String> = approvals
            .iter()
            .filter(|(_, entry)| {
                entry.client_request_id == client_request_id && !entry.decision_in_flight
            })
            .map(|(call_id, _)| call_id.clone())
            .collect();
        call_ids
            .into_iter()
            .filter_map(|call_id| approvals.remove(&call_id).map(|entry| (call_id, entry)))
            .collect()
    };
    for (call_id, entry) in dropped {
        let delivery = delivery_for(&call_id, &entry);
        deliver_logged(state, &delivery, deny(reason)).await;
    }
}

pub const NOTES_FOLDER_CHANGED: &str = "The Notes folder changed";

pub fn is_notes_tool(tool_name: &str) -> bool {
    tool_name.starts_with("nook:notes_")
}

/// Denies every pending Notes approval with "The Notes folder changed",
/// skipping entries with decision_in_flight.
pub async fn deny_pending_notes_approvals(state: &Arc<AppState>) {
    let dropped: Vec<(String, PendingApprovalEntry)> = {
        let mut approvals = state.pending_approvals.lock().await;
        let call_ids: Vec<String> = approvals
            .iter()
            .filter(|(_, entry)| {
                is_notes_tool(&entry.request.tool_name) && !entry.decision_in_flight
            })
            .map(|(call_id, _)| call_id.clone())
            .collect();
        call_ids
            .into_iter()
            .filter_map(|call_id| approvals.remove(&call_id).map(|entry| (call_id, entry)))
            .collect()
    };
    for (call_id, entry) in dropped {
        let delivery = delivery_for(&call_id, &entry);
        deliver_logged(state, &delivery, deny(NOTES_FOLDER_CHANGED)).await;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::Config;
    use crate::policy_assets::PolicyAssets;
    use crate::protocol::ToolDescriptor;
    use crate::providers::{Provider, ToolFacts, ToolOutput};
    use async_trait::async_trait;
    use serde_json::json;
    use std::collections::{HashMap, HashSet};
    use std::path::PathBuf;
    use std::sync::atomic::{AtomicUsize, Ordering};
    use tokio::sync::{mpsc, Mutex, RwLock};

    struct TestProvider(AtomicUsize);

    #[async_trait]
    impl Provider for TestProvider {
        fn descriptors(&self) -> Vec<ToolDescriptor> {
            vec![ToolDescriptor {
                id: "nook:test_tool".to_string(),
                name: "test_tool".to_string(),
                description: "Test tool.".to_string(),
                input_schema: json!({ "type": "object" }),
            }]
        }

        fn facts(
            &self,
            _id: &str,
            _arguments: &serde_json::Value,
        ) -> Result<ToolFacts, ProviderError> {
            Ok(ToolFacts::default())
        }

        async fn call(
            &self,
            _id: &str,
            _arguments: serde_json::Value,
            _context: CallContext,
        ) -> ToolOutput {
            self.0.fetch_add(1, Ordering::SeqCst);
            ToolOutput::ok("done")
        }
    }

    const SCHEMA: &str = include_str!("../../../../policy/cedar/schema.cedarschema");
    const POLICIES: &str = include_str!("../../../../policy/cedar/policies.cedar");
    const TOOLS: &str = include_str!("../../../../policy/tools.yaml");

    #[tokio::test]
    async fn execution_keeps_using_its_registry_after_handle_is_replaced() {
        let provider = Arc::new(TestProvider(AtomicUsize::new(0)));
        let registry = Arc::new(ProviderRegistry::new(vec![provider.clone()]).unwrap());
        let (tx_out, mut rx_out) = mpsc::channel(16);
        let client_id = Uuid::new_v4();
        let message_id = Uuid::new_v4();
        let chat_id = Uuid::new_v4();

        let execution = Execution {
            client_request_id: client_id,
            message_id,
            chat_id,
            tool_name: "nook:test_tool".to_string(),
            arguments: "{}".to_string(),
            tx_out,
            registry: registry.clone(),
        };

        let delivery = Delivery {
            worker_request_id: Uuid::new_v4(),
            call_id: "call_1".to_string(),
            execution: Some(execution.clone()),
        };

        let state = Arc::new(AppState {
            config: RwLock::new(Config::default()),
            config_path: PathBuf::from("/tmp/nook-test-config.toml"),
            pool: crate::server::DbPool::default(),
            supervisor: Mutex::new(None),
            preserve_worker_on_settings_update: false,
            open_requests: Mutex::new(HashMap::new()),
            pending_approvals: Mutex::new(HashMap::new()),
            cancelled_requests: Mutex::new(HashSet::new()),
            approval_timeout: Duration::from_secs(5),
            providers: crate::providers::ProviderHandle::new(ProviderRegistry::empty()),
            running_tools: Mutex::new(HashMap::new()),
            policy: Arc::new(PolicyAssets::from_sources(TOOLS, SCHEMA, POLICIES).unwrap()),
        });

        start_execution(&state, &delivery, &execution).await;

        let started = rx_out.recv().await.unwrap();
        assert!(matches!(started, DaemonMessage::ToolCallStarted { .. }));

        let finished = rx_out.recv().await.unwrap();
        match finished {
            DaemonMessage::ToolCallFinished { result, .. } => {
                assert_eq!(result, "done");
            }
            other => panic!("expected ToolCallFinished, got {other:?}"),
        }
        assert_eq!(provider.0.load(Ordering::SeqCst), 1);
    }
}
