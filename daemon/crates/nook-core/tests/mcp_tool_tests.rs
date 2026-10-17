#![cfg(unix)]

mod common;

use common::{tool_result, Harness};
use nook_core::config::McpServerConfig;
use nook_core::protocol::DaemonMessage;
use nook_core::protocol::ToolSourceState;
use nook_core::providers::mcp::{McpProvider, StartupLimits};
use nook_core::providers::Provider;
use nook_core::sandbox::Sandbox;
use serde_json::{json, Value};
use std::collections::BTreeMap;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::{Duration, Instant};
use uuid::Uuid;

const ECHO: &str = "mcp:fake:echo";

fn example_binary() -> PathBuf {
    let exe = std::env::current_exe().unwrap();
    exe.parent()
        .unwrap()
        .parent()
        .unwrap()
        .join("examples")
        .join("fake_mcp_server")
}

fn server(name: &str, mode: &str) -> McpServerConfig {
    let binary = example_binary();
    McpServerConfig {
        name: name.to_string(),
        command: binary.display().to_string(),
        args: vec![mode.to_string()],
        env: BTreeMap::new(),
        network: false,
        read_paths: vec![binary.parent().unwrap().display().to_string()],
        write_paths: vec![],
        timeout_seconds: 60,
    }
}

async fn sandbox() -> Option<Sandbox> {
    match Sandbox::detect().await {
        Ok(sandbox) => Some(sandbox),
        Err(error) => {
            assert!(
                std::env::var_os("NOOK_REQUIRE_SANDBOX").is_none(),
                "The sandbox is required but unavailable: {error}"
            );
            eprintln!("Skipping MCP test: {error}");
            None
        }
    }
}

fn quick() -> StartupLimits {
    StartupLimits {
        initialize: Duration::from_secs(2),
        list_tools: Duration::from_secs(2),
        restart_interval: Duration::from_secs(3600),
    }
}

async fn provider(configs: &[McpServerConfig], limits: StartupLimits) -> Option<McpProvider> {
    let sandbox = sandbox().await?;
    McpProvider::start(configs, &sandbox, limits).await
}

async fn harness(configs: &[McpServerConfig]) -> Option<Harness> {
    harness_with(configs, quick()).await
}

async fn harness_with(configs: &[McpServerConfig], limits: StartupLimits) -> Option<Harness> {
    let provider = provider(configs, limits).await?;
    Some(Harness::new(vec![Arc::new(provider)]).await)
}

fn names(h: &Harness) -> Vec<String> {
    let mut names: Vec<String> = h
        .state
        .providers
        .descriptors()
        .iter()
        .map(|d| d.name.clone())
        .collect();
    names.sort();
    names
}

async fn run(h: &mut Harness, chat: Uuid, call: &str, tool: &str, args: Value) -> Value {
    h.approve_and_run(chat, call, tool, &args.to_string(), "allow_once")
        .await
}

#[tokio::test]
async fn only_valid_tools_are_offered_with_clean_names() {
    let Some(h) = harness(&[server("fake", "normal")]).await else {
        return;
    };

    assert_eq!(
        names(&h),
        [
            "mcp__fake__big",
            "mcp__fake__die",
            "mcp__fake__dup_name",
            "mcp__fake__echo",
            "mcp__fake__env",
            "mcp__fake__fail",
            "mcp__fake__mixed",
            "mcp__fake__plain",
            "mcp__fake__read_file",
            "mcp__fake__slow",
            "mcp__fake__structured",
        ]
    );
    let echo = h.state.providers.descriptor(ECHO).unwrap();
    assert_eq!(echo.description, "Echo the text back. (MCP server: fake)");
    let plain = h.state.providers.descriptor("mcp:fake:plain").unwrap();
    assert_eq!(plain.description, "plain (MCP server: fake)");
    assert!(h.state.providers.contains("mcp:fake:dup.name"));
    assert!(!h.state.providers.contains("mcp:fake:dup_name"));
    for skipped in ["bad_schema", "no_schema", "has:colon"] {
        assert!(!h.state.providers.contains(&format!("mcp:fake:{skipped}")));
    }
}

