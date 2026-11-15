#![cfg(unix)]

mod common;

use common::{authorization_result, tool_result, Harness};
use nook_core::config::Config;
use nook_core::protocol::{ClientMessage, DaemonMessage, ErrorCode};
use nook_core::server::Server;
use std::os::unix::fs::PermissionsExt;
use std::path::PathBuf;
use std::time::Duration;
use tempfile::TempDir;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::net::UnixStream;
use tokio::sync::broadcast;
use uuid::Uuid;

struct Fixture {
    _dir: TempDir,
    program: PathBuf,
    config_dir: TempDir,
}

impl Fixture {
    fn new() -> Self {
        let dir = tempfile::tempdir().unwrap();
        let _ = std::fs::set_permissions(dir.path(), std::fs::Permissions::from_mode(0o700));
        let program = dir.path().join("fake_osascript.sh");
        let script = "#!/bin/sh\necho \"ok\"\n";
        std::fs::write(&program, script).unwrap();
        std::fs::set_permissions(&program, std::fs::Permissions::from_mode(0o755)).unwrap();
        let config_dir = tempfile::tempdir().unwrap();
        let _ = std::fs::set_permissions(config_dir.path(), std::fs::Permissions::from_mode(0o700));
        Self {
            _dir: dir,
            program,
            config_dir,
        }
    }

    fn config_path(&self) -> PathBuf {
        self.config_dir.path().join("config.toml")
    }

    async fn harness(&self) -> Harness {
        self.harness_with_program(Some(self.program.clone())).await
    }

    async fn harness_with_program(&self, program: Option<PathBuf>) -> Harness {
        self.harness_with_config_and_program(Config::default(), program)
            .await
    }

    async fn harness_with_config_and_program(
        &self,
        config: Config,
        program: Option<PathBuf>,
    ) -> Harness {
        let config_path = self.config_path();
        let toml_str = toml::to_string_pretty(&config).unwrap();
        std::fs::write(&config_path, toml_str).unwrap();
        Harness::with_config_and_notes(config, config_path, program).await
    }
}

struct Session {
    lines: tokio::io::Lines<BufReader<tokio::net::unix::OwnedReadHalf>>,
    writer: tokio::net::unix::OwnedWriteHalf,
    socket_path: PathBuf,
    _shutdown: broadcast::Sender<()>,
}

impl Drop for Session {
    fn drop(&mut self) {
        let _ = std::fs::remove_file(&self.socket_path);
    }
}

impl Session {
    async fn open(h: &Harness) -> Self {
        let socket_path = PathBuf::from(format!(
            "/tmp/nk_notes_{}.sock",
            &Uuid::new_v4().to_string()[..8]
        ));
        let (shutdown, shutdown_rx) = broadcast::channel(1);
        let server = Server::bind(&socket_path, shutdown_rx, h.state.clone())
            .await
            .unwrap();
        tokio::spawn(server.run());
        let stream = UnixStream::connect(&socket_path).await.unwrap();
        let (reader, writer) = stream.into_split();
        Self {
            lines: BufReader::new(reader).lines(),
            writer,
            socket_path,
            _shutdown: shutdown,
        }
    }

    async fn ask(&mut self, message: ClientMessage) -> DaemonMessage {
        let mut line = serde_json::to_string(&message).unwrap();
        line.push('\n');
        self.writer.write_all(line.as_bytes()).await.unwrap();
        let reply = tokio::time::timeout(Duration::from_secs(5), self.lines.next_line())
            .await
            .expect("Timeout waiting for the Daemon")
            .unwrap()
            .expect("The Daemon closed the connection");
        serde_json::from_str(&reply).unwrap()
    }
}

#[tokio::test]
async fn get_notes_settings_returns_the_stored_folder_and_supported_state() {
    let fixture = Fixture::new();
    let h = fixture.harness().await;
    let mut session = Session::open(&h).await;
    let id = Uuid::new_v4();

    let reply = session.ask(ClientMessage::GetNotesSettings { id }).await;

    match reply {
        DaemonMessage::NotesSettings {
            id: reply_id,
            folder,
            supported,
            reason,
        } => {
            assert_eq!(reply_id, id);
            assert_eq!(folder, "Nook");
            assert!(supported);
            assert_eq!(reason, None);
        }
        other => panic!("Expected NotesSettings, got {other:?}"),
    }
}

