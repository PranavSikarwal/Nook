#![cfg(unix)]

mod common;

use common::{authorization_result, Call, Harness};
use nook_core::protocol::{ClientMessage, DaemonMessage, GrantInfo, ToolPolicyInfo};
use nook_core::server::Server;
use std::path::PathBuf;
use std::time::Duration;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::net::UnixStream;
use tokio::sync::broadcast;
use uuid::Uuid;

const FETCH: &str = "nook:web_fetch";
const SEARCH: &str = "nook:web_search";

fn fetch_args(url: &str) -> String {
    serde_json::json!({ "url": url }).to_string()
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
            "/tmp/nk_perm_{}.sock",
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

    async fn permissions(&mut self) -> (Vec<GrantInfo>, Vec<ToolPolicyInfo>) {
        match self
            .ask(ClientMessage::ListPermissions { id: Uuid::new_v4() })
            .await
        {
            DaemonMessage::Permissions { grants, tools, .. } => (grants, tools),
            other => panic!("Expected permissions, got {other:?}"),
        }
    }

    async fn tool_sources(&mut self) -> Vec<nook_core::protocol::ToolSourceInfo> {
        match self
            .ask(ClientMessage::ListToolSources { id: Uuid::new_v4() })
            .await
        {
            DaemonMessage::ToolSources { sources, .. } => sources,
            other => panic!("Expected tool sources, got {other:?}"),
        }
    }

    async fn set_preference(&mut self, tool_id: &str, tier: &str) -> DaemonMessage {
        self.ask(ClientMessage::SetToolPreference {
            id: Uuid::new_v4(),
            tool_id: tool_id.to_string(),
            tier: tier.to_string(),
        })
        .await
    }

    async fn revoke(&mut self, grant_id: &str) -> DaemonMessage {
        self.ask(ClientMessage::RevokeGrant {
            id: Uuid::new_v4(),
            grant_id: grant_id.to_string(),
        })
        .await
    }
}

async fn fetch(h: &Harness, chat: Uuid, call_id: &str, url: &str) -> Call {
    h.authorize(chat, call_id, FETCH, &fetch_args(url)).await
}

async fn always_allow_host(h: &mut Harness, chat: Uuid, call_id: &str, url: &str) {
    let mut call = fetch(h, chat, call_id, url).await;
    let DaemonMessage::ApprovalRequested { actions, .. } = h.panel().await else {
        panic!("Expected an approval request");
    };
    assert!(actions.iter().any(|a| a.id == "always_allow_host"));
    h.decide(chat, call_id, "always_allow_host").await;
    assert_eq!(authorization_result(&mut call).await.0, "allow");
}

#[tokio::test]
async fn an_always_allow_host_grant_works_in_every_chat_for_that_host_only() {
    let mut h = Harness::new(vec![]).await;
    always_allow_host(&mut h, Uuid::new_v4(), "c1", "https://docs.example.com/a").await;

    let mut other_chat = fetch(&h, Uuid::new_v4(), "c2", "https://docs.example.com/b").await;
    assert_eq!(authorization_result(&mut other_chat).await.0, "allow");
    h.assert_panel_silent();

    let _other_host = fetch(&h, Uuid::new_v4(), "c3", "https://other.example.com/").await;
    assert!(matches!(
        h.panel().await,
        DaemonMessage::ApprovalRequested { .. }
    ));
}

#[tokio::test]
async fn a_persistent_grant_survives_deleting_the_chat_that_made_it() {
    let mut h = Harness::new(vec![]).await;
    let chat = Uuid::new_v4();
    always_allow_host(&mut h, chat, "c1", "https://docs.example.com/a").await;

    h.state
        .policy
        .authorizer
        .grant_store()
        .lock()
        .await
        .remove_chat(chat);

    let mut again = fetch(&h, Uuid::new_v4(), "c2", "https://docs.example.com/a").await;
    assert_eq!(authorization_result(&mut again).await.0, "allow");
}

#[tokio::test]
async fn the_grant_list_names_the_tool_and_the_host() {
    let mut h = Harness::new(vec![]).await;
    always_allow_host(&mut h, Uuid::new_v4(), "c1", "https://docs.example.com/a").await;
    let mut session = Session::open(&h).await;

    let (grants, _) = session.permissions().await;

    assert_eq!(grants.len(), 1);
    assert_eq!(grants[0].tool_name, "Web fetch");
    assert_eq!(grants[0].scope, "persistent_host");
    assert_eq!(
        grants[0].description,
        "Always allow Web fetch for docs.example.com, in every chat"
    );
}

