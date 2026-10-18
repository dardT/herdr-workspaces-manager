# Herdr workspaces manager

Find local projects and open them in new Herdr workspaces or in panes and tabs within your current workspace. This standalone Rust picker searches Git repositories and ordinary folders, with favorites, recent projects, and configurable search roots. It opens in a Herdr popup without Python, Node, or a package manager at launch.

[![Workspaces manager demo showing search, favorites, root scopes, and batch creation](docs/workspaces-manager-demo.gif)](docs/workspaces-manager-demo.mp4)

*Animated preview — click to open the full video.*

## Get started

Requires **Herdr 0.9.3+** on macOS or Linux.

1. Install the plugin:

   ```sh
   herdr plugin install dardT/herdr-workspaces-manager
   ```

2. Open the picker from a Herdr workspace:

   ```sh
   herdr plugin action invoke open --plugin herdr-workspaces-manager
   ```

3. **Strongly recommended:** add a shortcut to `~/.config/herdr/config.toml` so you can open the picker directly in Herdr without typing the CLI command each time:

   ```toml
   [[keys.command]]
   key = "prefix+shift+o"
   type = "plugin_action"
   command = "herdr-workspaces-manager.open"
   description = "Find projects and open workspaces or panes"
   ```

   Run `herdr server reload-config` after editing the file. With Herdr's default prefix, `prefix+shift+o` means **Ctrl+B**, then **Shift+O**. The opening shortcut is separate from the picker shortcuts below.

## Use the picker

### Find a project

The picker starts in **All**, which discovers directories under your home directory and, on macOS, `/Users/Shared` when available. Type to fuzzy-search names, paths, or Git branches. Queries beginning with `/` match absolute path prefixes (for example, `/Users/Shared/`); `~/` matches prefixes under your home directory. Results are ordered by directory depth (shallowest first), then full path, except in Recent. Use **Up/Down**, the mouse wheel, or a click to highlight a row. Clicking a row only highlights it; it does not create a workspace.

Switch between **All**, **Favorites**, **Recent**, and each configured root with **Ctrl+G** (forward), **Ctrl+Shift+G** (backward), or a click on a scope label. Each root is a narrower search scope and can point outside your home directory. Favorites and Recent form their own scopes rather than being inserted into other scopes. Scope labels keep a stable order. **Recent** fuzzy-filters its projects but always lists matches by last-opened time, newest first; dates are not displayed.

A **★** marks a favorite; a **⌂** marks a directory that exactly matches a configured root. Both can appear on the same row. Results show `git` for ordinary Git checkouts, `wt` for linked Git worktrees, and `dir` for other directories. Git rows display a compact branch label (or `@` plus a short commit ID for detached HEAD). Branch names are searchable in every scope without changing its ordering.

### Organize favorites and roots

| Action | Shortcut |
| --- | --- |
| Toggle the highlighted directory as a favorite | **Ctrl+F** |
| Add the highlighted directory's full path as a root | **Ctrl+R** |
| Remove a highlighted configured root | **Ctrl+Shift+R** |

