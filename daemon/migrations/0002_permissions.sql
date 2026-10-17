create table if not exists app.persistent_grants (
  id          text primary key,
  tool_key    text not null,
  scope       text not null check (scope in ('persistent', 'persistent_host')),
  host        text,
  created_at  timestamptz not null default now()
);

create table if not exists app.tool_preferences (
  tool_id     text primary key,
  tier        text not null check (tier in ('ask', 'deny')),
  updated_at  timestamptz not null default now()
);