#[tokio::test]
async fn a_good_change_updates_config_toml_and_rebuilds_the_tools() {
    let fixture = Fixture::new();
    let h = fixture.harness().await;
    let mut session = Session::open(&h).await;
    let id = Uuid::new_v4();

    let reply = session
        .ask(ClientMessage::SetNotesFolder {
            id,
            folder: "  Project Notes  ".to_string(),
        })
        .await;

    match reply {
        DaemonMessage::NotesSettings {
            id: reply_id,
            folder,
            supported,
            reason,
        } => {
            assert_eq!(reply_id, id);
            assert_eq!(folder, "Project Notes");
            assert!(supported);
            assert_eq!(reason, None);
        }
        other => panic!("Expected NotesSettings, got {other:?}"),
    }

    // Verify config.toml on disk was updated
    let content = std::fs::read_to_string(fixture.config_path()).unwrap();
    let loaded: Config = toml::from_str(&content).unwrap();
    assert_eq!(loaded.tools.notes.folder, "Project Notes");

    // Verify descriptors were rebuilt with the new folder name
    let create_tool = h
        .state
        .providers
        .descriptor("nook:notes_create_note")
        .expect("create tool descriptor");
    assert!(
        create_tool.description.contains("Project Notes"),
        "tool description should name new folder: {}",
        create_tool.description
    );
}

#[tokio::test]
async fn a_bad_folder_name_is_refused_and_changes_nothing() {
    let fixture = Fixture::new();
    let h = fixture.harness().await;
    let mut session = Session::open(&h).await;
    let id = Uuid::new_v4();

    for bad_name in [
        "",
        "   ",
        "bad:name",
        "Recently Deleted",
        "recently deleted",
    ] {
        let reply = session
            .ask(ClientMessage::SetNotesFolder {
                id,
                folder: bad_name.to_string(),
            })
            .await;

        match reply {
            DaemonMessage::Error {
                id: reply_id,
                error,
            } => {
                assert_eq!(reply_id, id);
                assert_eq!(error.code, ErrorCode::InvalidRequest);
                assert_eq!(error.message, "The Notes folder name is not valid");
            }
            other => panic!("Expected Error for bad name '{bad_name}', got {other:?}"),
        }
    }

    // Verify config.toml on disk was not changed
    let content = std::fs::read_to_string(fixture.config_path()).unwrap();
    let loaded: Config = toml::from_str(&content).unwrap();
    assert_eq!(loaded.tools.notes.folder, "Nook");
}

#[tokio::test]
async fn a_bad_name_already_in_config_toml_is_repaired_by_a_good_change() {
    let fixture = Fixture::new();
    let mut config = Config::default();
    config.tools.notes.folder = "Recently Deleted".to_string();
    let h = fixture
        .harness_with_config_and_program(config, Some(fixture.program.clone()))
        .await;
    let mut session = Session::open(&h).await;

    // Get notes settings should report the stored value and the reason
    let get_id = Uuid::new_v4();
    let reply = session
        .ask(ClientMessage::GetNotesSettings { id: get_id })
        .await;

    match reply {
        DaemonMessage::NotesSettings {
            folder,
            supported,
            reason,
            ..
        } => {
            assert_eq!(folder, "Recently Deleted");
            assert!(supported);
            assert_eq!(
                reason.as_deref(),
                Some("The Notes folder name is not valid")
            );
        }
        other => panic!("Expected NotesSettings with reason, got {other:?}"),
    }

    // Repair with a valid name
    let set_id = Uuid::new_v4();
    let set_reply = session
        .ask(ClientMessage::SetNotesFolder {
            id: set_id,
            folder: "Repaired Notes".to_string(),
        })
        .await;

    match set_reply {
        DaemonMessage::NotesSettings {
            folder,
            supported,
            reason,
            ..
        } => {
            assert_eq!(folder, "Repaired Notes");
            assert!(supported);
            assert_eq!(reason, None);
        }
        other => panic!("Expected repaired NotesSettings, got {other:?}"),
    }

    // Subsequent GetNotesSettings should now report ready with no reason
    let check_reply = session
        .ask(ClientMessage::GetNotesSettings { id: Uuid::new_v4() })
        .await;
    match check_reply {
        DaemonMessage::NotesSettings {
            folder,
            supported,
            reason,
            ..
        } => {
            assert_eq!(folder, "Repaired Notes");
            assert!(supported);
            assert_eq!(reason, None);
        }
        other => panic!("Expected clean NotesSettings, got {other:?}"),
    }
}