Adding a root does not change the active scope or rescan All. To add a directory that is not in the results, add its full path to `roots` in [the plugin configuration](#configuration). Removing a highlighted root works from any scope; if you are in a root scope but have highlighted a different directory, removal asks for confirmation. In Favorites, a removed favorite remains highlighted briefly so **Ctrl+F** can immediately restore it.

Some terminals cannot report **Ctrl+Shift+R** distinctly. In that case, edit `roots` in the configuration file.

### Open projects

- Press **Enter** to create a workspace for the highlighted directory and close the picker.
- Press **Ctrl+T** to open it in the current workspace and focus its new pane.
- Press **Ctrl+Space** to add or remove a directory from a pending batch; plain Space still types into search. Some terminals report Ctrl+Space as a NUL key; the picker accepts both encodings.
- With a batch pending, press **Enter** to review workspaces or **Ctrl+T** to review panes/tabs in the current workspace. These are mutually exclusive destinations for that batch. Remove a mistake with **Delete/Backspace** (or Ctrl+Space), press **Esc** to select more, or press **Enter** to confirm.

The picker previews the highlighted project's proposed workspace name. Workspace batch review refreshes open workspaces before estimating names. Pane batch review estimates placements in the invoking tab; live layout changes may affect where panes open. If a preview is unavailable, opening remains possible.

Each workspace selection creates a new workspace, even if that path is already open. Ctrl+T instead splits the largest eligible pane in the invoking tab, choosing right or down to extend a rectangular grid of at most 2 columns × 2 rows by default. If the tab is full or its layout cannot be extended within those limits, it creates a new tab in the same workspace, without moving existing panes or changing Herdr's default tab label. Batch openings continue filling each new tab and focus the last created pane. Set `max_pane_columns` and `max_pane_rows` to positive integers to customize the grid.

Successfully opened project paths are added to Recent, with repeated paths moved to the top; the latest 30 distinct paths are kept across picker sessions. Partial batches record projects opened before a failure. Created workspaces stay unfocused by default. Workspace labels use the shortest suffix that distinguishes open paths; repeated identical paths receive `#1`, `#2`, and so on. Existing labels change only when a new workspace makes them ambiguous.

### Browse and edit

| Action | Shortcut |
| --- | --- |
| Browse the highlighted directory's subfolders | **Tab** |
| Browse the current directory's parent | **Shift+Tab** |
| Leave directory browsing for All | **Ctrl+G** |
| Show configurable shortcuts | **?** |
| Close the picker or return from review/help | **Esc** |

Entering or leaving a directory clears the search. Tab shows descendants only; Shift+Tab from All or your home directory opens its parent (`/Users` on macOS). Highlight `Shared` there and press Tab to browse `/Users/Shared`. Shift+Tab can continue toward the filesystem root. Browsing `/Users` may also index accessible folders in other users’ homes. The help overlay is also available through the `? Shortcuts` footer hint.

Search supports shell-style editing: **Ctrl+A/E** moves to the start/end; **Ctrl+U/K** deletes before/after the caret; **Ctrl+W** deletes the previous word. Arrow keys, Home, End, Backspace, and Delete also work. **Left** is the reliable way to move the caret left if Herdr intercepts **Ctrl+B** as its prefix. **Ctrl+F** always toggles a favorite, not caret-right.

## Configuration

The plugin creates `config.json` on first launch. Find its directory with:

```sh
herdr plugin config-dir herdr-workspaces-manager
```

For example, edit `$(herdr plugin config-dir herdr-workspaces-manager)/config.json` to set search roots, favorites, and layout limits:

```json
{
  "roots": ["~/ZedProjects", "~/PycharmProjects"],
  "favorites": ["~/ZedProjects/special-project"],
  "max_depth": 20,
  "include_hidden": true,
  "focus_created": false,
  "max_pane_columns": 2,
  "max_pane_rows": 2
}
```

Recent history is kept in `recent.json` under Herdr's plugin state directory (`HERDR_PLUGIN_STATE_DIR`), or beside `config.json` when running the picker directly. It is maintained automatically, not added to the configuration. Existing `~/ZedProjects` and `~/PycharmProjects` directories become initial roots when present. Other defaults include a 20-level discovery limit, `include_hidden: false`, and exclusion of hidden folders and common build/dependency directories (`node_modules`, `target`, `.venv`, and others). You can override those exclusions with `excluded_names` and the picker shortcuts with `keybindings`; press **?** in the picker to see the active bindings. Existing configurations automatically gain missing bindings; the old generated `shift+enter,ctrl+t` batch binding is migrated to `ctrl+space`, while custom bindings are preserved. The old generated `ctrl+d` root-removal binding is migrated to `ctrl+shift+r`.

### How discovery works

All searches your home directory and, on macOS, `/Users/Shared` when it exists. It skips macOS's `~/Library` for responsiveness; an explicitly configured root inside `~/Library` remains searchable. Unreadable shared directories are skipped under normal filesystem permissions. Discovery runs in the background and respects applicable `.gitignore` and global Git ignore rules. Set `include_hidden` to `true` to search dot-directories such as `.claude` and `.pi`. This overrides the legacy `".*"` entry in `excluded_names` (also present in existing configs); other excluded names, Git ignore rules, and the `.git` metadata directory remain pruned. Discovery follows directory symlinks under `/Users/Shared` on macOS, preserving their Shared paths in search results, but does not follow them under your home directory. A symlinked project may therefore also appear under its real home path. Discovery displays at most 1,000 matches at once. Favorites may include paths outside your home directory. Project opening acts on the **local** Herdr session.

## Installation details and development

The installer downloads a matching release binary for macOS (Apple Silicon or Intel) or Linux (x86-64 or ARM64) and checks its SHA-256 checksum **before** extraction. An unavailable asset or unsupported platform falls back to a Rust/Cargo source build; a checksum mismatch stops installation. Forks can override the release repository with `HERDR_WORKSPACES_MANAGER_REPO=OWNER/REPO`.

For local development, Rust/Cargo is required. Run `task link` to build a release binary and link this checkout to Herdr; after code changes, run `task build` again. The local launcher prefers `target/release/herdr-workspaces-manager` over an installed `bin/` copy. Use `task check` for formatting and Clippy, or `task release-check` for both checks and a release build.

For release steps, see [CONTRIBUTING.md](CONTRIBUTING.md). Runtime or installer changes require a new version: CI rejects changes since the latest release tag when the manifest still names that release. The release workflow validates the tag, builds four native binaries, publishes each archive with a `.sha256` companion, uses [CHANGELOG.md](CHANGELOG.md) as release notes, and smoke-tests installation without Cargo on all four target platforms. `target/` and installed `bin/` binaries are not committed.
