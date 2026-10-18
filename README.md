# Herdr workspaces manager

Find project directories, organize favorites and search roots, and create Herdr workspaces from a fast, standalone Rust picker. Git repositories and ordinary folders are both searchable; full paths make duplicate names easy to distinguish. The plugin opens in a Herdr popup and needs no Python, Node, or package manager at launch.

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
   herdr plugin action invoke --plugin herdr-workspaces-manager open
   ```

3. **Strongly recommended:** add a shortcut to `~/.config/herdr/config.toml` so you can open the picker directly in Herdr without typing the CLI command each time:

   ```toml
   [[keys.command]]
   key = "prefix+shift+o"
   type = "plugin_action"
   command = "herdr-workspaces-manager.open"
   description = "Find projects and create workspaces"
   ```

   Run `herdr server reload-config` after editing the file. With Herdr's default prefix, `prefix+shift+o` means **Ctrl+B**, then **Shift+O**. The opening shortcut is separate from the picker shortcuts below.

## Use the picker

### Find a project

The picker starts in **All**, which discovers directories under your home directory. Type to fuzzy-search names, paths, or Git branches; results are ordered by directory depth (shallowest first), then full path, except in Recent. Use **Up/Down**, the mouse wheel, or a click to highlight a row. Clicking a row only highlights it; it does not create a workspace.

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

### Create workspaces

- Press **Enter** to create a workspace for the highlighted directory and close the picker.
- Press **Shift+Enter** to add or remove a directory from a pending batch. Use **Ctrl+T** if your terminal does not distinguish Shift+Enter.
- With a batch pending, press **Enter** to review only the selected paths and their proposed workspace names. Remove a mistake with **Delete/Backspace** (or Shift+Enter/Ctrl+T), press **Esc** to return and select more, or press **Enter** again to create the remaining workspaces.

The picker also previews the highlighted project's proposed workspace name. Batch review refreshes the list of open Herdr workspaces before predicting names; if Herdr cannot provide that list, creation remains available. Names are estimates and may change if other workspaces are created meanwhile.

Each selection creates a new workspace, even if that path is already open. Successfully created project paths are added to Recent, with repeated paths moved to the top; the latest 30 distinct paths are kept across picker sessions. Partial batches record projects whose workspaces were created even if another creation fails. Created workspaces stay unfocused by default. Labels use the shortest suffix that distinguishes open paths; repeated identical paths receive `#1`, `#2`, and so on. Existing labels change only when a new workspace makes them ambiguous.

### Browse and edit

| Action | Shortcut |
| --- | --- |
| Browse the highlighted directory's subfolders | **Tab** |
| Browse the current directory's parent | **Shift+Tab** |
| Leave directory browsing for All | **Ctrl+G** |
| Show configurable shortcuts | **?** |
| Close the picker or return from review/help | **Esc** |

Entering or leaving a directory clears the search. Tab shows descendants only; Shift+Tab stops at your home directory when browsing from home. The help overlay is also available through the `? Shortcuts` footer hint.

Search supports shell-style editing: **Ctrl+A/E** moves to the start/end; **Ctrl+U/K** deletes before/after the caret; **Ctrl+W** deletes the previous word. Arrow keys, Home, End, Backspace, and Delete also work. **Left** is the reliable way to move the caret left if Herdr intercepts **Ctrl+B** as its prefix. **Ctrl+F** always toggles a favorite, not caret-right.

## Configuration

The plugin creates `config.json` on first launch. Find its directory with:

```sh
herdr plugin config-dir herdr-workspaces-manager
```

For example, edit `$(herdr plugin config-dir herdr-workspaces-manager)/config.json` to set roots and favorites:

```json
{
  "roots": ["~/ZedProjects", "~/PycharmProjects"],
  "favorites": ["~/ZedProjects/special-project"],
  "max_depth": 20,
  "focus_created": false
}
```

Recent history is kept in `recent.json` under Herdr's plugin state directory (`HERDR_PLUGIN_STATE_DIR`), or beside `config.json` when running the picker directly. It is maintained automatically, not added to the configuration. Existing `~/ZedProjects` and `~/PycharmProjects` directories become initial roots when present. Other defaults include a 20-level discovery limit and exclusion of hidden folders and common build/dependency directories (`node_modules`, `target`, `.venv`, and others). You can override those exclusions with `excluded_names` and the picker shortcuts with `keybindings`; press **?** in the picker to see the active bindings. Existing configurations automatically gain missing scope/browsing bindings, and the old generated `ctrl+d` root-removal binding is migrated to `ctrl+shift+r`.

### How discovery works

All searches your home directory but skips macOS's `~/Library` for responsiveness; an explicitly configured root inside `~/Library` remains searchable. Discovery runs in the background and respects applicable `.gitignore` and global Git ignore rules. It prunes hidden folders and excluded names, does not follow directory symlinks, and displays at most 1,000 fuzzy matches at once. Favorites may include paths outside your home directory. Workspace creation acts on the **local** Herdr session.

## Installation details and development

The installer downloads a matching release binary for macOS (Apple Silicon or Intel) or Linux (x86-64 or ARM64) and checks its SHA-256 checksum **before** extraction. An unavailable asset or unsupported platform falls back to a Rust/Cargo source build; a checksum mismatch stops installation. Forks can override the release repository with `HERDR_WORKSPACES_MANAGER_REPO=OWNER/REPO`.

For local development, Rust/Cargo is required. Run `task link` to build a release binary and link this checkout to Herdr; after code changes, run `task build` again. The local launcher prefers `target/release/herdr-workspaces-manager` over an installed `bin/` copy. Use `task check` for formatting and Clippy, or `task release-check` for both checks and a release build.

To publish a release, update the version in `Cargo.toml`, `Cargo.lock`, and `herdr-plugin.toml`, then push the matching tag (for example, `v0.1.1`). Runtime or installer changes require a new version: CI rejects changes since the latest release tag when the manifest still names that release. The release workflow validates the tag, builds four native binaries, publishes each archive with a `.sha256` companion, and smoke-tests installation without Cargo on all four target platforms. `target/` and installed `bin/` binaries are not committed.
