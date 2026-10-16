# Data model

One Postgres database named `nook` on the local Postgres.app (16.3). It has two schemas.

- `app` is owned by the Daemon. It holds the Chat list and the Transcript.
- `worker` is owned by the Worker. LangGraph creates its checkpoint tables there when the Worker calls `setup()`.

The Daemon never reads `worker` tables. The Worker never reads `app` tables. A Chat links the two through its id, which is the LangGraph `thread_id`.

## Schema `app`

Migrations are plain SQL files in `daemon/migrations/`, named `0001_init.sql` and so on. The Daemon applies them in order at start.

```sql
create schema if not exists app;

create table app.chats (
  id          uuid primary key,
  title       text not null default '',
  created_at  timestamptz not null default now(),
  updated_at  timestamptz not null default now()
);

create table app.messages (
  id          uuid primary key,
  chat_id     uuid not null references app.chats(id) on delete cascade,
  role        text not null check (role in ('user', 'assistant')),
  text        text not null default '',
  status      text not null check (status in ('complete', 'cancelled', 'error')),
  error_code  text,
  error_text  text,
  created_at  timestamptz not null default now()
);
create index messages_chat_created_idx on app.messages (chat_id, created_at);

create table app.attachments (
  id          uuid primary key,
  message_id  uuid not null references app.messages(id) on delete cascade,
  kind        text not null check (kind in ('image', 'text', 'pdf')),
  name        text not null,
  mime        text not null,
  size_bytes  bigint not null,
  path        text not null
);
create index attachments_message_idx on app.attachments (message_id);
```

Notes:

1. `app.chats.updated_at` is set whenever a Message is added. History sorts by it, newest first.
2. A user Message is saved with status `complete` before the Worker starts. An assistant Message is saved when `message_finished` or `error` arrives. If the Daemon stops mid-reply, it marks any reply still open as `error` with code `internal` at the next start.
3. A failed reply saves an assistant Message with status `error` and the error code and text, so the Transcript shows the failure after a restart.
4. The Daemon does not store streaming deltas. It accumulates them in memory and writes the full text once.

## Schema `worker`

Created by `AsyncPostgresSaver.setup()`. The Worker's connection sets `search_path` to `worker`. If LangGraph will not write there (open point 2 in the overview), the Worker uses `public` and nothing else changes.

The `thread_id` for every checkpoint is the Chat id as a string.

## Attachment files

Files live in `~/Library/Application Support/Nook/attachments/<chat_id>/<attachment_id>-<name>`. The Panel writes them. The Daemon stores the path. The Worker reads the file from the path when it builds a model request.

## Deleting a Chat

1. The Daemon sends `delete_chat` to the Worker. The Worker calls `adelete_thread(chat_id)` on its checkpointer.
2. The Daemon deletes the `app.chats` row. Foreign keys remove its Messages and Attachment rows.
3. The Daemon removes the `attachments/<chat_id>/` folder.

If step 1 fails, the Daemon still runs steps 2 and 3 and logs the failure. An orphaned checkpoint is harmless and can be cleaned up later.

## Limits

| Limit | Value | Enforced by |
| --- | --- | --- |
| Attachments per Message | 5 | Panel and Daemon |
| Size of one Attachment | 10 MB | Panel and Daemon |
| Extracted text per file | 200,000 characters, then cut with a visible "truncated" note | Worker |
| Image types | PNG, JPEG, WebP, GIF | Panel and Daemon |
| Text types | Plain text, Markdown, source code, JSON, CSV | Panel and Daemon |
| PDF | Text is extracted in the Worker with `pypdf` | Worker |

Checkpoints store inline images as base64, so every step of a reply rewrites that data into Postgres. These limits keep the volume bounded for v1. Storing image references in checkpoints is a later change.