#[tokio::test]
async fn a_revoked_grant_asks_again_and_the_list_updates() {
    let mut h = Harness::new(vec![]).await;
    always_allow_host(&mut h, Uuid::new_v4(), "c1", "https://docs.example.com/a").await;
    let mut session = Session::open(&h).await;
    let (grants, _) = session.permissions().await;

    let DaemonMessage::Permissions { grants: after, .. } =
        session.revoke(&grants[0].grant_id).await
    else {
        panic!("Expected the updated permissions");
    };

    assert!(after.is_empty());
    let _call = fetch(&h, Uuid::new_v4(), "c2", "https://docs.example.com/a").await;
    assert!(matches!(
        h.panel().await,
        DaemonMessage::ApprovalRequested { .. }
    ));
}

#[tokio::test]
async fn revoking_an_unknown_grant_is_an_error() {
    let h = Harness::new(vec![]).await;
    let mut session = Session::open(&h).await;

    let reply = session.revoke("does-not-exist").await;

    let DaemonMessage::Error { error, .. } = reply else {
        panic!("Expected an error");
    };
    assert_eq!(error.message, "Unknown grant");
}

#[tokio::test]
async fn a_chat_grant_is_not_listed_as_permanent() {
    let mut h = Harness::new(vec![]).await;
    let chat = Uuid::new_v4();
    let mut call = fetch(&h, chat, "c1", "https://docs.example.com/a").await;
    h.panel().await;
    h.decide(chat, "c1", "allow_for_chat_host").await;
    assert_eq!(authorization_result(&mut call).await.0, "allow");
    let mut session = Session::open(&h).await;

    let (grants, _) = session.permissions().await;

    assert!(grants.is_empty());
}

#[tokio::test]
async fn an_action_the_tool_did_not_offer_cannot_create_a_permanent_grant() {
    let mut h = Harness::new(vec![]).await;
    let chat = Uuid::new_v4();
    let _call = fetch(&h, chat, "c1", "https://docs.example.com/a").await;
    h.panel().await;

    h.decide(chat, "c1", "always_allow").await;

    let DaemonMessage::Error { error, .. } = h.panel().await else {
        panic!("Expected an error");
    };
    assert!(error.message.contains("not offered"));
    let mut session = Session::open(&h).await;
    assert!(session.permissions().await.0.is_empty());
}

#[tokio::test]
async fn the_tool_list_shows_defaults_and_only_stricter_choices() {
    let h = Harness::new(vec![]).await;
    let mut session = Session::open(&h).await;

    let (_, tools) = session.permissions().await;

    let by_id = |id: &str| tools.iter().find(|t| t.tool_id == id).unwrap();
    let search = by_id(SEARCH);
    assert_eq!(search.default_tier, "allow");
    assert_eq!(search.tier, "default");
    let tiers: Vec<&str> = search.choices.iter().map(|c| c.tier.as_str()).collect();
    assert_eq!(tiers, ["default", "ask", "deny"]);
    let fetch = by_id(FETCH);
    assert_eq!(fetch.default_tier, "ask_once_per_host");
    let tiers: Vec<&str> = fetch.choices.iter().map(|c| c.tier.as_str()).collect();
    assert_eq!(tiers, ["default", "deny"]);
    assert!(fetch.choices.iter().all(|c| !c.effect.is_empty()));
    assert!(
        tools
            .iter()
            .all(|t| !t.tool_id.contains("local") && t.tool_id != "nook:run_command"),
        "tools that have no provider must not be listed"
    );
}

#[tokio::test]
async fn denying_a_tool_stops_it_even_with_a_permanent_grant() {
    let mut h = Harness::new(vec![]).await;
    always_allow_host(&mut h, Uuid::new_v4(), "c1", "https://docs.example.com/a").await;
    let mut session = Session::open(&h).await;

    let reply = session.set_preference(FETCH, "deny").await;
    let DaemonMessage::Permissions { tools, .. } = reply else {
        panic!("Expected the updated permissions");
    };
    assert_eq!(
        tools.iter().find(|t| t.tool_id == FETCH).unwrap().tier,
        "deny"
    );

    let mut call = fetch(&h, Uuid::new_v4(), "c2", "https://docs.example.com/a").await;
    let (decision, reason) = authorization_result(&mut call).await;
    assert_eq!(decision, "deny");
    assert_eq!(reason.as_deref(), Some("The tool is denied by policy"));
    h.assert_panel_silent();

    session.set_preference(FETCH, "default").await;
    let mut allowed = fetch(&h, Uuid::new_v4(), "c3", "https://docs.example.com/a").await;
    assert_eq!(authorization_result(&mut allowed).await.0, "allow");
}

