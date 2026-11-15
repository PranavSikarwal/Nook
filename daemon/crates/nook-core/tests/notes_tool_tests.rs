#![cfg(unix)]

mod common;

use common::{authorization_result, tool_result, Harness};
use nook_core::config::Config;
use nook_core::protocol::DaemonMessage;
use serde_json::json;
use std::os::unix::fs::PermissionsExt;
use std::path::PathBuf;
use tempfile::TempDir;
use uuid::Uuid;

const CREATE_NOTE: &str = "nook:notes_create_note";
const DELETE_NOTE: &str = "nook:notes_delete_note";

struct Fixture {
    _dir: TempDir,
    program: PathBuf,
    log_file: PathBuf,
}

impl Fixture {
    fn new(response_stdout: &str) -> Self {
        let dir = tempfile::tempdir().unwrap();
        let log_file = dir.path().join("fake_osascript.log");
        let program = dir.path().join("fake_osascript.sh");
        let script = format!(
            "#!/bin/sh\nfor arg in \"$@\"; do printf '%s\\0' \"$arg\" >> \"{}\"; done\nprintf '%s' \"{}\"\n",
            log_file.display(),
            response_stdout
        );
        std::fs::write(&program, script).unwrap();
        std::fs::set_permissions(&program, std::fs::Permissions::from_mode(0o755)).unwrap();
        Self {
            _dir: dir,
            program,
            log_file,
        }
    }

    fn new_with_delay(response_stdout: &str, delay_secs: f64) -> Self {
        let dir = tempfile::tempdir().unwrap();
        let log_file = dir.path().join("fake_osascript.log");
        let program = dir.path().join("fake_osascript.sh");
        let script = format!(
            "#!/bin/sh\necho \"start $4\" >> \"{}\"\nsleep {}\necho \"end $4\" >> \"{}\"\nprintf '%s' \"{}\"\n",
            log_file.display(),
            delay_secs,
            log_file.display(),
            response_stdout
        );
        std::fs::write(&program, script).unwrap();
        std::fs::set_permissions(&program, std::fs::Permissions::from_mode(0o755)).unwrap();
        Self {
            _dir: dir,
            program,
            log_file,
        }
    }

    async fn harness(&self) -> Harness {
        Harness::with_config_and_notes(
            Config::default(),
            PathBuf::from("/tmp/nook-unused-config.toml"),
            Some(self.program.clone()),
        )
        .await
    }

    fn logged_args(&self) -> Vec<String> {
        if !self.log_file.exists() {
            return Vec::new();
        }
        let bytes = std::fs::read(&self.log_file).unwrap();
        bytes
            .split(|&b| b == 0)
            .filter(|slice| !slice.is_empty())
            .map(|slice| String::from_utf8_lossy(slice).into_owned())
            .collect()
    }

    fn logged_lines(&self) -> Vec<String> {
        if !self.log_file.exists() {
            return Vec::new();
        }
        std::fs::read_to_string(&self.log_file)
            .unwrap()
            .lines()
            .map(String::from)
            .collect()
    }
}

#[tokio::test]
async fn the_program_runs_only_after_an_allow() {
    let note_id = "x-coredata://12345678-1234-1234-1234-1234567890ab/ICNote/p1";
    let fixture = Fixture::new(&format!("{note_id}\n"));
    let mut h = fixture.harness().await;
    let chat = Uuid::new_v4();
    let args = json!({
        "title": "Meeting Notes",
        "body": "Discuss project plan"
    })
    .to_string();

    let mut call = h.execute(chat, "c1", CREATE_NOTE, &args).await;

    let panel_msg = h.panel().await;
    assert!(matches!(panel_msg, DaemonMessage::ApprovalRequested { .. }));
    assert!(
        fixture.logged_args().is_empty(),
        "fake program must not run before approval"
    );

    h.decide(chat, "c1", "allow_once").await;
    h.drain_tool_events().await;

    let result = tool_result(&mut call).await;
    assert_eq!(result["status"], "ok");
    assert_eq!(
        result["output"],
        format!(
            "Created the note \"Meeting Notes\" in the Notes folder \"Nook\". Its id is {note_id}."
        )
    );

    let logged = fixture.logged_args();
    assert_eq!(
        logged.len(),
        4,
        "expected exactly 4 arguments: -e, script, folder, html"
    );
    assert_eq!(logged[0], "-e");
    assert!(logged[1].contains("tell application \"Notes\""));
    assert_eq!(logged[2], ":Nook");
    assert_eq!(
        logged[3],
        ":<div>Meeting Notes</div><div>Discuss project plan</div>"
    );
}

