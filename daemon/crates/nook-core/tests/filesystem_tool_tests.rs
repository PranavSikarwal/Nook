#![cfg(unix)]

mod common;

use common::{tool_result, Harness};
use nook_core::config::{FilesystemConfig, RootAccess, RootConfig};
use nook_core::protocol::DaemonMessage;
use nook_core::providers::filesystem::FilesystemProvider;
use serde_json::{json, Value};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use tempfile::TempDir;
use uuid::Uuid;

const READ: &str = "nook:read_local_file";
const LIST: &str = "nook:list_local_directory";
const WRITE: &str = "nook:write_local_file";

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
        std::fs::create_dir(base.join("rw/sub")).unwrap();
        std::fs::write(base.join("rw/sub/deep.txt"), "deep").unwrap();
        std::fs::write(base.join("ro/readme.txt"), "ro readme").unwrap();
        std::fs::write(base.join("outside/secret.txt"), "secret").unwrap();
        Self { _dir: dir, base }
    }

    fn path(&self, relative: &str) -> String {
        self.base.join(relative).display().to_string()
    }

    fn config(&self) -> FilesystemConfig {
        FilesystemConfig {
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
        }
    }

    async fn harness(&self) -> Harness {
        let provider = FilesystemProvider::from_config(&self.config()).unwrap();
        Harness::new(vec![Arc::new(provider)]).await
    }
}

fn args(path: &str) -> String {
    json!({ "path": path }).to_string()
}

fn write_args(path: &str, content: &str, overwrite: bool) -> String {
    json!({ "path": path, "content": content, "overwrite": overwrite }).to_string()
}

async fn approved(h: &mut Harness, chat: Uuid, call: &str, tool: &str, a: &str) -> Value {
    h.approve_and_run(chat, call, tool, a, "allow_once").await
}

#[tokio::test]
async fn the_catalog_lists_the_file_tools() {
    let area = Area::new();
    let provider = FilesystemProvider::from_config(&area.config()).unwrap();
    let registry = nook_core::providers::ProviderRegistry::new(vec![Arc::new(provider)]).unwrap();

    let names: Vec<&str> = registry
        .descriptors()
        .iter()
        .map(|d| d.name.as_str())
        .collect();

    assert_eq!(
        names,
        [
            "read_local_file",
            "list_local_directory",
            "write_local_file"
        ]
    );
}

#[tokio::test]
async fn every_file_tool_names_the_allowed_folders_and_their_access() {
    let area = Area::new();
    let provider = FilesystemProvider::from_config(&area.config()).unwrap();
    let registry = nook_core::providers::ProviderRegistry::new(vec![Arc::new(provider)]).unwrap();

    for descriptor in registry.descriptors() {
        assert!(
            descriptor
                .description
                .contains(&format!("{} (read and write)", area.path("rw"))),
            "{}: {}",
            descriptor.name,
            descriptor.description
        );
        assert!(
            descriptor
                .description
                .contains(&format!("{} (read only)", area.path("ro"))),
            "{}: {}",
            descriptor.name,
            descriptor.description
        );
    }
}

#[tokio::test]
async fn a_read_only_setup_offers_no_write_tool() {
    let area = Area::new();
    let mut config = area.config();
    config.roots.retain(|root| root.access == RootAccess::Read);
    let provider = FilesystemProvider::from_config(&config).unwrap();
    let registry = nook_core::providers::ProviderRegistry::new(vec![Arc::new(provider)]).unwrap();

    assert!(registry.descriptors().iter().all(|d| d.id != WRITE));
}

#[tokio::test]
async fn no_usable_root_means_no_provider() {
    let config = FilesystemConfig {
        roots: vec![
            RootConfig {
                path: "relative/path".to_string(),
                access: RootAccess::Read,
            },
            RootConfig {
                path: "/definitely/not/here".to_string(),
                access: RootAccess::Read,
            },
        ],
        ..FilesystemConfig::default()
    };

    assert!(FilesystemProvider::from_config(&config).is_none());
}

#[tokio::test]
async fn reads_a_file_after_approval() {
    let area = Area::new();
    let mut h = area.harness().await;
    let file = area.path("rw/notes.txt");

    let result = approved(&mut h, Uuid::new_v4(), "c1", READ, &args(&file)).await;

    assert_eq!(result["status"], "ok");
    assert_eq!(result["output"], "rw notes");
}