#[tokio::test]
async fn a_call_asks_first_and_returns_the_server_text() {
    let Some(mut h) = harness(&[server("fake", "normal")]).await else {
        return;
    };
    let chat = Uuid::new_v4();
    let _call = h
        .execute(chat, "c1", ECHO, &json!({ "text": "hi" }).to_string())
        .await;

    let DaemonMessage::ApprovalRequested {
        resource_summary,
        actions,
        tool_name,
        ..
    } = h.panel().await
    else {
        panic!("Expected an approval request");
    };

    assert_eq!(tool_name, ECHO);
    assert_eq!(
        resource_summary,
        "Call echo on MCP server fake with {\"text\":\"hi\"}"
    );
    let ids: Vec<&str> = actions.iter().map(|a| a.id.as_str()).collect();
    assert_eq!(
        ids,
        ["allow_once", "allow_for_chat", "always_allow", "deny"]
    );
}

#[tokio::test]
async fn an_approved_call_returns_the_answer() {
    let Some(mut h) = harness(&[server("fake", "normal")]).await else {
        return;
    };

    let result = run(&mut h, Uuid::new_v4(), "c1", ECHO, json!({ "text": "hi" })).await;

    assert_eq!(result["status"], "ok");
    assert_eq!(result["output"], "echo: hi");
}

#[tokio::test]
async fn a_denied_call_never_reaches_the_server() {
    let Some(mut h) = harness(&[server("fake", "normal")]).await else {
        return;
    };
    let chat = Uuid::new_v4();
    let mut call = h
        .execute(chat, "c1", ECHO, &json!({ "text": "hi" }).to_string())
        .await;
    h.panel().await;

    h.decide(chat, "c1", "deny").await;

    let result = tool_result(&mut call).await;
    assert_eq!(result["status"], "denied");
    assert_eq!(result["output"], "Denied by the user");
    h.assert_panel_silent();
}

#[tokio::test]
async fn a_chat_grant_covers_one_tool_in_one_chat() {
    let Some(mut h) = harness(&[server("fake", "normal")]).await else {
        return;
    };
    let chat = Uuid::new_v4();
    let granted = h
        .approve_and_run(
            chat,
            "c1",
            ECHO,
            &json!({ "text": "a" }).to_string(),
            "allow_for_chat",
        )
        .await;
    assert_eq!(granted["status"], "ok");

    let mut again = h
        .execute(chat, "c2", ECHO, &json!({ "text": "b" }).to_string())
        .await;
    h.drain_tool_events().await;
    assert_eq!(tool_result(&mut again).await["output"], "echo: b");

    let _other_tool = h.execute(chat, "c3", "mcp:fake:fail", "{}").await;
    assert!(matches!(
        h.panel().await,
        DaemonMessage::ApprovalRequested { .. }
    ));
    let _other_chat = h
        .execute(
            Uuid::new_v4(),
            "c4",
            ECHO,
            &json!({ "text": "c" }).to_string(),
        )
        .await;
    assert!(matches!(
        h.panel().await,
        DaemonMessage::ApprovalRequested { .. }
    ));
}

#[tokio::test]
async fn arguments_that_break_the_server_schema_are_refused_before_any_prompt() {
    let Some(mut h) = harness(&[server("fake", "normal")]).await else {
        return;
    };
    let chat = Uuid::new_v4();

    for arguments in [
        "{}",
        r#"{"text":5}"#,
        r#"{"text":"a","extra":1}"#,
        "[]",
        "not json",
    ] {
        assert_eq!(
            h.expect_denied(chat, ECHO, arguments).await,
            "Invalid arguments",
            "{arguments}"
        );
    }
}