#[tokio::test]
async fn a_deny_leaves_the_file_empty() {
    let fixture = Fixture::new("ok\n");
    let mut h = fixture.harness().await;
    let chat = Uuid::new_v4();
    let args = json!({
        "title": "Private Note",
        "body": "Secret content"
    })
    .to_string();

    let mut call = h.execute(chat, "c1", CREATE_NOTE, &args).await;

    let panel_msg = h.panel().await;
    assert!(matches!(panel_msg, DaemonMessage::ApprovalRequested { .. }));

    h.decide(chat, "c1", "deny").await;

    let result = tool_result(&mut call).await;
    assert_eq!(result["status"], "denied");
    assert!(
        fixture.logged_args().is_empty(),
        "fake program must never run when denied"
    );
    h.assert_panel_silent();
}

#[tokio::test]
async fn a_cancel_before_the_approval_does_not_run_it() {
    let fixture = Fixture::new("ok\n");
    let mut h = fixture.harness().await;
    let chat = Uuid::new_v4();
    let args = json!({
        "title": "Cancelled Note",
        "body": "Should not be created"
    })
    .to_string();

    let mut call = h.execute(chat, "c1", CREATE_NOTE, &args).await;

    let panel_msg = h.panel().await;
    assert!(matches!(panel_msg, DaemonMessage::ApprovalRequested { .. }));

    h.cancel(call.client_id).await;

    let result = tool_result(&mut call).await;
    assert_eq!(result["status"], "denied");
    assert_eq!(result["output"], "Cancelled");
    assert!(
        fixture.logged_args().is_empty(),
        "fake program must not run after cancel"
    );

    // A late approval decision must not run the tool
    h.decide(chat, "c1", "allow_once").await;
    common::assert_worker_silent(&mut call).await;
    assert!(
        fixture.logged_args().is_empty(),
        "fake program must still not run after late approval"
    );
}

#[tokio::test]
async fn a_call_with_a_registry_built_with_none_is_denied_with_tool_runs_in_the_worker() {
    let mut h = Harness::with_config_and_notes(
        Config::default(),
        PathBuf::from("/tmp/nook-unused-config.toml"),
        None,
    )
    .await;
    let chat = Uuid::new_v4();
    let args = json!({
        "title": "Should fail",
        "body": "Not supported on this platform"
    })
    .to_string();

    let output = h.expect_denied(chat, CREATE_NOTE, &args).await;
    assert_eq!(output, "Tool runs in the Worker");
}

#[tokio::test]
async fn the_delete_approval_has_no_chat_button() {
    let fixture = Fixture::new("ok\n");
    let mut h = fixture.harness().await;
    let chat = Uuid::new_v4();
    let delete_args = json!({
        "note_id": "x-coredata://12345678-1234-1234-1234-1234567890ab/ICNote/p1",
        "note_title": "Deprecated Note"
    })
    .to_string();

    let mut call = h.execute(chat, "c1", DELETE_NOTE, &delete_args).await;

    let panel_msg = h.panel().await;
    let DaemonMessage::ApprovalRequested {
        actions,
        resource_summary,
        ..
    } = panel_msg
    else {
        panic!("expected ApprovalRequested, got {panel_msg:?}");
    };

    assert_eq!(
        resource_summary,
        "Delete note \"Deprecated Note\" in \"Nook\""
    );
    let action_ids: Vec<&str> = actions.iter().map(|a| a.id.as_str()).collect();
    assert_eq!(
        action_ids,
        ["allow_once", "deny"],
        "delete approval must offer only allow_once and deny"
    );
    assert!(
        !action_ids.contains(&"allow_for_chat"),
        "delete approval must not have allow_for_chat"
    );

    h.decide(chat, "c1", "deny").await;
    let result = tool_result(&mut call).await;
    assert_eq!(result["status"], "denied");
}