#[tokio::test]
async fn the_approval_names_the_exact_path() {
    let area = Area::new();
    let mut h = area.harness().await;
    let file = area.path("rw/notes.txt");
    let _call = h.execute(Uuid::new_v4(), "c1", READ, &args(&file)).await;

    let DaemonMessage::ApprovalRequested {
        resource_summary,
        actions,
        ..
    } = h.panel().await
    else {
        panic!("Expected an approval request");
    };

    assert_eq!(resource_summary, format!("Read {file}"));
    let ids: Vec<&str> = actions.iter().map(|a| a.id.as_str()).collect();
    assert_eq!(
        ids,
        [
            "allow_once",
            "allow_for_chat_path",
            "allow_for_chat_dir",
            "deny"
        ]
    );
}

#[tokio::test]
async fn lists_a_directory_after_approval() {
    let area = Area::new();
    let mut h = area.harness().await;

    let result = approved(&mut h, Uuid::new_v4(), "c1", LIST, &args(&area.path("rw"))).await;

    assert_eq!(result["status"], "ok");
    assert_eq!(result["output"], "notes.txt (8 bytes)\nsub/");
}

#[tokio::test]
async fn writes_a_new_file_and_refuses_to_replace_it_without_overwrite() {
    let area = Area::new();
    let mut h = area.harness().await;
    let chat = Uuid::new_v4();
    let target = area.path("rw/new.txt");

    let first = approved(
        &mut h,
        chat,
        "c1",
        WRITE,
        &write_args(&target, "one", false),
    )
    .await;
    assert_eq!(first["status"], "ok", "{first}");
    assert_eq!(std::fs::read_to_string(&target).unwrap(), "one");

    let second = approved(
        &mut h,
        chat,
        "c2",
        WRITE,
        &write_args(&target, "two", false),
    )
    .await;
    assert_eq!(second["status"], "error");
    assert_eq!(std::fs::read_to_string(&target).unwrap(), "one");

    let third = approved(&mut h, chat, "c3", WRITE, &write_args(&target, "two", true)).await;
    assert_eq!(third["status"], "ok");
    assert_eq!(std::fs::read_to_string(&target).unwrap(), "two");
}

#[tokio::test]
async fn a_write_prompt_shows_the_size_and_the_mode() {
    let area = Area::new();
    let mut h = area.harness().await;
    let target = area.path("rw/new.txt");
    let _call = h
        .execute(
            Uuid::new_v4(),
            "c1",
            WRITE,
            &write_args(&target, "abc", true),
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
        format!("Write 3 bytes to {target} (replace the file if it exists)")
    );
    let ids: Vec<&str> = actions.iter().map(|a| a.id.as_str()).collect();
    assert_eq!(ids, ["allow_once", "allow_for_chat_path", "deny"]);
}

#[tokio::test]
async fn a_denied_write_changes_nothing() {
    let area = Area::new();
    let mut h = area.harness().await;
    let chat = Uuid::new_v4();
    let target = area.path("rw/denied.txt");
    let mut call = h
        .execute(chat, "c1", WRITE, &write_args(&target, "x", false))
        .await;
    h.panel().await;

    h.decide(chat, "c1", "deny").await;

    assert_eq!(tool_result(&mut call).await["status"], "denied");
    assert!(!Path::new(&target).exists());
}

#[tokio::test]
async fn paths_outside_the_roots_are_refused_before_any_prompt() {
    let area = Area::new();
    let mut h = area.harness().await;
    let chat = Uuid::new_v4();

    for tool in [READ, LIST] {
        let output = h
            .expect_denied(chat, tool, &args(&area.path("outside/secret.txt")))
            .await;
        assert_eq!(output, "Path is outside the allowed folders");
    }
    let output = h
        .expect_denied(
            chat,
            WRITE,
            &write_args(&area.path("outside/new.txt"), "x", false),
        )
        .await;
    assert_eq!(output, "Path is outside the allowed folders");
    assert!(!area.base.join("outside/new.txt").exists());
}

#[tokio::test]
async fn parent_components_and_relative_paths_are_refused() {
    let area = Area::new();
    let mut h = area.harness().await;
    let chat = Uuid::new_v4();

    let traversal = format!("{}/../outside/secret.txt", area.path("rw"));
    assert_eq!(
        h.expect_denied(chat, READ, &args(&traversal)).await,
        "Path must not contain .."
    );
    assert_eq!(
        h.expect_denied(chat, READ, &args("notes.txt")).await,
        "Path must be absolute"
    );
    assert_eq!(
        h.expect_denied(chat, READ, &args("\u{0}")).await,
        "Path is not valid"
    );
    assert_eq!(
        h.expect_denied(chat, READ, &args(&format!("{}\u{0}x", area.path("rw"))))
            .await,
        "Path is not valid"
    );
}