#[tokio::test]
async fn an_error_from_the_server_comes_back_as_an_error_result() {
    let Some(mut h) = harness(&[server("fake", "normal")]).await else {
        return;
    };

    let result = run(&mut h, Uuid::new_v4(), "c1", "mcp:fake:fail", json!({})).await;

    assert_eq!(result["status"], "error");
    assert_eq!(result["output"], "Error: something broke");
}

#[tokio::test]
async fn content_and_structured_answers_become_text() {
    let Some(mut h) = harness(&[server("fake", "normal")]).await else {
        return;
    };
    let chat = Uuid::new_v4();

    let mixed = run(&mut h, chat, "c1", "mcp:fake:mixed", json!({})).await;
    assert_eq!(
        mixed["output"],
        "first\n[image omitted]\nresource text\n[resource omitted]\n[resource link: file:///c]\nlast"
    );

    let structured = run(&mut h, chat, "c2", "mcp:fake:structured", json!({})).await;
    assert_eq!(structured["output"], "{\"answer\":42}");
}

#[tokio::test]
async fn a_huge_answer_is_cut() {
    let Some(mut h) = harness(&[server("fake", "normal")]).await else {
        return;
    };

    let result = run(&mut h, Uuid::new_v4(), "c1", "mcp:fake:big", json!({})).await;

    let output = result["output"].as_str().unwrap();
    assert!(output.len() < 70_000);
    assert!(output.ends_with("[output truncated]"));
}

#[tokio::test]
async fn a_slow_call_times_out_with_a_clear_error() {
    let mut config = server("fake", "normal");
    config.timeout_seconds = 1;
    let Some(mut h) = harness(&[config]).await else {
        return;
    };
    let chat = Uuid::new_v4();
    let start = Instant::now();

    let slow = run(&mut h, chat, "c1", "mcp:fake:slow", json!({})).await;

    assert_eq!(slow["status"], "error");
    assert_eq!(
        slow["output"],
        "Error: the MCP server did not answer in time"
    );
    assert!(start.elapsed() < Duration::from_secs(10));
}

#[tokio::test]
async fn the_server_cannot_read_a_file_outside_its_read_paths() {
    let secret_dir = tempfile::tempdir().unwrap();
    let secret = secret_dir.path().canonicalize().unwrap().join("secret.txt");
    std::fs::write(&secret, "top secret").unwrap();
    let readable_dir = tempfile::tempdir().unwrap();
    let readable = readable_dir
        .path()
        .canonicalize()
        .unwrap()
        .join("shared.txt");
    std::fs::write(&readable, "shared text").unwrap();
    let mut config = server("fake", "normal");
    config
        .read_paths
        .push(readable_dir.path().display().to_string());
    let Some(mut h) = harness(&[config]).await else {
        return;
    };
    let chat = Uuid::new_v4();

    let hidden = run(
        &mut h,
        chat,
        "c1",
        "mcp:fake:read_file",
        json!({ "path": secret.display().to_string() }),
    )
    .await;
    assert_eq!(hidden["status"], "error");
    assert!(!hidden["output"].as_str().unwrap().contains("top secret"));

    let shared = run(
        &mut h,
        chat,
        "c2",
        "mcp:fake:read_file",
        json!({ "path": readable.display().to_string() }),
    )
    .await;
    assert_eq!(shared["output"], "shared text");
}

#[tokio::test]
async fn the_server_environment_holds_only_fixed_and_configured_variables() {
    std::env::set_var("NOOK_TEST_LEAK", "leaked");
    let mut config = server("fake", "normal");
    config
        .env
        .insert("FAKE_TOKEN".to_string(), "token".to_string());
    let Some(mut h) = harness(&[config]).await else {
        return;
    };

    let result = run(&mut h, Uuid::new_v4(), "c1", "mcp:fake:env", json!({})).await;

    let text = result["output"].as_str().unwrap();
    let names: Vec<&str> = text.split(',').collect();
    for expected in ["FAKE_TOKEN", "HOME", "LANG", "PATH", "TMPDIR"] {
        assert!(names.contains(&expected), "{text}");
    }
    assert!(!text.contains("NOOK_TEST_LEAK"), "{text}");
}

