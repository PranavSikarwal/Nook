#![cfg(unix)]

mod common;

use common::{text_from_worker, tool_result, Harness};
use nook_core::config::{Config, FilesystemConfig, RootAccess, RootConfig, ToolsConfig};
use nook_core::folders;
use nook_core::protocol::DaemonMessage;
use serde_json::{json, Value};
use std::path::{Path, PathBuf};
use tempfile::TempDir;
use uuid::Uuid;

const WRITE: &str = "nook:write_local_file";

struct Area {
    _dir: TempDir,
    base: PathBuf,
}

impl Area {
    fn new() -> Self {
        let dir = tempfile::tempdir().unwrap();
        let base = dir.path().canonicalize().unwrap();
        for name in ["one", "two", "app"] {
            std::fs::create_dir(base.join(name)).unwrap();
        }
        Self { _dir: dir, base }
    }

    fn path(&self, name: &str) -> String {
        self.base.join(name).display().to_string()
    }

    fn config_path(&self) -> PathBuf {
        self.base.join("app/config.toml")
    }

    async fn harness(&self, roots: &[&str]) -> Harness {
        let config = Config {
            tools: ToolsConfig {
                filesystem: FilesystemConfig {
                    roots: roots
                        .iter()
                        .map(|name| RootConfig {
                            path: self.path(name),
                            access: RootAccess::ReadWrite,
                        })
                        .collect(),
                    ..Default::default()
                },
                ..Default::default()
            },
            ..Config::default()
        };
        Config::update_filesystem_roots(&self.config_path(), &config.tools.filesystem.roots)
            .unwrap();
        Harness::with_config(config, self.config_path()).await
    }
}

fn saved_roots(path: &Path) -> Vec<RootConfig> {
    let value: toml::Value = toml::from_str(&std::fs::read_to_string(path).unwrap()).unwrap();
    value["tools"]["filesystem"]
        .clone()
        .try_into::<FilesystemConfig>()
        .unwrap()
        .roots
}

fn write_args(path: &Path, content: &str) -> String {
    json!({ "path": path, "content": content, "overwrite": false }).to_string()
}

async fn catalog(h: &Harness) -> Value {
    let mut call = h.run_text(Uuid::new_v4(), "catalog?").await;
    text_from_worker(&mut call, "Catalog: ").await
}

#[tokio::test]
async fn an_added_folder_works_at_once_and_is_saved() {
    let area = Area::new();
    let mut h = area.harness(&["one"]).await;
    let chat = Uuid::new_v4();
    let target = area.base.join("two/new.txt");

    let roots = folders::add(&h.state, &area.path("two"), RootAccess::ReadWrite)
        .await
        .unwrap();

    assert_eq!(roots.len(), 2);
    assert!(roots.iter().all(|root| root.usable));
    let saved = saved_roots(&area.config_path());
    let saved_paths: Vec<&str> = saved.iter().map(|root| root.path.as_str()).collect();
    assert_eq!(saved_paths, [area.path("one"), area.path("two")]);
    let result = h
        .approve_and_run(
            chat,
            "w1",
            WRITE,
            &write_args(&target, "hello"),
            "allow_once",
        )
        .await;
    assert_eq!(result["status"], "ok", "{result}");
    assert_eq!(std::fs::read_to_string(target).unwrap(), "hello");
}

#[tokio::test]
async fn the_tool_descriptions_and_the_worker_catalog_follow_a_change() {
    let area = Area::new();
    let h = area.harness(&["one"]).await;
    let before = catalog(&h).await;
    assert!(!before.to_string().contains(&area.path("two")));

    folders::add(&h.state, &area.path("two"), RootAccess::Read)
        .await
        .unwrap();

    let after = catalog(&h).await;
    let write = after
        .as_array()
        .unwrap()
        .iter()
        .find(|tool| tool["id"] == WRITE)
        .unwrap();
    assert!(write["description"]
        .as_str()
        .unwrap()
        .contains(&format!("{} (read only)", area.path("two"))));
    let described = h.state.providers.descriptor(WRITE).unwrap();
    assert!(described.description.contains(&area.path("two")));
}

#[tokio::test]
async fn changing_the_access_of_a_folder_takes_effect_at_once() {
    let area = Area::new();
    let mut h = area.harness(&["one", "two"]).await;
    let chat = Uuid::new_v4();
    let target = area.base.join("two/blocked.txt");

    folders::set_access(&h.state, &area.path("two"), RootAccess::Read)
        .await
        .unwrap();

    let output = h
        .expect_denied(chat, WRITE, &write_args(&target, "x"))
        .await;
    assert_eq!(output, "The folder is read-only");
    assert!(!target.exists());
    assert_eq!(saved_roots(&area.config_path())[1].access, RootAccess::Read);
}

