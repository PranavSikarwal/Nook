#![cfg(unix)]

mod common;

use common::{authorization_result, Harness};
use nook_core::db::{self, StoredGrant};
use nook_core::permissions;
use nook_core::protocol::DaemonMessage;
use nook_core::tool_approval::APPROVAL_TIMEOUT;
use sqlx::postgres::PgConnectOptions;
use sqlx::PgPool;
use std::str::FromStr;
use std::time::Duration;
use uuid::Uuid;

const FETCH: &str = "nook:web_fetch";
const SEARCH: &str = "nook:web_search";

struct TestDatabase {
    admin: PgPool,
    name: String,
    url: String,
    pool: PgPool,
}

impl TestDatabase {
    async fn create() -> Option<Self> {
        Self::create_with(true).await
    }

    async fn create_with(migrate: bool) -> Option<Self> {
        let base =
            std::env::var("NOOK_DATABASE_URL").unwrap_or_else(|_| "postgresql:///nook".to_string());
        let admin = match db::create_pool(&base).await {
            Ok(pool) => pool,
            Err(error) => {
                eprintln!("Skipping database test: Postgres unreachable: {error}");
                return None;
            }
        };
        let name = format!(
            "nook_test_{}",
            &Uuid::new_v4().to_string().replace('-', "")[..12]
        );
        if let Err(error) = sqlx::query(&format!("CREATE DATABASE {name}"))
            .execute(&admin)
            .await
        {
            eprintln!("Skipping database test: cannot create {name}: {error}");
            return None;
        }
        let options = PgConnectOptions::from_str(&base).ok()?.database(&name);
        let pool = PgPool::connect_with(options).await.ok()?;
        if migrate {
            db::run_migrations(&pool).await.expect("migrations");
        }
        let (without_query, query) = match base.split_once('?') {
            Some((head, tail)) => (head, format!("?{tail}")),
            None => (base.as_str(), String::new()),
        };
        let prefix = without_query
            .rsplit_once('/')
            .map_or(without_query, |p| p.0);
        let url = format!("{prefix}/{name}{query}");
        Some(Self {
            admin,
            name,
            url,
            pool,
        })
    }

    async fn drop_database(self) {
        self.pool.close().await;
        let _ = sqlx::query(&format!(
            "DROP DATABASE IF EXISTS {} WITH (FORCE)",
            self.name
        ))
        .execute(&self.admin)
        .await;
    }
}

async fn daemon(database: &TestDatabase) -> Harness {
    Harness::build_with_pool(
        common::repo_policy(),
        vec![],
        APPROVAL_TIMEOUT,
        Some(database.pool.clone()),
    )
    .await
}

