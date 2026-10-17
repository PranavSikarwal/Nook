#![cfg(unix)]

mod common;

use common::{tool_result, Harness};
use nook_core::config::{FilesystemConfig, RootAccess, RootConfig, TerminalConfig, ToolsConfig};
use nook_core::protocol::DaemonMessage;
use nook_core::providers::terminal::TerminalProvider;
use nook_core::sandbox::Sandbox;
use serde_json::{json, Value};
use std::path::PathBuf;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tempfile::TempDir;
use tokio::net::TcpListener;
use uuid::Uuid;

const RUN: &str = "nook:run_command";

struct Area {
    _dir: TempDir,
    base: PathBuf,
}

impl Area {
    fn new() -> Self {
        let dir = tempfile::tempdir().unwrap();
        let base = dir.path().canonicalize().unwrap();
        for name in ["rw", "ro", "outside"] {
            std::fs::create_dir(base.join(name)).unwrap();
        }
        std::fs::write(base.join("rw/notes.txt"), "rw notes").unwrap();
        std::fs::write(base.join("ro/readme.txt"), "ro readme").unwrap();
        std::fs::write(base.join("outside/secret.txt"), "secret").unwrap();
        Self { _dir: dir, base }
    }

    fn path(&self, relative: &str) -> String {
        self.base.join(relative).display().to_string()
    }

    fn tools(&self, terminal: TerminalConfig) -> ToolsConfig {
        ToolsConfig {
            filesystem: FilesystemConfig {
                roots: vec![
                    RootConfig {
                        path: self.path("rw"),
                        access: RootAccess::ReadWrite,
                    },
                    RootConfig {
                        path: self.path("ro"),
                        access: RootAccess::Read,
                    },
                ],
                ..FilesystemConfig::default()
            },
            terminal,
            mcp_servers: vec![],
        }
    }
}

fn enabled() -> TerminalConfig {
    TerminalConfig {
        enabled: true,
        ..TerminalConfig::default()
    }
}

async fn harness(tools: &ToolsConfig) -> Option<Harness> {
    let sandbox = match Sandbox::detect().await {
        Ok(sandbox) => sandbox,
        Err(error) => {
            assert!(
                std::env::var_os("NOOK_REQUIRE_SANDBOX").is_none(),
                "The sandbox is required but unavailable: {error}"
            );
            eprintln!("Skipping terminal test: {error}");
            return None;
        }
    };
    let provider = TerminalProvider::new(tools, sandbox)?;
    Some(Harness::new(vec![Arc::new(provider)]).await)
}

fn argv(parts: &[&str]) -> String {
    json!({ "argv": parts }).to_string()
}

fn argv_in(parts: &[&str], cwd: &str) -> String {
    json!({ "argv": parts, "cwd": cwd }).to_string()
}

async fn run(h: &mut Harness, chat: Uuid, call: &str, arguments: &str) -> Value {
    h.approve_and_run(chat, call, RUN, arguments, "allow_once")
        .await
}

fn output(result: &Value) -> String {
    result["output"].as_str().unwrap().to_string()
}

#[tokio::test]
async fn the_tool_is_offered_only_when_enabled_and_a_folder_exists() {
    let area = Area::new();
    let off = area.tools(TerminalConfig::default());
    assert!(harness(&off).await.is_none());

    let mut no_roots = area.tools(enabled());
    no_roots.filesystem.roots.clear();
    assert!(harness(&no_roots).await.is_none());

    let Some(h) = harness(&area.tools(enabled())).await else {
        return;
    };
    assert!(h.state.providers.contains(RUN));
    let descriptor = h.state.providers.descriptor(RUN).unwrap();
    assert_eq!(descriptor.name, "run_command");
    assert!(descriptor.description.contains("cannot use the network"));
}

#[tokio::test]
async fn runs_a_command_by_name_and_reports_the_exit_code() {
    let area = Area::new();
    let Some(mut h) = harness(&area.tools(enabled())).await else {
        return;
    };

    let result = run(&mut h, Uuid::new_v4(), "c1", &argv(&["echo", "hi"])).await;

    assert_eq!(result["status"], "ok");
    assert_eq!(
        output(&result),
        "exit code: 0\n--- stdout ---\nhi\n--- stderr ---\n"
    );
}

#[tokio::test]
async fn a_failing_command_still_returns_its_exit_code_and_stderr() {
    let area = Area::new();
    let Some(mut h) = harness(&area.tools(enabled())).await else {
        return;
    };

    let result = run(
        &mut h,
        Uuid::new_v4(),
        "c1",
        &argv(&["sh", "-c", "echo oops >&2; exit 3"]),
    )
    .await;

    assert_eq!(result["status"], "ok");
    assert_eq!(
        output(&result),
        "exit code: 3\n--- stdout ---\n--- stderr ---\noops\n"
    );
}