#[tokio::test]
async fn supported_is_false_when_the_program_path_is_none() {
    let fixture = Fixture::new();
    let h = fixture.harness_with_program(None).await;
    let mut session = Session::open(&h).await;

    // Initial get_notes_settings has supported == false
    let get_reply = session
        .ask(ClientMessage::GetNotesSettings { id: Uuid::new_v4() })
        .await;
    match get_reply {
        DaemonMessage::NotesSettings {
            folder,
            supported,
            reason,
            ..
        } => {
            assert_eq!(folder, "Nook");
            assert!(!supported);
            assert_eq!(reason, None);
        }
        other => panic!("Expected NotesSettings with supported=false, got {other:?}"),
    }

    // A folder change still updates config even when supported == false
    let set_reply = session
        .ask(ClientMessage::SetNotesFolder {
            id: Uuid::new_v4(),
            folder: "LinuxNotes".to_string(),
        })
        .await;
    match set_reply {
        DaemonMessage::NotesSettings {
            folder,
            supported,
            reason,
            ..
        } => {
            assert_eq!(folder, "LinuxNotes");
            assert!(!supported);
            assert_eq!(reason, None);
        }
        other => panic!("Expected NotesSettings with supported=false, got {other:?}"),
    }

    let content = std::fs::read_to_string(fixture.config_path()).unwrap();
    let loaded: Config = toml::from_str(&content).unwrap();
    assert_eq!(loaded.tools.notes.folder, "LinuxNotes");
}

#[tokio::test]
async fn a_failed_write_returns_the_fixed_error_and_changes_nothing() {
    let fixture = Fixture::new();
    // Harness with a path in a missing directory that cannot be created
    let unwritable_path = PathBuf::from("/nonexistent_directory_for_nook_test/config.toml");
    let h = Harness::with_config_and_notes(
        Config::default(),
        unwritable_path,
        Some(fixture.program.clone()),
    )
    .await;
    let mut session = Session::open(&h).await;
    let id = Uuid::new_v4();

    let reply = session
        .ask(ClientMessage::SetNotesFolder {
            id,
            folder: "NewFolder".to_string(),
        })
        .await;

    match reply {
        DaemonMessage::Error {
            id: reply_id,
            error,
        } => {
            assert_eq!(reply_id, id);
            assert_eq!(error.code, ErrorCode::InvalidRequest);
            assert_eq!(error.message, "The folder could not be saved");
        }
        other => panic!("Expected Error 'The folder could not be saved', got {other:?}"),
    }

    // In-memory config must be unchanged
    assert_eq!(h.state.config.read().await.tools.notes.folder, "Nook");
}

#[tokio::test]
async fn another_tools_approval_and_grant_stay_across_a_notes_folder_change() {
    let fixture = Fixture::new();
    let mut h = fixture.harness().await;
    let mut session = Session::open(&h).await;
    let chat = Uuid::new_v4();

    // 1. Start another tool's call: nook:web_fetch
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

    // 2. Add a chat grant for web_fetch
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

    // 3. Create a Notes chat grant through the real flow (allow_for_chat)
    let res = h
        .approve_and_run(
            chat,
            "notes_init",
            "nook:notes_create_note",
            r#"{"title":"Note Title","body":"Note Body"}"#,
            "allow_for_chat",
        )
        .await;
    assert_eq!(res["status"], "ok");

    // 4. Start a Notes tool call (nook:notes_delete_note)
    let mut notes_call = h
        .execute(
            chat,
            "notes1",
            "nook:notes_delete_note",
            r#"{"note_id":"x-coredata://12345678-1234-1234-1234-1234567890ab/ICNote/p1","note_title":"Note Title"}"#,
        )
        .await;
    let notes_panel_msg = h.panel().await;
    assert!(matches!(
        notes_panel_msg,
        DaemonMessage::ApprovalRequested { .. }
    ));

    // 5. Change the Notes folder via the socket session
    let set_reply = session
        .ask(ClientMessage::SetNotesFolder {
            id: Uuid::new_v4(),
            folder: "NewFolder".to_string(),
        })
        .await;
    assert!(matches!(
        set_reply,
        DaemonMessage::NotesSettings {
            folder,
            supported: true,
            ..
        } if folder == "NewFolder"
    ));

    // 6. Verify that the pending approval for fetch1 STAYED
    {
        let approvals = h.state.pending_approvals.lock().await;
        assert!(
            approvals.contains_key("fetch1"),
            "approval for another tool must stay"
        );
        // And the notes tool approval was DENIED/removed
        assert!(
            !approvals.contains_key("notes1"),
            "approval for notes tool must be removed"
        );
    }

    // 7. Verify that the grant for web_fetch STAYED and notes chat grant was DROPPED
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

    // 8. Verify the notes call received denial with "The Notes folder changed"
    let notes_result = tool_result(&mut notes_call).await;
    assert_eq!(notes_result["status"], "denied");
    assert_eq!(notes_result["output"], "The Notes folder changed");

    // 9. Decide allow_once on fetch1: it must succeed
    h.decide(chat, "fetch1", "allow_once").await;
    let (decision, _) = authorization_result(&mut other_call).await;
    assert_eq!(decision, "allow");
}
