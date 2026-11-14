#![cfg(unix)]

mod common;

use common::{tool_result, Harness};
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