#[tokio::test]
async fn two_parallel_calls_never_overlap() {
    let note_id = "x-coredata://12345678-1234-1234-1234-1234567890ab/ICNote/p1";
    let fixture = Fixture::new_with_delay(&format!("{note_id}\n"), 0.08);
    let mut h = fixture.harness().await;
    let chat1 = Uuid::new_v4();
    let chat2 = Uuid::new_v4();
    let args1 = json!({ "title": "Note 1", "body": "call_one" }).to_string();
    let args2 = json!({ "title": "Note 2", "body": "call_two" }).to_string();

    let mut call1 = h.execute(chat1, "c1", CREATE_NOTE, &args1).await;
    let mut call2 = h.execute(chat2, "c2", CREATE_NOTE, &args2).await;

    let p1 = h.panel().await;
    let p2 = h.panel().await;
    assert!(matches!(p1, DaemonMessage::ApprovalRequested { .. }));
    assert!(matches!(p2, DaemonMessage::ApprovalRequested { .. }));

    h.decide(chat1, "c1", "allow_once").await;
    h.decide(chat2, "c2", "allow_once").await;

    let (res1, res2) = tokio::join!(tool_result(&mut call1), tool_result(&mut call2));
    assert_eq!(res1["status"], "ok");
    assert_eq!(res2["status"], "ok");

    let lines = fixture.logged_lines();
    assert_eq!(
        lines.len(),
        4,
        "expected 4 log lines: start, end, start, end; got {lines:?}"
    );
    assert!(lines[0].starts_with("start "));
    assert!(lines[1].starts_with("end "));
    assert!(lines[2].starts_with("start "));
    assert!(lines[3].starts_with("end "));
}

#[tokio::test]
async fn two_parallel_calls_never_overlap_also_across_a_rebuilt() {
    let note_id = "x-coredata://12345678-1234-1234-1234-1234567890ab/ICNote/p1";
    let fixture = Fixture::new_with_delay(&format!("{note_id}\n"), 0.08);
    let mut h = fixture.harness().await;
    let chat1 = Uuid::new_v4();
    let chat2 = Uuid::new_v4();
    let args1 = json!({ "title": "Note 1", "body": "call_before_rebuilt" }).to_string();
    let args2 = json!({ "title": "Note 2", "body": "call_after_rebuilt" }).to_string();

    let mut call1 = h.execute(chat1, "c1", CREATE_NOTE, &args1).await;
    let p1 = h.panel().await;
    assert!(matches!(p1, DaemonMessage::ApprovalRequested { .. }));

    // Start running call1
    h.decide(chat1, "c1", "allow_once").await;
    let started1 = h.panel().await;
    assert!(matches!(started1, DaemonMessage::ToolCallStarted { .. }));

    // While call1 is running, rebuild and replace the registry
    let tools = h.state.config.read().await.tools.clone();
    let rebuilt = h.state.providers.current().rebuilt(&tools);
    h.state.providers.replace(rebuilt);

    // Now execute and approve call2 on the rebuilt registry
    let mut call2 = h.execute(chat2, "c2", CREATE_NOTE, &args2).await;
    let p2 = h.panel().await;
    assert!(matches!(p2, DaemonMessage::ApprovalRequested { .. }));
    h.decide(chat2, "c2", "allow_once").await;

    let (res1, res2) = tokio::join!(tool_result(&mut call1), tool_result(&mut call2));
    assert_eq!(res1["status"], "ok");
    assert_eq!(res2["status"], "ok");

    let lines = fixture.logged_lines();
    assert_eq!(
        lines.len(),
        4,
        "expected 4 log lines across rebuilt; got {lines:?}"
    );
    assert!(lines[0].starts_with("start "));
    assert!(lines[1].starts_with("end "));
    assert!(lines[2].starts_with("start "));
    assert!(lines[3].starts_with("end "));
}