#[tokio::test]
async fn the_approval_shows_the_command_line_and_offers_only_one_time_runs() {
    let area = Area::new();
    let Some(mut h) = harness(&area.tools(enabled())).await else {
        return;
    };
    let _call = h
        .execute(
            Uuid::new_v4(),
            "c1",
            RUN,
            &argv(&["echo", "hello world", "it's"]),
        )
        .await;

    let DaemonMessage::ApprovalRequested {
        resource_summary,
        actions,
        ..
    } = h.panel().await
    else {
        panic!("Expected an approval request");
    };

    assert_eq!(
        resource_summary,
        format!("Run `echo 'hello world' 'it'\\''s'` in {}", area.path("rw"))
    );
    let ids: Vec<&str> = actions.iter().map(|a| a.id.as_str()).collect();
    assert_eq!(ids, ["allow_once", "deny"]);
}

#[tokio::test]
async fn a_denied_command_never_runs() {
    let area = Area::new();
    let Some(mut h) = harness(&area.tools(enabled())).await else {
        return;
    };
    let chat = Uuid::new_v4();
    let marker = area.path("rw/ran.txt");
    let mut call = h.execute(chat, "c1", RUN, &argv(&["touch", &marker])).await;
    h.panel().await;

    h.decide(chat, "c1", "deny").await;

    assert_eq!(tool_result(&mut call).await["status"], "denied");
    assert!(!std::path::Path::new(&marker).exists());
}

#[tokio::test]
async fn every_command_asks_even_after_an_earlier_approval() {
    let area = Area::new();
    let Some(mut h) = harness(&area.tools(enabled())).await else {
        return;
    };
    let chat = Uuid::new_v4();
    run(&mut h, chat, "c1", &argv(&["echo", "one"])).await;

    let _again = h.execute(chat, "c2", RUN, &argv(&["echo", "one"])).await;

    assert!(matches!(
        h.panel().await,
        DaemonMessage::ApprovalRequested { .. }
    ));
}

#[tokio::test]
async fn runs_in_the_first_writable_folder_by_default_and_in_a_given_folder() {
    let area = Area::new();
    let Some(mut h) = harness(&area.tools(enabled())).await else {
        return;
    };
    let chat = Uuid::new_v4();

    let default = run(&mut h, chat, "c1", &argv(&["pwd"])).await;
    assert!(output(&default).contains(&area.path("rw")), "{default}");

    let given = run(&mut h, chat, "c2", &argv_in(&["pwd"], &area.path("ro"))).await;
    assert!(output(&given).contains(&area.path("ro")), "{given}");
}

#[tokio::test]
async fn a_folder_outside_the_allowed_ones_is_refused_before_any_prompt() {
    let area = Area::new();
    let Some(mut h) = harness(&area.tools(enabled())).await else {
        return;
    };

    let reason = h
        .expect_denied(
            Uuid::new_v4(),
            RUN,
            &argv_in(&["pwd"], &area.path("outside")),
        )
        .await;

    assert_eq!(reason, "Path is outside the allowed folders");
}

#[tokio::test]
async fn can_write_in_a_writable_folder_but_not_in_a_read_only_one() {
    let area = Area::new();
    let Some(mut h) = harness(&area.tools(enabled())).await else {
        return;
    };
    let chat = Uuid::new_v4();

    let ok = run(
        &mut h,
        chat,
        "c1",
        &argv(&["sh", "-c", "echo made > created.txt"]),
    )
    .await;
    assert!(output(&ok).starts_with("exit code: 0"), "{ok}");
    assert_eq!(
        std::fs::read_to_string(area.base.join("rw/created.txt")).unwrap(),
        "made\n"
    );

    let blocked = run(
        &mut h,
        chat,
        "c2",
        &argv_in(&["sh", "-c", "echo made > created.txt"], &area.path("ro")),
    )
    .await;
    assert!(!output(&blocked).starts_with("exit code: 0"), "{blocked}");
    assert!(!area.base.join("ro/created.txt").exists());
}