#[tokio::test]
async fn a_server_that_stops_makes_later_calls_fail_cleanly() {
    let Some(mut h) = harness(&[server("fake", "crash")]).await else {
        return;
    };
    tokio::time::sleep(Duration::from_millis(300)).await;

    let result = run(&mut h, Uuid::new_v4(), "c1", ECHO, json!({ "text": "hi" })).await;

    assert_eq!(result["status"], "error");
    assert_eq!(result["output"], "Error: the MCP server stopped");
}

#[tokio::test]
async fn a_server_that_died_is_started_again_by_the_next_call() {
    let limits = StartupLimits {
        restart_interval: Duration::ZERO,
        ..quick()
    };
    let Some(mut h) = harness_with(&[server("fake", "normal")], limits).await else {
        return;
    };

    let died = run(&mut h, Uuid::new_v4(), "c1", "mcp:fake:die", json!({})).await;
    assert_eq!(died["status"], "error");
    assert_eq!(died["output"], "Error: the MCP server stopped");

    let chat = Uuid::new_v4();
    let again = run(&mut h, chat, "c2", ECHO, json!({ "text": "hi" })).await;
    assert_eq!(again["status"], "ok");
    assert_eq!(again["output"], "echo: hi");
}

#[tokio::test]
async fn a_server_that_died_soon_after_a_start_stays_stopped() {
    let Some(mut h) = harness(&[server("fake", "normal")]).await else {
        return;
    };

    run(&mut h, Uuid::new_v4(), "c1", "mcp:fake:die", json!({})).await;
    let again = run(&mut h, Uuid::new_v4(), "c2", ECHO, json!({ "text": "hi" })).await;

    assert_eq!(again["status"], "error");
    assert_eq!(again["output"], "Error: the MCP server stopped");
}

#[tokio::test]
async fn a_server_that_never_answers_initialize_is_skipped_without_blocking_the_others() {
    let configs = [server("stuck", "hang"), server("fake", "normal")];
    let start = Instant::now();

    let Some(h) = harness(&configs).await else {
        return;
    };

    assert!(start.elapsed() < Duration::from_secs(8));
    assert!(h.state.providers.contains(ECHO));
    assert!(!h.state.providers.contains("mcp:stuck:echo"));
}

#[tokio::test]
async fn a_server_with_an_unknown_protocol_version_is_skipped() {
    assert!(provider(&[server("old", "old")], quick()).await.is_none());
}

#[tokio::test]
async fn servers_that_cannot_start_or_have_bad_settings_are_skipped() {
    let mut missing = server("missing", "normal");
    missing.command = "/definitely/not/here".to_string();
    let mut empty = server("empty", "normal");
    empty.command = "  ".to_string();
    let bad_name = server("has space", "normal");
    let good = server("good", "normal");
    let duplicate = server("good", "normal");

    let Some(h) = harness(&[missing, empty, bad_name, good, duplicate]).await else {
        return;
    };

    assert!(h.state.providers.contains("mcp:good:echo"));
    assert!(names(&h).iter().all(|name| name.starts_with("mcp__good__")));
}

#[tokio::test]
async fn two_servers_keep_their_tools_apart() {
    let Some(h) = harness(&[server("one", "normal"), server("two", "normal")]).await else {
        return;
    };

    assert!(h.state.providers.contains("mcp:one:echo"));
    assert!(h.state.providers.contains("mcp:two:echo"));
    let names = names(&h);
    assert!(names.contains(&"mcp__one__echo".to_string()));
    assert!(names.contains(&"mcp__two__echo".to_string()));
}

#[tokio::test]
async fn a_chatty_server_and_a_paged_list_work() {
    let Some(h) = harness(&[server("chatty", "chatty")]).await else {
        return;
    };
    assert!(h.state.providers.contains("mcp:chatty:echo"));

    let Some(paged) = harness(&[server("paged", "paged")]).await else {
        return;
    };
    assert_eq!(names(&paged), ["mcp__paged__echo", "mcp__paged__fail"]);
}

