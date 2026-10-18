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

3. **Recommended:** open `~/.config/herdr/config.toml` in your editor and add a shortcut to open the picker:

   ```toml
   [[keys.command]]
   key = "prefix+shift+o"
   type = "plugin_action"
   command = "herdr-workspaces-manager.open"
   description = "Find projects and open workspaces or panes"
   ```

   Apply it with `herdr server reload-config`. With the default prefix, press **Ctrl+B**, then **Shift+O**. This opens the picker; its internal shortcuts are listed below.

## Use the picker

### Find a project

The picker starts in **All**, which discovers directories under your home directory and, on macOS, `/Users/Shared` when available. Type to fuzzy-search names, paths, or Git branches. Queries beginning with `/` match absolute path prefixes (for example, `/Users/Shared/`); `~/` matches prefixes under your home directory. Results are ordered by directory depth (shallowest first), then full path, except in Recent. Use **Up/Down**, the mouse wheel, or a click to highlight a row. Clicking a row only highlights it; it does not create a workspace.

Switch between **All**, **Favorites**, **Recent**, and each configured root with **Ctrl+G** (forward), **Ctrl+Shift+G** (backward), or a click on a scope label. Each root is a narrower search scope and can point outside your home directory. Favorites and Recent form their own scopes rather than being inserted into other scopes. Scope labels keep a stable order. **Recent** fuzzy-filters its projects but always lists matches by last-opened time, newest first; dates are not displayed.

A **★** marks a favorite; a **⌂** marks a directory that exactly matches a configured root. Both can appear on the same row. Results show `git` for ordinary Git checkouts, `wt` for linked Git worktrees, and `dir` for other directories. Git rows display a compact branch label (or `@` plus a short commit ID for detached HEAD). Branch names are searchable in every scope without changing its ordering.

### Organize favorites and roots