#[tokio::test]
async fn a_folder_change_while_a_decision_waits_denies_the_call_and_records_no_grant() {
    let note_id = "x-coredata://12345678-1234-1234-1234-1234567890ab/ICNote/p1";
    let fixture = Fixture::new(&format!("{note_id}\n"));
    let mut h = fixture.harness().await;
    let chat = Uuid::new_v4();
    let args = serde_json::json!({
        "title": "Meeting Notes",
        "body": "Discuss project plan"
    })
    .to_string();

    let mut call = h.execute(chat, "c1", CREATE_NOTE, &args).await;
    let panel_msg = h.panel().await;
    assert!(matches!(panel_msg, DaemonMessage::ApprovalRequested { .. }));

    // Hold state.config.write()
    let mut cfg_guard = h.state.config.write().await;

    let state_clone = h.state.clone();
    let panel_tx = h.panel_tx.clone();
    let decision_task = tokio::spawn(async move {
        nook_core::tool_approval::handle_approval_decision(
            &state_clone,
            Uuid::new_v4(),
            chat,
            "c1".to_string(),
            "allow_for_chat".to_string(),
            &panel_tx,
        )
        .await;
    });

    // Wait until the decision has claimed the pending approval (decision_in_flight is true)
    let mut reached = false;
    for _ in 0..100 {
        tokio::task::yield_now().await;
        let in_flight = {
            let approvals = h.state.pending_approvals.lock().await;
            approvals
                .get("c1")
                .map(|e| e.decision_in_flight)
                .unwrap_or(false)
        };
        if in_flight {
            reached = true;
            break;
        }
    }
    assert!(reached, "decision_in_flight was not reached");
    assert!(
        !decision_task.is_finished(),
        "decision task finished while guard held"
    );

    // Change the folder in memory under the write lock
    cfg_guard.tools.notes.folder = "Work".to_string();
    nook_core::notes_folder::apply_folder_change(&h.state, &mut cfg_guard).await;

    // Release the write lock
    drop(cfg_guard);

    decision_task.await.unwrap();

    // The call must be denied with "The Notes folder changed"
    let result = tool_result(&mut call).await;
    assert_eq!(result["status"], "denied");
    assert_eq!(result["output"], "The Notes folder changed");

    // No grant was recorded
    let store_arc = h.state.policy.authorizer.grant_store();
    let grant_store = store_arc.lock().await;
    assert!(
        grant_store
            .matching_scopes(chat, CREATE_NOTE, None, None, None)
            .is_empty(),
        "grant must not be recorded on folder mismatch"
    );

    // Fake program never ran
    assert!(fixture.logged_args().is_empty());
}

#[tokio::test]
async fn a_decision_before_a_folder_change_runs_in_the_old_folder() {
    let note_id = "x-coredata://12345678-1234-1234-1234-1234567890ab/ICNote/p1";
    let fixture = Fixture::new(&format!("{note_id}\n"));
    let mut h = fixture.harness().await;
    let chat = Uuid::new_v4();
    let args = serde_json::json!({
        "title": "Old Note",
        "body": "Content in old folder"
    })
    .to_string();

    let mut call = h.execute(chat, "c1", CREATE_NOTE, &args).await;
    let panel_msg = h.panel().await;
    assert!(matches!(panel_msg, DaemonMessage::ApprovalRequested { .. }));

    // Decision finishes first
    h.decide(chat, "c1", "allow_for_chat").await;
    h.drain_tool_events().await;

    // Then folder change runs
    {
        let mut cfg_guard = h.state.config.write().await;
        cfg_guard.tools.notes.folder = "Work".to_string();
        nook_core::notes_folder::apply_folder_change(&h.state, &mut cfg_guard).await;
    }

    let result = tool_result(&mut call).await;
    assert_eq!(result["status"], "ok");
    assert!(
        result["output"]
            .as_str()
            .unwrap()
            .contains("in the Notes folder \"Nook\""),
        "tool result must name the old folder Nook: {}",
        result["output"]
    );

    let logged = fixture.logged_args();
    assert_eq!(logged[2], ":Nook", "fake program must receive old folder");

    assert!(
        h.state
            .policy
            .authorizer
            .grant_store()
            .lock()
            .await
            .matching_scopes(chat, "notes_create_note", None, None, None)
            .is_empty(),
        "chat grant for old folder must be dropped"
    );
}

