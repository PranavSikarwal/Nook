use nook_core::db::{
    create_pool, delete_chat, ensure_chat, get_chat, list_chats, repair_open_replies,
    run_migrations, save_assistant_message, save_user_message,
};
use nook_core::protocol::{Attachment, AttachmentKind, MessageStatus};
use uuid::Uuid;

#[tokio::test]
async fn test_database_crud_and_cascade_delete() {
    let db_url = std::env::var("NOOK_DATABASE_URL")
        .unwrap_or_else(|_| "postgresql:///nook".to_string());

    let pool = match create_pool(&db_url).await {
        Ok(p) => p,
        Err(e) => {
            eprintln!("Skipping database test: Postgres unreachable: {e}");
            return;
        }
    };

    run_migrations(&pool).await.expect("Failed to run migrations");

    let chat_id = Uuid::new_v4();
    ensure_chat(&pool, chat_id, "Test Chat Title")
        .await
        .expect("Failed to create chat");

    let user_msg_id = Uuid::new_v4();
    let att_id = Uuid::new_v4();
    let attachment = Attachment {
        id: att_id,
        kind: AttachmentKind::Text,
        name: "test.txt".to_string(),
        mime: "text/plain".to_string(),
        size_bytes: 42,
        path: "/path/to/test.txt".to_string(),
    };

    save_user_message(
        &pool,
        user_msg_id,
        chat_id,
        "What is 2+2?",
        &[attachment],
    )
    .await
    .expect("Failed to save user message");

    let asst_msg_id = Uuid::new_v4();
    save_assistant_message(
        &pool,
        asst_msg_id,
        chat_id,
        "2+2 is 4.",
        MessageStatus::Complete,
        None,
    )
    .await
    .expect("Failed to save assistant message");

    // Verify list_chats
    let chats = list_chats(&pool).await.expect("Failed to list chats");
    assert!(chats.iter().any(|c| c.chat_id == chat_id));

    // Verify get_chat
    let chat_data = get_chat(&pool, chat_id)
        .await
        .expect("Failed to get chat")
        .expect("Chat not found");
    assert_eq!(chat_data.0, "Test Chat Title");
    assert_eq!(chat_data.1.len(), 2);
    assert_eq!(chat_data.1[0].attachments.len(), 1);
    assert_eq!(chat_data.1[0].attachments[0].name, "test.txt");

    // Verify delete_chat cascades
    let deleted = delete_chat(&pool, chat_id)
        .await
        .expect("Failed to delete chat");
    assert!(deleted);

    let after_delete = get_chat(&pool, chat_id)
        .await
        .expect("Failed to get chat after delete");
    assert!(after_delete.is_none());

    // Verify messages and attachments rows are gone
    let msg_count: (i64,) = sqlx::query_as("SELECT count(*) FROM app.messages WHERE chat_id = $1")
        .bind(chat_id)
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(msg_count.0, 0);

    let att_count: (i64,) = sqlx::query_as("SELECT count(*) FROM app.attachments WHERE message_id = $1")
        .bind(user_msg_id)
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(att_count.0, 0);

    // Test repair_open_replies
    let open_chat_id = Uuid::new_v4();
    ensure_chat(&pool, open_chat_id, "Incomplete Chat")
        .await
        .unwrap();
    let unreplied_msg_id = Uuid::new_v4();
    save_user_message(
        &pool,
        unreplied_msg_id,
        open_chat_id,
        "Pending prompt",
        &[],
    )
    .await
    .unwrap();

    let repaired = repair_open_replies(&pool).await.unwrap();
    assert!(repaired >= 1);

    let repaired_chat = get_chat(&pool, open_chat_id).await.unwrap().unwrap();
    assert_eq!(repaired_chat.1.len(), 2);
    let last_msg = &repaired_chat.1[1];
    assert_eq!(last_msg.role, nook_core::protocol::Role::Assistant);
    assert_eq!(last_msg.status, MessageStatus::Error);
    assert_eq!(
        last_msg.error.as_ref().unwrap().code,
        nook_core::protocol::ErrorCode::Internal
    );

    let _ = delete_chat(&pool, open_chat_id).await;
}