Toggle favorites or add and remove search roots using the [shortcuts below](#shortcuts). Adding a root does not change the active scope. To add a directory not shown in the results, put its path in `roots` in [the plugin configuration](#configuration). Removing the active root while another directory is highlighted asks for confirmation. In Favorites, a removed favorite remains highlighted briefly so it can be restored. If your terminal cannot distinguish Ctrl+Shift+R, edit `roots` directly.

### Open projects

- Press **Enter** to create a workspace for the highlighted directory and close the picker.
- Press **Ctrl+T** to open it in the current workspace and focus its new pane.
- Press **Ctrl+Space** to add or remove a directory from a pending batch; plain Space still types into search. Some terminals report Ctrl+Space as a NUL key; the picker accepts both encodings.
- With a batch pending, press **Enter** to review workspaces or **Ctrl+T** to review panes/tabs in the current workspace. These are mutually exclusive destinations for that batch. Remove a mistake with **Delete/Backspace** (or Ctrl+Space), press **Esc** to select more, or press **Enter** to confirm.

The picker previews the highlighted project's proposed workspace name. Workspace batch review refreshes open workspaces before estimating names. Pane batch review estimates placements in the invoking tab; live layout changes may affect where panes open. If a preview is unavailable, opening remains possible.

Each workspace selection creates a new workspace, even if that path is already open. Ctrl+T instead splits the largest eligible pane in the invoking tab, choosing right or down to extend a rectangular grid of at most 2 columns × 2 rows by default. If the tab is full or its layout cannot be extended within those limits, it creates a new tab in the same workspace, without moving existing panes or changing Herdr's default tab label. Batch openings continue filling each new tab and focus the last created pane. Set `max_pane_columns` and `max_pane_rows` to positive integers to customize the grid.

Successfully opened project paths are added to Recent, with repeated paths moved to the top; the latest 30 distinct paths are kept across picker sessions. Partial batches record projects opened before a failure. Created workspaces stay unfocused by default. Workspace labels use the shortest suffix that distinguishes open paths; repeated identical paths receive `#1`, `#2`, and so on. Existing labels change only when a new workspace makes them ambiguous.

### Shortcuts

| Action | Keys | Config key |
| --- | --- | --- |
| Create workspace / confirm batch | **Enter** | Fixed |
| Open pane or tab in current workspace | **Ctrl+T** | `open_in_workspace` |
| Toggle batch selection | **Ctrl+Space** | `select` |
| Next / previous scope | **Ctrl+G** / **Ctrl+Shift+G** | `scope` / `scope_backward` |
| Browse into / out of a directory | **Tab** / **Shift+Tab** | `enter_directory` / `parent_directory` |
| Toggle favorite | **Ctrl+F** | `favorite` |
| Add / remove root | **Ctrl+R** / **Ctrl+Shift+R** | `add_root` / `remove_root` |
| Show shortcuts | **?** | `help` |
| Navigate results / review | **Up** / **Down** | Fixed |
| Remove selection in review | **Delete** / **Backspace** | Fixed |
| Move search caret | **Left** / **Right**, **Home** / **End**, **Ctrl+A/E/B** | Fixed |
| Edit search query | **Backspace**, **Delete**, **Ctrl+U/K/W** | Fixed |
| Back / close picker | **Esc** | Fixed |

Entering or leaving a directory clears the search. Shift+Tab from All or your home directory opens its parent (`/Users` on macOS); you can then browse `Shared`. Browsing `/Users` may also index accessible folders in other users’ homes. Use **Left** if Herdr intercepts **Ctrl+B** as its prefix; **Ctrl+F** always toggles a favorite, not caret-right.

## Configuration

The picker creates `config.json` on first launch. Find its directory with:

```sh
herdr plugin config-dir herdr-workspaces-manager
```

Open `config.json` there in your preferred editor. Add or adjust these settings inside the existing JSON object rather than replacing your roots or favorites; merge the `keybindings` entries if that object already exists:

```json
{
  "max_pane_columns": 2,
  "max_pane_rows": 2,
  "keybindings": {
    "select": "ctrl+space",
    "open_in_workspace": "ctrl+t"
  }
}
```

Other settings: `roots`, `favorites`, `max_depth` (20), `include_hidden` (false), `focus_created` (false), and `excluded_names`. Recent history is stored separately in `recent.json`. The picker migrates its old generated Shift+Enter/Ctrl+T batch binding to Ctrl+Space; custom bindings are preserved.

### How discovery works

All searches your home directory and, on macOS, `/Users/Shared` when it exists. It skips macOS's `~/Library` for responsiveness; an explicitly configured root inside `~/Library` remains searchable. Unreadable shared directories are skipped under normal filesystem permissions. Discovery runs in the background and respects applicable `.gitignore` and global Git ignore rules. Set `include_hidden` to `true` to search dot-directories such as `.claude` and `.pi`. This overrides the legacy `".*"` entry in `excluded_names` (also present in existing configs); other excluded names, Git ignore rules, and the `.git` metadata directory remain pruned. Discovery follows directory symlinks under `/Users/Shared` on macOS, preserving their Shared paths in search results, but does not follow them under your home directory. A symlinked project may therefore also appear under its real home path. Discovery displays at most 1,000 matches at once. Favorites may include paths outside your home directory. Project opening acts on the **local** Herdr session.

## Installation details and development

The installer downloads a matching release binary for macOS (Apple Silicon or Intel) or Linux (x86-64 or ARM64) and checks its SHA-256 checksum **before** extraction. An unavailable asset or unsupported platform falls back to a Rust/Cargo source build; a checksum mismatch stops installation. Forks can override the release repository with `HERDR_WORKSPACES_MANAGER_REPO=OWNER/REPO`.

For local development, Rust/Cargo is required. Run `task link` to build a release binary and link this checkout to Herdr; after code changes, run `task build` again. The local launcher prefers `target/release/herdr-workspaces-manager` over an installed `bin/` copy. Use `task check` for formatting and Clippy, or `task release-check` for both checks and a release build.

For release steps, see [CONTRIBUTING.md](CONTRIBUTING.md). Runtime or installer changes require a new version: CI rejects changes since the latest release tag when the manifest still names that release. The release workflow validates the tag, builds four native binaries, publishes each archive with a `.sha256` companion, uses [CHANGELOG.md](CHANGELOG.md) as release notes, and smoke-tests installation without Cargo on all four target platforms. `target/` and installed `bin/` binaries are not committed.
