use crate::authorization::AuthorizationRequest;
use crate::policy::ScopedGrant;
use crate::protocol::{
    AuthorizationDecision, DaemonMessage, DaemonWorkerRequest, ErrorCode, ErrorInfo,
};
use crate::server::{claim_pending_approval, AppState, PendingApprovalEntry};
use std::sync::Arc;
use tokio::sync::mpsc;
use tracing::warn;
use uuid::Uuid;

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

pub async fn handle_authorize_tool(
    state: &Arc<AppState>,
    ctx: &AuthorizeContext,
    call: AuthorizeToolCall,
    tx_out: &mpsc::Sender<DaemonMessage>,
) {
    let call_id = call.call_id.clone();
    if call.chat_id != ctx.chat_id {
        let verdict = deny("The authorization request does not belong to the running chat");
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
        Err(error) => return reply_to_worker(state, ctx, &call_id, deny(error.to_string())).await,
    };

    match state.policy.authorizer.authorize_tool(&request).await {
        Ok(true) => {
            let verdict = (AuthorizationDecision::Allow, None);
            return reply_to_worker(state, ctx, &call_id, verdict).await;
        }
        Ok(false) => {}
        Err(error) => {
            let verdict = deny(format!("Authorization failed: {error}"));
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
                offered_actions: actions.clone(),
            },
        );
    }
    let _ = tx_out
        .send(DaemonMessage::ApprovalRequested {
            id: ctx.client_request_id,
            chat_id: ctx.chat_id,
            message_id: call.message_id,
            call_id,
            tool_name: call.tool_name,
            arguments: call.arguments,
            explanation,
            resource_summary,
            actions,
        })
        .await;
}

fn scope_for_action(action: &str) -> Option<&'static str> {
    match action {
        "allow_once" => Some("once"),
        "allow_for_chat_host" => Some("chat_host"),
        "allow_for_chat" => Some("chat"),
        "always_allow" => Some("persistent"),
        _ => None,
    }
}

async fn resolve_decision(state: &AppState, entry: &PendingApprovalEntry, action: &str) -> Verdict {
    if action == "deny" {
        return deny("Denied by the user");
    }
    let Some(scope) = scope_for_action(action) else {
        return deny("Unsupported approval action");
    };
    let Some(grant) = ScopedGrant::for_request(&entry.request, scope) else {
        return deny("The approval action does not apply to this request");
    };
    let store = state.policy.authorizer.grant_store();
    store.lock().await.add_grant(grant.clone());
    match state.policy.authorizer.authorize_tool(&entry.request).await {
        Ok(true) => (AuthorizationDecision::Allow, None),
        Ok(false) => {
            store.lock().await.remove(&grant);
            deny("Policy does not allow this request")
        }
        Err(error) => {
            store.lock().await.remove(&grant);
            deny(format!("Authorization failed: {error}"))
        }
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