#[tokio::test]
async fn a_sibling_folder_with_the_same_prefix_is_outside() {
    let area = Area::new();
    std::fs::create_dir(area.base.join("rw-other")).unwrap();
    std::fs::write(area.base.join("rw-other/file.txt"), "x").unwrap();
    let mut h = area.harness().await;

    let output = h
        .expect_denied(Uuid::new_v4(), READ, &args(&area.path("rw-other/file.txt")))
        .await;

    assert_eq!(output, "Path is outside the allowed folders");
}

#[tokio::test]
async fn a_write_in_a_read_only_folder_is_refused() {
    let area = Area::new();
    let mut h = area.harness().await;

    let output = h
        .expect_denied(
            Uuid::new_v4(),
            WRITE,
            &write_args(&area.path("ro/new.txt"), "x", false),
        )
        .await;

    assert_eq!(output, "The folder is read-only");
    assert!(!area.base.join("ro/new.txt").exists());
}

#[tokio::test]
async fn a_link_inside_the_folder_is_followed() {
    let area = Area::new();
    std::os::unix::fs::symlink("notes.txt", area.base.join("rw/alias.txt")).unwrap();
    let mut h = area.harness().await;

    let result = approved(
        &mut h,
        Uuid::new_v4(),
        "c1",
        READ,
        &args(&area.path("rw/alias.txt")),
    )
    .await;

    assert_eq!(result["output"], "rw notes");
}

#[tokio::test]
async fn a_link_that_leaves_the_folder_cannot_be_read() {
    let area = Area::new();
    std::os::unix::fs::symlink(
        area.base.join("outside/secret.txt"),
        area.base.join("rw/escape.txt"),
    )
    .unwrap();
    let mut h = area.harness().await;

    let result = approved(
        &mut h,
        Uuid::new_v4(),
        "c1",
        READ,
        &args(&area.path("rw/escape.txt")),
    )
    .await;

    assert_eq!(result["status"], "error");
    assert!(!result["output"].as_str().unwrap().contains("secret"));
}

#[tokio::test]
async fn a_folder_link_that_leaves_the_folder_cannot_be_listed_or_written_through() {
    let area = Area::new();
    std::os::unix::fs::symlink(area.base.join("outside"), area.base.join("rw/out")).unwrap();
    let mut h = area.harness().await;
    let chat = Uuid::new_v4();

    let listing = approved(&mut h, chat, "c1", LIST, &args(&area.path("rw/out"))).await;
    assert_eq!(listing["status"], "error");

    let write = approved(
        &mut h,
        chat,
        "c2",
        WRITE,
        &write_args(&area.path("rw/out/planted.txt"), "x", false),
    )
    .await;
    assert_eq!(write["status"], "error");
    assert!(!area.base.join("outside/planted.txt").exists());
}

#[tokio::test]
async fn a_write_through_a_file_link_that_leaves_the_folder_is_blocked() {
    let area = Area::new();
    std::os::unix::fs::symlink(
        area.base.join("outside/secret.txt"),
        area.base.join("rw/link.txt"),
    )
    .unwrap();
    let mut h = area.harness().await;

    let result = approved(
        &mut h,
        Uuid::new_v4(),
        "c1",
        WRITE,
        &write_args(&area.path("rw/link.txt"), "overwritten", true),
    )
    .await;

    assert_eq!(result["status"], "error");
    assert_eq!(
        std::fs::read_to_string(area.base.join("outside/secret.txt")).unwrap(),
        "secret"
    );
}

#[tokio::test]
async fn a_large_file_is_refused() {
    let area = Area::new();
    let mut config = area.config();
    config.max_read_bytes = 10;
    std::fs::write(area.base.join("rw/big.txt"), "x".repeat(11)).unwrap();
    let provider = FilesystemProvider::from_config(&config).unwrap();
    let mut h = Harness::new(vec![Arc::new(provider)]).await;

    let result = approved(
        &mut h,
        Uuid::new_v4(),
        "c1",
        READ,
        &args(&area.path("rw/big.txt")),
    )
    .await;

    assert_eq!(result["status"], "error");
    assert!(result["output"].as_str().unwrap().contains("read limit"));
}

#[tokio::test]
async fn content_above_the_write_limit_is_refused() {
    let area = Area::new();
    let mut config = area.config();
    config.max_write_bytes = 4;
    let provider = FilesystemProvider::from_config(&config).unwrap();
    let mut h = Harness::new(vec![Arc::new(provider)]).await;

    let output = h
        .expect_denied(
            Uuid::new_v4(),
            WRITE,
            &write_args(&area.path("rw/w.txt"), "12345", false),
        )
        .await;

    assert_eq!(output, "Content is larger than the write limit");
}