#[tokio::test]
async fn a_folder_change_prevents_a_chat_grant_from_running_in_the_new_folder() {
    let note_id = "x-coredata://12345678-1234-1234-1234-1234567890ab/ICNote/p1";
    let fixture = Fixture::new(&format!("{note_id}\n"));
    let mut h = fixture.harness().await;
    let chat = Uuid::new_v4();
    let args1 = serde_json::json!({
        "title": "First Note",
        "body": "First content"
    })
    .to_string();

    // Run call 1 with allow_for_chat to establish chat grant
    let res1 = h
        .approve_and_run(chat, "c1", CREATE_NOTE, &args1, "allow_for_chat")
        .await;
    assert_eq!(res1["status"], "ok");

    let store_arc = h.state.policy.authorizer.grant_store();
    assert!(
        !store_arc
            .lock()
            .await
            .matching_scopes(chat, "notes_create_note", None, None, None)
            .is_empty(),
        "chat grant should exist"
    );

    // Now test the race: write lock held while call 2 is started
    let mut cfg_guard = h.state.config.write().await;

    let args2 = serde_json::json!({
        "title": "Second Note",
        "body": "Second content"
    })
    .to_string();

    // Spawn call 2 - handle_authorize_tool will take state.config.read() and wait
    let state_clone = h.state.clone();
    let panel_tx = h.panel_tx.clone();
    let client_id = Uuid::new_v4();
    let worker_id = Uuid::new_v4();
    let call_task = tokio::spawn(async move {
        nook_core::tool_approval::handle_authorize_tool(
            &state_clone,
            &nook_core::tool_approval::AuthorizeContext {
                client_request_id: client_id,
                worker_request_id: worker_id,
                chat_id: chat,
            },
            nook_core::tool_approval::AuthorizeToolCall {
                message_id: Uuid::new_v4(),
                call_id: "c2".to_string(),
                chat_id: chat,
                tool_name: CREATE_NOTE.to_string(),
                arguments: args2,
                execute: true,
            },
            &panel_tx,
        )
        .await;
    });

    for _ in 0..10 {
        tokio::task::yield_now().await;
    }
    assert!(
        !call_task.is_finished(),
        "call task finished while guard held"
    );

    // Folder change happens under the write lock
    cfg_guard.tools.notes.folder = "Work".to_string();
    nook_core::notes_folder::apply_folder_change(&h.state, &mut cfg_guard).await;

    // Release write lock
    drop(cfg_guard);

    call_task.await.unwrap();

    // Because chat grant was dropped and folder changed, call 2 MUST NOT run automatically.
    // It must show an approval request on the panel for the new folder.
    let panel_msg = h.panel().await;
    match panel_msg {
        DaemonMessage::ApprovalRequested {
            call_id, tool_name, ..
        } => {
            assert_eq!(call_id, "c2");
            assert_eq!(tool_name, "nook:notes_create_note");
        }
        other => panic!("expected ApprovalRequested for new folder, got {other:?}"),
    }

    // Deny call 2
    h.decide(chat, "c2", "deny").await;
    h.assert_panel_silent();
}

#[tokio::test]
async fn a_folder_change_leaves_an_approval_and_a_grant_of_another_tool_intact() {
    let note_id = "x-coredata://12345678-1234-1234-1234-1234567890ab/ICNote/p1";
    let fixture = Fixture::new(&format!("{note_id}\n"));
    let mut h = fixture.harness().await;
    let chat = Uuid::new_v4();

    // Start another tool's call: nook:web_fetch
    let mut other_call = h
        .authorize(
            chat,
            "fetch1",
            "nook:web_fetch",
            r#"{"url":"https://example.com"}"#,
        )
        .await;

    let panel_msg = h.panel().await;
    assert!(matches!(panel_msg, DaemonMessage::ApprovalRequested { .. }));

    // Add a grant for another tool in grant_store
    let other_grant = nook_core::policy::ScopedGrant {
        chat_id: chat,
        tool_name: "web_fetch".to_string(),
        scope: "chat".to_string(),
        host: Some("example.org".to_string()),
        path: None,
        argument_digest: None,
    };
    h.state
        .policy
        .authorizer
        .grant_store()
        .lock()
        .await
        .add_grant(other_grant);

    // Create a Notes chat grant through the real flow (allow_for_chat)
    let res = h
        .approve_and_run(
            chat,
            "c_notes",
            CREATE_NOTE,
            r#"{"title":"Note 1","body":"body 1"}"#,
            "allow_for_chat",
        )
        .await;
    assert_eq!(res["status"], "ok");

    // Perform the Notes folder change
    {
        let mut cfg_guard = h.state.config.write().await;
        cfg_guard.tools.notes.folder = "Work".to_string();
        nook_core::notes_folder::apply_folder_change(&h.state, &mut cfg_guard).await;
    }

    // The pending approval for fetch1 must STILL be pending
    {
        let approvals = h.state.pending_approvals.lock().await;
        assert!(
            approvals.contains_key("fetch1"),
            "pending approval for another tool must stay"
        );
    }

    // The grant for web_fetch must STILL be present
    {
        let store_arc = h.state.policy.authorizer.grant_store();
        let store = store_arc.lock().await;
        assert!(
            !store
                .matching_scopes(chat, "web_fetch", Some("example.org"), None, None)
                .is_empty(),
            "grant for another tool must stay"
        );
        assert!(
            store
                .matching_scopes(chat, "notes_create_note", None, None, None)
                .is_empty(),
            "chat grant for notes tool must be dropped"
        );
    }

    // Decide allow_once on fetch1 - it must succeed
    h.decide(chat, "fetch1", "allow_once").await;
    let (decision, _) = authorization_result(&mut other_call).await;
    assert_eq!(decision, "allow");
}