#[tokio::test]
async fn cannot_read_or_write_outside_the_folder_it_runs_in() {
    let area = Area::new();
    let Some(mut h) = harness(&area.tools(enabled())).await else {
        return;
    };
    let chat = Uuid::new_v4();

    let read = run(
        &mut h,
        chat,
        "c1",
        &argv(&["cat", &area.path("outside/secret.txt")]),
    )
    .await;
    assert!(!output(&read).contains("--- stdout ---\nsecret"), "{read}");
    assert!(!output(&read).starts_with("exit code: 0"));

    let other_root = run(
        &mut h,
        chat,
        "c2",
        &argv(&["cat", &area.path("ro/readme.txt")]),
    )
    .await;
    assert!(!output(&other_root).contains("ro readme"), "{other_root}");

    let write = run(
        &mut h,
        chat,
        "c3",
        &argv(&[
            "sh",
            "-c",
            &format!("echo x > '{}'", area.path("outside/new.txt")),
        ]),
    )
    .await;
    assert!(!output(&write).starts_with("exit code: 0"), "{write}");
    assert!(!area.base.join("outside/new.txt").exists());
}

#[tokio::test]
async fn the_environment_holds_only_the_fixed_variables() {
    let area = Area::new();
    std::env::set_var("NOOK_TEST_LEAK", "leaked");
    let Some(mut h) = harness(&area.tools(enabled())).await else {
        return;
    };

    let result = run(&mut h, Uuid::new_v4(), "c1", &argv(&["env"])).await;

    let text = output(&result);
    assert!(!text.contains("NOOK_TEST_LEAK"), "{text}");
    let mut names: Vec<&str> = text
        .lines()
        .filter_map(|line| line.split_once('=').map(|(name, _)| name))
        .collect();
    names.sort_unstable();
    for expected in ["HOME", "LANG", "NO_COLOR", "PATH", "TERM", "TMPDIR"] {
        assert!(names.contains(&expected), "{names:?}");
    }
    assert!(
        names.iter().all(|name| [
            "HOME", "LANG", "NO_COLOR", "PATH", "TERM", "TMPDIR", "PWD", "SHLVL", "_", "OLDPWD"
        ]
        .contains(name)),
        "{names:?}"
    );
}

#[tokio::test]
async fn each_run_gets_a_private_home_that_is_removed_afterwards() {
    let area = Area::new();
    let Some(mut h) = harness(&area.tools(enabled())).await else {
        return;
    };
    let chat = Uuid::new_v4();

    let result = run(
        &mut h,
        chat,
        "c1",
        &argv(&["sh", "-c", "echo data > \"$HOME/file\"; echo $HOME"]),
    )
    .await;

    let text = output(&result);
    let home = text.lines().nth(2).unwrap();
    assert!(home.starts_with('/'), "{text}");
    assert!(
        !std::path::Path::new(home).exists(),
        "{home} was not removed"
    );
}

#[tokio::test]
async fn the_network_is_off_by_default() {
    let area = Area::new();
    let Some(mut h) = harness(&area.tools(enabled())).await else {
        return;
    };
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let port = listener.local_addr().unwrap().port();
    let script = format!("exec 3<>/dev/tcp/127.0.0.1/{port}");

    let result = run(
        &mut h,
        Uuid::new_v4(),
        "c1",
        &argv(&["bash", "-c", &script]),
    )
    .await;

    assert!(!output(&result).starts_with("exit code: 0"), "{result}");
}

#[tokio::test]
async fn the_network_works_when_the_config_turns_it_on() {
    let area = Area::new();
    let terminal = TerminalConfig {
        network: true,
        ..enabled()
    };
    let Some(mut h) = harness(&area.tools(terminal)).await else {
        return;
    };
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let port = listener.local_addr().unwrap().port();
    let script = format!("exec 3<>/dev/tcp/127.0.0.1/{port}");
    let chat = Uuid::new_v4();
    let call = h
        .execute(chat, "c1", RUN, &argv(&["bash", "-c", &script]))
        .await;
    let DaemonMessage::ApprovalRequested {
        resource_summary, ..
    } = h.panel().await
    else {
        panic!("Expected an approval request");
    };
    assert!(
        resource_summary.ends_with("(network on)"),
        "{resource_summary}"
    );
    let mut call = call;
    h.decide(chat, "c1", "allow_once").await;
    h.drain_tool_events().await;

    let result = tool_result(&mut call).await;

    assert!(output(&result).starts_with("exit code: 0"), "{result}");
}

#[tokio::test]
async fn a_slow_command_is_stopped_at_the_time_limit() {
    let area = Area::new();
    let Some(mut h) = harness(&area.tools(enabled())).await else {
        return;
    };
    let arguments = json!({ "argv": ["sleep", "30"], "timeout_seconds": 1 }).to_string();
    let start = Instant::now();

    let result = run(&mut h, Uuid::new_v4(), "c1", &arguments).await;

    assert_eq!(result["status"], "error");
    assert!(
        output(&result).starts_with("timed out after 1 seconds"),
        "{result}"
    );
    assert!(
        start.elapsed() < Duration::from_secs(10),
        "{:?}",
        start.elapsed()
    );
}

