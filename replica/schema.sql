-- Local SQLite (via GRDB or SwiftData-equivalent). One file: ~/Library/Application Support/<App>/data.sqlite
-- Single local user, so no user_id / RLS: the OS sandbox is the access rule.
PRAGMA foreign_keys = ON;

create table shelf_items (
  id text primary key,                    -- uuid
  kind text not null check (kind in ('file','folder','image','text','url')),
  display_name text not null,
  bookmark blob,                          -- security-scoped bookmark (reference mode)
  stored_path text,                       -- set only in copy mode (inside app container)
  mode text not null default 'reference' check (mode in ('reference','copy')),
  byte_size integer,
  pinned integer not null default 0,
  added_at text not null default (strftime('%Y-%m-%dT%H:%M:%fZ','now')),
  check (bookmark is not null or stored_path is not null)
);
create index shelf_items_added on shelf_items (pinned desc, added_at desc);

create table clip_entries (
  id text primary key,
  kind text not null check (kind in ('text','image','file','url')),
  text_value text,
  blob_path text,
  source_app text,
  copied_at text not null default (strftime('%Y-%m-%dT%H:%M:%fZ','now'))
);
create index clip_entries_copied on clip_entries (copied_at desc);

create table notes (
  id text primary key,
  body text not null,
  created_at text not null default (strftime('%Y-%m-%dT%H:%M:%fZ','now')),
  updated_at text not null default (strftime('%Y-%m-%dT%H:%M:%fZ','now'))
);

create table shortcuts (
  id text primary key,
  bundle_id text not null unique,
  position integer not null
);
create index shortcuts_pos on shortcuts (position);

create table counters (
  id text primary key,
  label text not null,
  kind text not null check (kind in ('counter','days_left','water')),
  value integer not null default 0,
  target_date text                        -- for days_left
);
-- Settings live in UserDefaults (not a table). Timers are in-memory, no table.