#[tokio::test]
async fn a_file_that_is_not_text_and_a_directory_are_errors() {
    let area = Area::new();
    std::fs::write(area.base.join("rw/bin.dat"), [0xff, 0xfe, 0x00]).unwrap();
    let mut h = area.harness().await;
    let chat = Uuid::new_v4();

    let binary = approved(&mut h, chat, "c1", READ, &args(&area.path("rw/bin.dat"))).await;
    assert!(binary["output"].as_str().unwrap().contains("UTF-8"));

    let directory = approved(&mut h, chat, "c2", READ, &args(&area.path("rw/sub"))).await;
    assert!(directory["output"]
        .as_str()
        .unwrap()
        .contains("not a regular file"));

    let file_as_dir = approved(&mut h, chat, "c3", LIST, &args(&area.path("rw/notes.txt"))).await;
    assert!(file_as_dir["output"]
        .as_str()
        .unwrap()
        .contains("not a directory"));
}

#[tokio::test]
async fn a_write_needs_an_existing_parent_folder() {
    let area = Area::new();
    let mut h = area.harness().await;

    let result = approved(
        &mut h,
        Uuid::new_v4(),
        "c1",
        WRITE,
        &write_args(&area.path("rw/missing/new.txt"), "x", false),
    )
    .await;

    assert_eq!(result["status"], "error");
    assert!(!area.base.join("rw/missing").exists());
}

#[tokio::test]
async fn a_path_grant_covers_the_same_file_only() {
    let area = Area::new();
    let mut h = area.harness().await;
    let chat = Uuid::new_v4();
    let notes = area.path("rw/notes.txt");

    let first = h
        .approve_and_run(chat, "c1", READ, &args(&notes), "allow_for_chat_path")
        .await;
    assert_eq!(first["status"], "ok");

    let mut again = h.execute(chat, "c2", READ, &args(&notes)).await;
    h.drain_tool_events().await;
    assert_eq!(tool_result(&mut again).await["status"], "ok");

    let _other = h
        .execute(chat, "c3", READ, &args(&area.path("rw/sub/deep.txt")))
        .await;
    assert!(matches!(
        h.panel().await,
        DaemonMessage::ApprovalRequested { .. }
    ));
}

#[tokio::test]
async fn a_folder_grant_covers_files_below_it_but_not_a_sibling_with_the_same_prefix() {
    let area = Area::new();
    std::fs::create_dir(area.base.join("rw/sub-other")).unwrap();
    std::fs::write(area.base.join("rw/sub-other/x.txt"), "x").unwrap();
    let mut h = area.harness().await;
    let chat = Uuid::new_v4();

    let granted = h
        .approve_and_run(
            chat,
            "c1",
            READ,
            &args(&area.path("rw/sub/deep.txt")),
            "allow_for_chat_dir",
        )
        .await;
    assert_eq!(granted["status"], "ok");

    std::fs::write(area.base.join("rw/sub/more.txt"), "more").unwrap();
    let mut covered = h
        .execute(chat, "c2", READ, &args(&area.path("rw/sub/more.txt")))
        .await;
    h.drain_tool_events().await;
    assert_eq!(tool_result(&mut covered).await["output"], "more");

    let _other_tool = h
        .execute(chat, "c4", LIST, &args(&area.path("rw/sub")))
        .await;
    assert!(matches!(
        h.panel().await,
        DaemonMessage::ApprovalRequested { .. }
    ));

    let _sibling = h
        .execute(chat, "c3", READ, &args(&area.path("rw/sub-other/x.txt")))
        .await;
    assert!(matches!(
        h.panel().await,
        DaemonMessage::ApprovalRequested { .. }
    ));
}

#[tokio::test]
async fn a_grant_does_not_carry_to_another_chat() {
    let area = Area::new();
    let mut h = area.harness().await;
    let file = area.path("rw/notes.txt");
    h.approve_and_run(
        Uuid::new_v4(),
        "c1",
        READ,
        &args(&file),
        "allow_for_chat_dir",
    )
    .await;

    let _other_chat = h.execute(Uuid::new_v4(), "c2", READ, &args(&file)).await;

    assert!(matches!(
        h.panel().await,
        DaemonMessage::ApprovalRequested { .. }
    ));
}