#[tokio::test]
async fn output_is_cut_at_the_limit() {
    let area = Area::new();
    let terminal = TerminalConfig {
        max_output_bytes: 2048,
        ..enabled()
    };
    let Some(mut h) = harness(&area.tools(terminal)).await else {
        return;
    };

    let result = run(
        &mut h,
        Uuid::new_v4(),
        "c1",
        &argv(&["sh", "-c", "yes | head -c 500000"]),
    )
    .await;

    let text = output(&result);
    assert!(text.contains("[output truncated]"), "{}", text.len());
    assert!(text.len() < 4096, "{}", text.len());
}

#[tokio::test]
async fn programs_that_raise_privileges_are_closed_by_policy() {
    let area = Area::new();
    let Some(mut h) = harness(&area.tools(enabled())).await else {
        return;
    };
    let chat = Uuid::new_v4();

    for (index, program) in ["sudo", "/usr/bin/sudo", "su", "doas", "pkexec", "osascript"]
        .iter()
        .enumerate()
    {
        let call_id = format!("c{index}");
        let mut call = h
            .execute(chat, &call_id, RUN, &argv(&[program, "true"]))
            .await;
        h.panel().await;
        h.decide(chat, &call_id, "allow_once").await;
        let result = tool_result(&mut call).await;
        assert_eq!(result["status"], "denied", "{program}");
        assert_eq!(result["output"], "Policy does not allow this request");
    }
}

#[tokio::test]
async fn invalid_commands_are_refused_before_any_prompt() {
    let area = Area::new();
    let Some(mut h) = harness(&area.tools(enabled())).await else {
        return;
    };
    let chat = Uuid::new_v4();

    for arguments in [
        json!({}).to_string(),
        json!({ "argv": [] }).to_string(),
        json!({ "argv": "ls" }).to_string(),
        json!({ "argv": [1, 2] }).to_string(),
        json!({ "argv": ["ls"], "timeout_seconds": 0 }).to_string(),
        json!({ "argv": ["ls"], "timeout_seconds": 100000 }).to_string(),
        json!({ "argv": ["ls"], "extra": true }).to_string(),
    ] {
        let reason = h.expect_denied(chat, RUN, &arguments).await;
        assert_eq!(reason, "Invalid arguments", "{arguments}");
    }
    assert_eq!(
        h.expect_denied(chat, RUN, &json!({ "argv": [""] }).to_string())
            .await,
        "The command is empty"
    );
    assert_eq!(
        h.expect_denied(chat, RUN, &json!({ "argv": ["a\u{0}b"] }).to_string())
            .await,
        "A command argument is not valid"
    );
    let many: Vec<&str> = vec!["x"; 65];
    assert_eq!(
        h.expect_denied(chat, RUN, &json!({ "argv": many }).to_string())
            .await,
        "Invalid arguments"
    );
}

#[cfg(target_os = "macos")]
#[tokio::test]
async fn cancelling_kills_a_running_command_and_its_children() {
    let area = Area::new();
    let Some(mut h) = harness(&area.tools(enabled())).await else {
        return;
    };
    let chat = Uuid::new_v4();
    let pid_file = area.path("rw/child.pid");
    let script = format!("sleep 31 & echo $! > '{pid_file}'; wait");
    let mut call = h
        .execute(chat, "c1", RUN, &argv(&["sh", "-c", &script]))
        .await;
    h.panel().await;
    h.decide(chat, "c1", "allow_once").await;
    assert!(matches!(
        h.panel().await,
        DaemonMessage::ToolCallStarted { .. }
    ));
    for _ in 0..100 {
        if std::fs::read_to_string(&pid_file).is_ok_and(|text| !text.trim().is_empty()) {
            break;
        }
        tokio::time::sleep(Duration::from_millis(50)).await;
    }
    let pid: i32 = std::fs::read_to_string(&pid_file)
        .unwrap()
        .trim()
        .parse()
        .unwrap();

    nook_core::tool_approval::abort_running_tools(&h.state, call.client_id).await;

    let mut gone = false;
    for _ in 0..60 {
        if unsafe { libc::kill(pid, 0) } != 0 {
            gone = true;
            break;
        }
        tokio::time::sleep(Duration::from_millis(100)).await;
    }
    assert!(gone, "The child process is still running");
    common::assert_worker_silent(&mut call).await;
}