#[tokio::test]
async fn asking_every_time_turns_the_automatic_search_into_a_prompt() {
    let mut h = Harness::new(vec![]).await;
    let mut session = Session::open(&h).await;
    let mut automatic = h
        .authorize(Uuid::new_v4(), "c1", SEARCH, r#"{"query":"rust"}"#)
        .await;
    assert_eq!(authorization_result(&mut automatic).await.0, "allow");

    session.set_preference(SEARCH, "ask").await;
    let chat = Uuid::new_v4();
    let mut asked = h.authorize(chat, "c2", SEARCH, r#"{"query":"rust"}"#).await;
    let DaemonMessage::ApprovalRequested { actions, .. } = h.panel().await else {
        panic!("Expected an approval request");
    };
    let ids: Vec<&str> = actions.iter().map(|a| a.id.as_str()).collect();
    assert_eq!(
        ids,
        ["allow_once", "allow_for_chat", "always_allow", "deny"]
    );
    h.decide(chat, "c2", "allow_for_chat").await;
    assert_eq!(authorization_result(&mut asked).await.0, "allow");

    let mut covered = h.authorize(chat, "c3", SEARCH, r#"{"query":"cats"}"#).await;
    assert_eq!(authorization_result(&mut covered).await.0, "allow");
}

#[tokio::test]
async fn a_preference_that_would_loosen_a_tool_or_names_an_unknown_one_is_refused() {
    let h = Harness::new(vec![]).await;
    let mut session = Session::open(&h).await;

    for (tool, tier, message) in [
        (FETCH, "ask", "That choice is not allowed for this tool"),
        (FETCH, "allow", "That choice is not allowed for this tool"),
        (SEARCH, "allow", "That choice is not allowed for this tool"),
        (
            SEARCH,
            "nonsense",
            "That choice is not allowed for this tool",
        ),
        ("nook:nope", "deny", "Unknown tool"),
        ("nook:read_local_file", "deny", "Unknown tool"),
    ] {
        let reply = session.set_preference(tool, tier).await;
        let DaemonMessage::Error { error, .. } = reply else {
            panic!("Expected an error for {tool} {tier}");
        };
        assert_eq!(error.message, message, "{tool} {tier}");
    }
    let (_, tools) = session.permissions().await;
    assert!(tools.iter().all(|t| t.tier == "default"));
}

#[tokio::test]
async fn the_messages_round_trip_through_the_wire_format() {
    let id = Uuid::new_v4();
    for message in [
        ClientMessage::ListPermissions { id },
        ClientMessage::RevokeGrant {
            id,
            grant_id: "abc".to_string(),
        },
        ClientMessage::SetToolPreference {
            id,
            tool_id: "nook:web_fetch".to_string(),
            tier: "deny".to_string(),
        },
    ] {
        let text = serde_json::to_string(&message).unwrap();
        assert_eq!(
            serde_json::from_str::<ClientMessage>(&text).unwrap(),
            message
        );
    }
}

#[tokio::test]
async fn the_daemon_lists_tool_sources_over_the_socket() {
    let h = Harness::new(vec![]).await;
    let mut session = Session::open(&h).await;

    assert!(session.tool_sources().await.is_empty());
}

#[tokio::test]
async fn the_tool_source_messages_round_trip_through_the_wire_format() {
    use nook_core::protocol::{ToolSourceInfo, ToolSourceState};
    let id = Uuid::new_v4();
    let request = ClientMessage::ListToolSources { id };
    let reply = DaemonMessage::ToolSources {
        id,
        sources: vec![
            ToolSourceInfo {
                source_id: "mcp:notes".to_string(),
                display_name: "MCP server notes".to_string(),
                state: ToolSourceState::Off,
                tools: 0,
                reason: Some("The command is empty".to_string()),
            },
            ToolSourceInfo {
                source_id: "filesystem".to_string(),
                display_name: "File tools".to_string(),
                state: ToolSourceState::Ready,
                tools: 3,
                reason: None,
            },
        ],
    };

    for text in [
        serde_json::to_string(&request).unwrap(),
        serde_json::to_string(&reply).unwrap(),
    ] {
        assert!(text.contains("tool_sources") || text.contains("list_tool_sources"));
    }
    let reply_text = serde_json::to_string(&reply).unwrap();
    assert!(!reply_text.contains("\"reason\":null"));
    assert_eq!(
        serde_json::from_str::<DaemonMessage>(&reply_text).unwrap(),
        reply
    );
    assert_eq!(
        serde_json::from_str::<ClientMessage>(&serde_json::to_string(&request).unwrap()).unwrap(),
        request
    );
}