#[tokio::test]
async fn a_read_grant_does_not_allow_a_write() {
    let area = Area::new();
    let mut h = area.harness().await;
    let chat = Uuid::new_v4();
    h.approve_and_run(
        chat,
        "c1",
        READ,
        &args(&area.path("rw/notes.txt")),
        "allow_for_chat_dir",
    )
    .await;

    let _write = h
        .execute(
            chat,
            "c2",
            WRITE,
            &write_args(&area.path("rw/notes.txt"), "x", true),
        )
        .await;

    assert!(matches!(
        h.panel().await,
        DaemonMessage::ApprovalRequested { .. }
    ));
}

#[tokio::test]
async fn an_offered_action_is_required() {
    let area = Area::new();
    let mut h = area.harness().await;
    let chat = Uuid::new_v4();
    let _call = h
        .execute(
            chat,
            "c1",
            WRITE,
            &write_args(&area.path("rw/a.txt"), "x", false),
        )
        .await;
    h.panel().await;

    h.decide(chat, "c1", "allow_for_chat_dir").await;

    let DaemonMessage::Error { error, .. } = h.panel().await else {
        panic!("Expected an error");
    };
    assert!(error.message.contains("not offered"));
}

#[tokio::test]
async fn credential_files_are_closed_even_inside_an_allowed_folder() {
    let area = Area::new();
    for name in [
        ".ssh/id_rsa",
        ".aws/credentials",
        ".env",
        ".env.local",
        ".gnupg/key",
    ] {
        let path = area.base.join("rw").join(name);
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(&path, "secret").unwrap();
    }
    let mut h = area.harness().await;
    let chat = Uuid::new_v4();

    for name in [
        ".ssh/id_rsa",
        ".aws/credentials",
        ".env",
        ".env.local",
        ".gnupg/key",
    ] {
        let mut call = h
            .execute(chat, "c1", READ, &args(&area.path(&format!("rw/{name}"))))
            .await;
        h.panel().await;
        h.decide(chat, "c1", "allow_once").await;
        let result = tool_result(&mut call).await;
        assert_eq!(result["status"], "denied", "{name}");
        assert_eq!(result["output"], "Policy does not allow this request");
    }
}

#[tokio::test]
async fn git_internals_cannot_be_written() {
    let area = Area::new();
    std::fs::create_dir(area.base.join("rw/.git")).unwrap();
    let mut h = area.harness().await;
    let chat = Uuid::new_v4();
    let target = area.path("rw/.git/hooks-note");
    let mut call = h
        .execute(chat, "c1", WRITE, &write_args(&target, "x", false))
        .await;
    h.panel().await;

    h.decide(chat, "c1", "allow_once").await;

    assert_eq!(tool_result(&mut call).await["status"], "denied");
    assert!(!Path::new(&target).exists());
}

#[tokio::test]
async fn the_worker_cannot_run_a_file_tool_through_authorize_tool() {
    let area = Area::new();
    let h = area.harness().await;
    let mut call = h
        .run_text(
            Uuid::new_v4(),
            &format!("authorize:c1|{READ}|{}", args(&area.path("rw/notes.txt"))),
        )
        .await;
    let event = common::recv(&mut call.worker_rx).await;
    let nook_core::protocol::DaemonWorkerEvent::AuthorizeTool {
        message_id,
        call_id,
        chat_id,
        tool_name,
        arguments,
        ..
    } = event
    else {
        panic!("Expected AuthorizeTool");
    };
    nook_core::tool_approval::handle_authorize_tool(
        &h.state,
        &nook_core::tool_approval::AuthorizeContext {
            client_request_id: call.client_id,
            worker_request_id: call.worker_id,
            chat_id: call.chat_id,
        },
        nook_core::tool_approval::AuthorizeToolCall {
            message_id,
            call_id,
            chat_id,
            tool_name,
            arguments,
            execute: false,
        },
        &h.panel_tx,
    )
    .await;

    let result = common::text_from_worker(&mut call, "Result: ").await;

    assert_eq!(result["decision"], "deny");
    assert_eq!(result["reason"], "Tool runs in the Daemon");
}

#[tokio::test]
async fn the_configured_form_of_a_path_matches_the_canonical_form() {
    let area = Area::new();
    std::os::unix::fs::symlink(area.base.join("rw"), area.base.join("alias")).unwrap();
    let config = FilesystemConfig {
        roots: vec![RootConfig {
            path: area.path("alias"),
            access: RootAccess::Read,
        }],
        ..FilesystemConfig::default()
    };
    let provider = FilesystemProvider::from_config(&config).unwrap();
    let mut h = Harness::new(vec![Arc::new(provider)]).await;

    let result = approved(
        &mut h,
        Uuid::new_v4(),
        "c1",
        READ,
        &args(&area.path("alias/notes.txt")),
    )
    .await;

    assert_eq!(result["output"], "rw notes");
}
