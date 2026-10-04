create schema if not exists app;

create table if not exists app.chats (
  id          uuid primary key,
  title       text not null default '',
  created_at  timestamptz not null default now(),
  updated_at  timestamptz not null default now()
);

create table if not exists app.messages (
  id          uuid primary key,
  chat_id     uuid not null references app.chats(id) on delete cascade,
  role        text not null check (role in ('user', 'assistant')),
  text        text not null default '',
  status      text not null check (status in ('complete', 'cancelled', 'error')),
  error_code  text,
  error_text  text,
  created_at  timestamptz not null default now()
);
create index if not exists messages_chat_created_idx on app.messages (chat_id, created_at);

create table if not exists app.attachments (
  id          uuid primary key,
  message_id  uuid not null references app.messages(id) on delete cascade,
  kind        text not null check (kind in ('image', 'text', 'pdf')),
  name        text not null,
  mime        text not null,
  size_bytes  bigint not null,
  path        text not null
);
create index if not exists attachments_message_idx on app.attachments (message_id);
