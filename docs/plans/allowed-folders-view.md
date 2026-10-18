# Allowed folders view

Branch: `feat/allowed-folders-view`, cut from `feat/tp7-provider-and-sandbox` (PR #32). Merge #32 first.

## Why we are doing this

The folders that the file and command tools may use are set by hand in `config.toml`. The Daemon reads them once at start. A user cannot see them in Nook, and a change needs an editor and a restart.

## The steps, in order

1. Daemon: add four Panel messages, `list_roots`, `add_root`, `set_root_access`, and `remove_root`. Each answers with the full list (`roots`).
2. Daemon: check each new path. It must be absolute, exist, and be a folder. The Daemon resolves links first. It refuses a folder that is the home folder or contains it (this covers `/`). It refuses a folder that contains the Nook app folder or sits inside it, because `config.toml` lives there. It refuses a folder that overlaps one already listed. The list holds at most 16 folders.
3. Daemon: write the list to `config.toml` with `toml_edit`, so comments and other settings stay, and the file keeps its 0600 mode. Write the file first. Change the memory only after the write works.
4. Daemon: rebuild the file tool and the command tool from the new list. MCP servers keep running. The registry becomes swappable (`ProviderHandle`), so a running call keeps the registry it started with.
5. Daemon: send the new tool list to the Worker (`tool_catalog`). The Worker already accepts this message at any time. The Daemon now also sends it when the list is empty.
6. Daemon: check each pending approval again against the new registry. Deny the ones whose call is no longer valid, with "Path is outside the allowed folders".
7. Daemon: delete chat grants for a file path inside a removed folder.
8. Daemon: log each change (path, access, add or remove) in `daemon.log`.
9. Panel backend: Tauri commands `get_roots`, `add_root`, `set_root_access`, `remove_root`, and `pick_folder`. `pick_folder` opens the system folder picker from Rust through `tauri-plugin-dialog`. The webview gets no dialog permission.
10. Panel backend, macOS only: an application menu with "Settings..." (Cmd+,) and "Allowed Folders..." in the app menu. Both show the window and send an `open_settings` event with a tab name. The default Edit and Window menus stay.
11. Panel frontend: a "Folders" tab in Settings with the list, an access switch, a remove button, and "Add folder...". App.tsx opens Settings on the tab named by `open_settings`.

## What stays the same

- Cedar policy, approval cards, grant scopes, and the sandbox rules.
- Persistent grants stay per tool. File and command tools have none.
- A hand-edited `config.toml` still works. Hand edits made while Nook runs apply at the next start.
- Removing a folder does not touch the files in it.
- The terminal switch, MCP servers, and model settings are not part of this view.

## What is new

- `ProviderHandle` in `providers/mod.rs`, `ProviderRegistry::rebuilt`, `Config::update_filesystem_roots`, `GrantStore::remove_paths_inside`, `WorkerSupervisor::update_tools`.
- Messages `list_roots`, `add_root`, `set_root_access`, `remove_root` (Panel to Daemon) and `roots` (Daemon to Panel) in `contracts/panel-daemon.schema.json`.
- `tauri-plugin-dialog` in `panel/src-tauri`.

## What we are not building right now

- Per-chat folders, folders for MCP servers, and a switch for the command tool.
- The application menu on Windows and Linux. The window has no frame there, so a menu bar would change the overlay. Those platforms keep the Settings gear.
- A native folder picker test. WebDriver cannot drive it. The test build reads the folder from `NOOK_E2E_PICK_FOLDER`.
- A test of the macOS menu. WebDriver cannot click it. A Rust test checks that each menu id maps to its tab.

## Risks

- The file and command tools offer no "Always allow" action. Only `allow_once` (and, for files, `allow_for_chat_path`) exist, so a new folder never inherits a permanent grant. Every write and command in it asks first.
- A chat grant for one file path stays valid for that file until the chat ends or its folder is removed.
- A Daemon that outlives the Panel (open item 12) can hold the old folder list. The Folders tab reads the running Daemon, so what you see is what runs.

## How we will know it works

- [ ] Rust tests: add, change, and remove a folder; each refused path (`/`, home, a file, a missing path, a relative path, the app folder, an overlap, the 17th folder); `config.toml` keeps comments and the 0600 mode after a change; the tool descriptions list the new folders.
- [ ] A Rust test: a pending approval is denied after its folder is removed.
- [ ] A Rust test: the Worker receives a new `tool_catalog` after a change, also an empty one.
- [ ] A Rust test: a chat grant inside a removed folder is gone.
- [ ] Contract tests and examples for the five messages.
- [ ] A Worker test: an empty `tool_catalog` leaves the agent with no remote tools.
- [ ] Panel unit tests for the Folders tab.
- [ ] A wdio test: add a folder in the Folders tab, then write a file there through the deterministic flow.
- [ ] Manual: the menu shows both items, and "Allowed Folders..." opens the tab.
