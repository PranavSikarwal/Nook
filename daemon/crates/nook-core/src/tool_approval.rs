use crate::authorization::{AuthorizationError, AuthorizationRequest};
use crate::policy::ScopedGrant;
use crate::policy_assets::approval_action_scope;
use crate::protocol::{
    AuthorizationDecision, DaemonMessage, DaemonWorkerRequest, ErrorCode, ErrorInfo,
};
use crate::server::{claim_pending_approval, AppState, PendingApprovalEntry};
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::mpsc;
use tracing::warn;
use uuid::Uuid;

pub const APPROVAL_TIMEOUT: Duration = Duration::from_secs(300);

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
}

type Verdict = (AuthorizationDecision, Option<String>);

fn deny(reason: impl Into<String>) -> Verdict {
    (AuthorizationDecision::Deny, Some(reason.into()))
}

fn argument_denial(error: &AuthorizationError) -> &'static str {
    match error {
        AuthorizationError::UnknownTool(_) => "Unknown tool",
        _ => "Invalid arguments",
    }
}

fn policy_failure(error: impl std::fmt::Display) -> Verdict {
    warn!("Authorization evaluation failed: {error}");
    deny("Authorization failed")
}

async fn send_result(
    state: &AppState,
    worker_request_id: Uuid,
    call_id: &str,
    verdict: Verdict,
) -> Result<(), String> {
    let supervisor = state.supervisor.lock().await.clone();
    let supervisor = supervisor.ok_or_else(|| "Worker supervisor is not available".to_string())?;
    let (decision, reason) = verdict;
    supervisor
        .send_request(DaemonWorkerRequest::AuthorizationResult {
            request_id: worker_request_id,
            call_id: call_id.to_string(),
            decision,
            reason,
        })
        .await
        .map(|_| ())
        .map_err(|error| error.to_string())
}

async fn reply_to_worker(
    state: &AppState,
    ctx: &AuthorizeContext,
    call_id: &str,
    verdict: Verdict,
) {
    if let Err(error) = send_result(state, ctx.worker_request_id, call_id, verdict).await {
        warn!("Failed to send authorization result for {call_id}: {error}");
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
        still_pending && approvals.remove(call_id).is_some()
    };
    if expired {
        let verdict = deny("Approval timed out");
        if let Err(error) = send_result(state, worker_request_id, call_id, verdict).await {
            warn!("Failed to deny timed out call {call_id}: {error}");
        }
    }
}

pub async fn handle_authorize_tool(
    state: &Arc<AppState>,
    ctx: &AuthorizeContext,
    call: AuthorizeToolCall,
    tx_out: &mpsc::Sender<DaemonMessage>,
) {
    let call_id = call.call_id.clone();
    if call.chat_id != ctx.chat_id {
        let verdict = deny("Request does not match the running chat");
        return reply_to_worker(state, ctx, &call_id, verdict).await;
    }
    let request = match AuthorizationRequest::normalize(
        &state.policy.catalog,
        ctx.chat_id,
        &call.call_id,
        &call.tool_name,
        &call.arguments,
    ) {
        Ok(request) => request,
        Err(error) => {
            warn!("Tool arguments rejected: {error}");
            let verdict = deny(argument_denial(&error));
            return reply_to_worker(state, ctx, &call_id, verdict).await;
        }
    };

    match state.policy.authorizer.authorize_tool(&request).await {
        Ok(true) => {
            let verdict = (AuthorizationDecision::Allow, None);
            return reply_to_worker(state, ctx, &call_id, verdict).await;
        }
        Ok(false) => {}
        Err(error) => {
            let verdict = policy_failure(error);
            return reply_to_worker(state, ctx, &call_id, verdict).await;
        }
    }

    if request.approval_tier == "deny" {
        return reply_to_worker(state, ctx, &call_id, deny("The tool is denied by policy")).await;
    }
    let spec = state.policy.catalog.tool(&request.tool_name);
    let (explanation, actions) = match spec {
        Some(spec) if !spec.approval.actions.is_empty() => {
            (spec.description.clone(), spec.approval.actions.clone())
        }
        _ => {
            let verdict = deny("No approval action is available for this tool");
            return reply_to_worker(state, ctx, &call_id, verdict).await;
        }
    };

    let resource_summary = request.resource_summary();
    {
        let mut approvals = state.pending_approvals.lock().await;
        if approvals.contains_key(&call_id) {
            drop(approvals);
            let verdict = deny("An approval for this call is already pending");
            return reply_to_worker(state, ctx, &call_id, verdict).await;
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
            },
        );
    }
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
        return reply_to_worker(state, ctx, &call_id, verdict).await;
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

    let verdict = resolve_decision(state, &entry, &action).await;
    match send_result(state, entry.worker_request_id, &call_id, verdict).await {
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
    let dropped: Vec<(String, Uuid)> = {
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
            .filter_map(|call_id| {
                approvals
                    .remove(&call_id)
                    .map(|entry| (call_id, entry.worker_request_id))
            })
            .collect()
    };
    for (call_id, worker_request_id) in dropped {
        if let Err(error) = send_result(state, worker_request_id, &call_id, deny(reason)).await {
            warn!("Failed to deny cancelled call {call_id}: {error}");
        }
    }
}