#[tokio::test]
async fn a_removed_folder_is_refused_for_new_calls() {
    let area = Area::new();
    let mut h = area.harness(&["one", "two"]).await;
    let chat = Uuid::new_v4();
    let target = area.base.join("one/late.txt");

    folders::remove(&h.state, &area.path("one")).await.unwrap();

    let output = h
        .expect_denied(chat, WRITE, &write_args(&target, "x"))
        .await;
    assert_eq!(output, "Path is outside the allowed folders");
    assert!(!target.exists());
}

#[tokio::test]
async fn removing_a_folder_denies_a_waiting_approval() {
    let area = Area::new();
    let mut h = area.harness(&["one", "two"]).await;
    let chat = Uuid::new_v4();
    let target = area.base.join("one/waiting.txt");
    let mut call = h
        .execute(chat, "w1", WRITE, &write_args(&target, "x"))
        .await;
    assert!(matches!(
        h.panel().await,
        DaemonMessage::ApprovalRequested { .. }
    ));

    folders::remove(&h.state, &area.path("one")).await.unwrap();

    let result = tool_result(&mut call).await;
    assert_eq!(result["status"], "denied", "{result}");
    assert_eq!(result["output"], "Path is outside the allowed folders");
    assert!(!target.exists());
    assert!(h.state.pending_approvals.lock().await.is_empty());
}

#[tokio::test]
async fn a_waiting_approval_in_another_folder_stays() {
    let area = Area::new();
    let mut h = area.harness(&["one", "two"]).await;
    let chat = Uuid::new_v4();
    let target = area.base.join("two/stays.txt");
    let _call = h
        .execute(chat, "w1", WRITE, &write_args(&target, "x"))
        .await;
    assert!(matches!(
        h.panel().await,
        DaemonMessage::ApprovalRequested { .. }
    ));

    folders::remove(&h.state, &area.path("one")).await.unwrap();

    assert_eq!(h.state.pending_approvals.lock().await.len(), 1);
}

#[tokio::test]
async fn removing_every_folder_sends_the_worker_an_empty_catalog() {
    let area = Area::new();
    let h = area.harness(&["one"]).await;
    assert!(!catalog(&h).await.as_array().unwrap().is_empty());

    folders::remove(&h.state, &area.path("one")).await.unwrap();

    assert!(catalog(&h).await.as_array().unwrap().is_empty());
    assert!(h.state.providers.is_empty());
}

#[tokio::test]
async fn chat_grants_inside_a_removed_folder_are_dropped() {
    let area = Area::new();
    let mut h = area.harness(&["one", "two"]).await;
    let chat = Uuid::new_v4();
    let inside = area.base.join("one/file.txt");
    let other = area.base.join("two/file.txt");
    for (call, path) in [("g1", &inside), ("g2", &other)] {
        let result = h
            .approve_and_run(
                chat,
                call,
                WRITE,
                &write_args(path, "x"),
                "allow_for_chat_path",
            )
            .await;
        assert_eq!(result["status"], "ok", "{result}");
    }
    let store = h.state.policy.authorizer.grant_store();
    let before =
        store
            .lock()
            .await
            .matching_scopes(chat, "write_local_file", None, inside.to_str(), None);
    assert!(!before.is_empty());

    folders::remove(&h.state, &area.path("one")).await.unwrap();

    let guard = store.lock().await;
    assert!(guard
        .matching_scopes(chat, "write_local_file", None, inside.to_str(), None)
        .is_empty());
    assert!(!guard
        .matching_scopes(chat, "write_local_file", None, other.to_str(), None)
        .is_empty());
}

#[tokio::test]
async fn a_failed_save_changes_nothing() {
    let area = Area::new();
    let h = area.harness(&["one"]).await;
    std::fs::remove_dir_all(area.base.join("app")).unwrap();
    std::fs::write(area.base.join("app"), "not a folder").unwrap();

    let error = folders::add(&h.state, &area.path("two"), RootAccess::Read)
        .await
        .unwrap_err();

    assert_eq!(error, "The list could not be saved to config.toml");
    assert_eq!(folders::list(&h.state).await.len(), 1);
    assert!(!h
        .state
        .providers
        .descriptor(WRITE)
        .unwrap()
        .description
        .contains(&area.path("two")));
}

#[tokio::test]
async fn unknown_folders_and_bad_paths_are_errors() {
    let area = Area::new();
    let h = area.harness(&["one"]).await;

    assert!(folders::remove(&h.state, &area.path("two")).await.is_err());
    assert!(
        folders::set_access(&h.state, &area.path("two"), RootAccess::Read)
            .await
            .is_err()
    );
    assert!(folders::add(&h.state, "relative", RootAccess::Read)
        .await
        .is_err());
    assert!(folders::add(&h.state, &area.path("one"), RootAccess::Read)
        .await
        .is_err());
    assert_eq!(folders::list(&h.state).await.len(), 1);
}