#[tokio::test]
async fn grants_and_preferences_survive_a_restart() {
    let Some(database) = TestDatabase::create().await else {
        return;
    };
    let mut first = daemon(&database).await;
    let chat = Uuid::new_v4();
    let mut call = first
        .authorize(chat, "c1", FETCH, r#"{"url":"https://docs.example.com/a"}"#)
        .await;
    first.panel().await;
    first.decide(chat, "c1", "always_allow_host").await;
    assert_eq!(authorization_result(&mut call).await.0, "allow");
    permissions::set_preference(&first.state, SEARCH, "ask")
        .await
        .unwrap();

    let stored = db::load_persistent_grants(&database.pool).await.unwrap();
    assert_eq!(stored.len(), 1);
    assert_eq!(stored[0].tool_key, "web_fetch");
    assert_eq!(stored[0].scope, "persistent_host");
    assert_eq!(stored[0].host.as_deref(), Some("docs.example.com"));
    assert_eq!(
        db::load_tool_preferences(&database.pool).await.unwrap(),
        [(SEARCH.to_string(), "ask".to_string())]
    );

    let mut second = daemon(&database).await;
    let _before_restore = second
        .authorize(
            Uuid::new_v4(),
            "c2",
            FETCH,
            r#"{"url":"https://docs.example.com/a"}"#,
        )
        .await;
    assert!(matches!(
        second.panel().await,
        DaemonMessage::ApprovalRequested { .. }
    ));

    permissions::restore(&second.state).await;

    let mut after = second
        .authorize(
            Uuid::new_v4(),
            "c3",
            FETCH,
            r#"{"url":"https://docs.example.com/a"}"#,
        )
        .await;
    assert_eq!(authorization_result(&mut after).await.0, "allow");
    assert_eq!(
        second.state.policy.preference(SEARCH).as_deref(),
        Some("ask")
    );
    database.drop_database().await;
}

#[tokio::test]
async fn revoking_and_resetting_remove_the_stored_rows() {
    let Some(database) = TestDatabase::create().await else {
        return;
    };
    let mut first = daemon(&database).await;
    let chat = Uuid::new_v4();
    let _call = first
        .authorize(chat, "c1", FETCH, r#"{"url":"https://docs.example.com/a"}"#)
        .await;
    first.panel().await;
    first.decide(chat, "c1", "always_allow_host").await;
    permissions::set_preference(&first.state, FETCH, "deny")
        .await
        .unwrap();
    let (grants, _) = permissions::snapshot(&first.state).await;

    permissions::revoke_grant(&first.state, &grants[0].grant_id)
        .await
        .unwrap();
    permissions::set_preference(&first.state, FETCH, "default")
        .await
        .unwrap();

    assert!(db::load_persistent_grants(&database.pool)
        .await
        .unwrap()
        .is_empty());
    assert!(db::load_tool_preferences(&database.pool)
        .await
        .unwrap()
        .is_empty());
    database.drop_database().await;
}

#[tokio::test]
async fn a_stored_row_that_does_not_check_out_is_ignored() {
    let Some(database) = TestDatabase::create().await else {
        return;
    };
    for row in [
        StoredGrant {
            id: "wrong-id".to_string(),
            tool_key: "web_fetch".to_string(),
            scope: "persistent_host".to_string(),
            host: Some("evil.example".to_string()),
        },
        StoredGrant {
            id: "0".repeat(32),
            tool_key: "web_fetch".to_string(),
            scope: "persistent".to_string(),
            host: None,
        },
    ] {
        db::save_persistent_grant(&database.pool, &row)
            .await
            .unwrap();
    }
    let h = daemon(&database).await;

    permissions::restore(&h.state).await;

    let (grants, _) = permissions::snapshot(&h.state).await;
    assert!(grants.is_empty());
    database.drop_database().await;
}

#[tokio::test]
async fn the_same_grant_is_stored_once() {
    let Some(database) = TestDatabase::create().await else {
        return;
    };
    let row = StoredGrant {
        id: "a".repeat(32),
        tool_key: "web_fetch".to_string(),
        scope: "persistent_host".to_string(),
        host: Some("docs.example.com".to_string()),
    };

    db::save_persistent_grant(&database.pool, &row)
        .await
        .unwrap();
    db::save_persistent_grant(&database.pool, &row)
        .await
        .unwrap();

    assert_eq!(
        db::load_persistent_grants(&database.pool).await.unwrap(),
        [row]
    );
    database.drop_database().await;
}

#[tokio::test]
async fn a_changed_preference_replaces_the_old_one() {
    let Some(database) = TestDatabase::create().await else {
        return;
    };

    db::save_tool_preference(&database.pool, SEARCH, "ask")
        .await
        .unwrap();
    db::save_tool_preference(&database.pool, SEARCH, "deny")
        .await
        .unwrap();

    assert_eq!(
        db::load_tool_preferences(&database.pool).await.unwrap(),
        [(SEARCH.to_string(), "deny".to_string())]
    );
    database.drop_database().await;
}

async fn connect_later(h: &Harness, url: &str) {
    tokio::time::timeout(
        Duration::from_secs(10),
        nook_core::reconnect::reconnect_database(
            h.state.clone(),
            url.to_string(),
            Duration::from_millis(20),
            Duration::from_secs(2),
        ),
    )
    .await
    .expect("reconnect finished");
}

#[tokio::test]
async fn the_daemon_keeps_trying_while_the_database_is_down() {
    let h = Harness::build_with_pool(common::repo_policy(), vec![], APPROVAL_TIMEOUT, None).await;
    let task = tokio::spawn(nook_core::reconnect::reconnect_database(
        h.state.clone(),
        "postgresql://127.0.0.1:1/none".to_string(),
        Duration::from_millis(20),
        Duration::from_millis(200),
    ));

    tokio::time::sleep(Duration::from_millis(400)).await;

    assert!(h.state.pool.get().is_none());
    assert!(!task.is_finished());
    task.abort();
}

#[tokio::test]
async fn a_later_connection_prepares_the_database() {
    let Some(database) = TestDatabase::create_with(false).await else {
        return;
    };
    let h = Harness::build_with_pool(common::repo_policy(), vec![], APPROVAL_TIMEOUT, None).await;

    connect_later(&h, &database.url).await;

    assert!(h.state.pool.get().is_some());
    let rows = db::load_persistent_grants(&database.pool).await.unwrap();
    assert!(rows.is_empty());
    database.drop_database().await;
}

#[tokio::test]
async fn a_later_connection_loads_the_stored_grants() {
    let Some(database) = TestDatabase::create().await else {
        return;
    };
    let mut first = daemon(&database).await;
    let chat = Uuid::new_v4();
    let mut call = first
        .authorize(chat, "c1", FETCH, r#"{"url":"https://docs.example.com/a"}"#)
        .await;
    first.panel().await;
    first.decide(chat, "c1", "always_allow_host").await;
    assert_eq!(authorization_result(&mut call).await.0, "allow");

    let second =
        Harness::build_with_pool(common::repo_policy(), vec![], APPROVAL_TIMEOUT, None).await;
    assert!(permissions::snapshot(&second.state).await.0.is_empty());
    connect_later(&second, &database.url).await;

    let (grants, _) = permissions::snapshot(&second.state).await;
    assert_eq!(grants.len(), 1);
    database.drop_database().await;
}