#[tokio::test]
async fn cancelling_aborts_a_running_call() {
    let Some(mut h) = harness(&[server("fake", "normal")]).await else {
        return;
    };
    let chat = Uuid::new_v4();
    let mut call = h.execute(chat, "c1", "mcp:fake:slow", "{}").await;
    h.panel().await;
    h.decide(chat, "c1", "allow_once").await;
    assert!(matches!(
        h.panel().await,
        DaemonMessage::ToolCallStarted { .. }
    ));

    nook_core::tool_approval::abort_running_tools(&h.state, call.client_id).await;

    common::assert_worker_silent(&mut call).await;
    assert!(h.state.running_tools.lock().await.is_empty());
}

#[tokio::test]
async fn the_policy_resolves_every_mcp_tool_to_the_shared_entry() {
    let policy = common::repo_policy();

    let spec = policy.catalog.resolve("mcp:anything:at_all").unwrap();

    assert_eq!(spec.display_name, "MCP tool");
    assert_eq!(spec.approval.tier, "ask");
    assert!(policy.catalog.resolve("nook:nothing").is_none());
}

#[tokio::test]
async fn sources_list_each_server_as_ready_or_off_with_a_reason() {
    let Some(sandbox) = sandbox().await else {
        return;
    };
    let mut broken = server("broken", "normal");
    broken.command = "/nonexistent/server".to_string();
    let mut empty_command = server("blank", "normal");
    empty_command.command = " ".to_string();
    let configs = [
        server("fake", "normal"),
        broken,
        empty_command,
        server("fake", "normal"),
        server("bad name", "normal"),
    ];

    let (provider, failed) = McpProvider::start_reporting(&configs, &sandbox, quick()).await;

    let ready = provider.expect("one server runs").sources();
    assert_eq!(ready.len(), 1);
    assert_eq!(ready[0].source_id, "mcp:fake");
    assert_eq!(ready[0].state, ToolSourceState::Ready);
    assert!(ready[0].tools >= 2);
    let mut reasons: Vec<(String, String)> = failed
        .into_iter()
        .map(|s| {
            assert_eq!(s.state, ToolSourceState::Off);
            (s.source_id, s.reason.unwrap())
        })
        .collect();
    reasons.sort();
    assert_eq!(reasons.len(), 4);
    assert!(reasons
        .iter()
        .any(|(id, r)| id == "mcp:blank" && r == "The command is empty"));
    assert!(reasons
        .iter()
        .any(|(id, r)| id == "mcp:broken" && !r.is_empty()));
    assert!(reasons
        .iter()
        .any(|(id, r)| id == "mcp:fake" && r.contains("name")));
    assert!(reasons
        .iter()
        .any(|(id, r)| id == "mcp:bad name" && r.contains("not valid")));
}

#[tokio::test]
async fn a_stopped_server_shows_as_stopped_until_a_call_starts_it() {
    let limits = StartupLimits {
        restart_interval: Duration::ZERO,
        ..quick()
    };
    let Some(sandbox) = sandbox().await else {
        return;
    };
    let provider = McpProvider::start(&[server("fake", "normal")], &sandbox, limits)
        .await
        .unwrap();
    let mut h = Harness::new(vec![Arc::new(provider)]).await;

    run(&mut h, Uuid::new_v4(), "c1", "mcp:fake:die", json!({})).await;
    tokio::time::sleep(Duration::from_millis(300)).await;
    let stopped = h.state.providers.sources();
    assert_eq!(stopped[0].state, ToolSourceState::Stopped);
    assert!(stopped[0].reason.is_some());

    run(&mut h, Uuid::new_v4(), "c2", ECHO, json!({ "text": "hi" })).await;
    assert_eq!(h.state.providers.sources()[0].state, ToolSourceState::Ready);
}
