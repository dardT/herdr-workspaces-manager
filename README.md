# Herdr workspaces manager

A fast, standalone Rust plugin for discovering projects, managing roots and favorites, and creating Herdr workspaces. It runs in a session-modal Herdr popup, with no Python, Node, or package manager needed **at launch**. Git repositories and ordinary folders are both searchable; results show full paths to distinguish duplicate names.

## Install

Requires Herdr 0.9.3+. Install from the public GitHub repository:

```sh
herdr plugin install dardT/herdr-workspaces-manager
```

The install hook downloads the platform-specific binary from the matching GitHub release, verifies its SHA-256 checksum **before extraction**, and installs it into Herdr's plugin checkout. Precompiled binaries are published for macOS (Apple Silicon and Intel) and Linux (x86-64 and ARM64); no Rust, Python, Node, or package manager is required on those systems when the release assets are available. If an asset is unavailable or the platform is unsupported, installation falls back to a Rust/Cargo source build. A checksum mismatch aborts installation rather than running an unverified binary. The release repository is fixed as a fallback for source archives lacking Git metadata; `HERDR_WORKSPACES_MANAGER_REPO=OWNER/REPO` overrides it for forks.

For local development, Rust/Cargo is required. Run `task link` once: it builds the release binary and links the plugin to Herdr. After code changes, run `task build` again. Linking a local plugin does not run its manifest build step automatically; the pane launcher uses `target/release/herdr-workspaces-manager` for local builds.

Bind the picker in `~/.config/herdr/config.toml` and reload Herdr's configuration:

```toml
[[keys.command]]
key = "prefix+shift+o"
type = "plugin_action"
command = "herdr-workspaces-manager.open"
description = "Find projects and create workspaces"
```

`prefix+shift+o` is Ctrl+B, then Shift+O with Herdr's default prefix; change `key` to any supported Herdr binding. Reload the running server with `herdr server reload-config`. Alternatively, run `herdr plugin action invoke --plugin herdr-workspaces-manager open`.

## Picker controls

The initial **All** scope searches project directories throughout your home directory, regardless of configured roots, but skips macOS's `~/Library` for responsiveness. A configured root inside `~/Library` remains searchable in its own scope. The first scan may take longer than a single-root scan; results appear when background discovery finishes. Click a scope label, press Ctrl+G to cycle forward, or press Ctrl+Shift+G to cycle backward through All, Favorites, and each configured root individually. Root labels keep the same order when the active scope changes. Type to fuzzy-filter paths and names; matching paths are always ordered by directory depth (shallowest first), then alphabetically by full path. Use Up/Down, the mouse wheel, or click a row to highlight it. **Tab** browses only the highlighted directory's subfolders; **Shift+Tab** browses the current directory's parent (stopping at your home directory during home-based browsing). Entering or leaving a directory clears the query. Ctrl+G exits directory browsing to All. Search supports shell-like caret editing: Ctrl+A/E moves to the beginning/end, Ctrl+B or Left moves left, Right moves right, Ctrl+U/K deletes before/after the caret, and Ctrl+W deletes the previous word. Home, End, Backspace and Delete also work; long searches scroll to keep the caret visible. Ctrl+F remains the favorite toggle, **not** caret-right (use Right); Herdr's Ctrl+B prefix may intercept caret-left, so Left is always available. A click only highlights—it does not create a workspace.

- **Enter** with no pending selection creates the highlighted project's workspace and exits.
- **Shift+Enter** toggles the highlighted project in the pending batch. **Ctrl+T** is the fallback when the outer terminal cannot report modified Enter distinctly.
- **Enter** with pending selections opens a compact, scrollable review of **only** those selected paths. Use Up/Down or click a row, then Delete/Backspace (or Shift+Enter/Ctrl+T) to remove a mistake; Enter creates the remaining workspaces, and Esc returns to the picker to add more. If the batch becomes empty, the review closes. The review's create/remove/back labels are clickable.
- **Esc** in the picker closes it without creating anything.
- **?** (shown as `? Shortcuts` in the footer) opens an overlay listing the active, configurable picker bindings. Esc or ? returns to the picker; the footer hint is also clickable.
- Result rows display a bold amber **★** for favorites and a bold blue **⌂** for paths that exactly match configured roots; both symbols remain colored on the highlighted row and appear together when applicable. A configured-root scope includes its root directory as a selectable row, while Tab-browsing shows only descendants. Ctrl+F toggles the highlighted favorite; in Favorites, a removed item stays highlighted temporarily so Ctrl+F immediately restores it. It disappears on the next search or scope change if not restored. After filtering and highlighting a project (for example, searching `sandbox` and highlighting `~/ZedProjects/pi-sandbox`), Ctrl+R adds that project's **full path** as a peer root, without changing the active scope. The **+ Root** mouse button instead accepts an arbitrary full directory path. Adding a root does not rescan All, since All already searches your home directory; configured roots are narrower scopes (and can also target paths outside your home directory). Ctrl+Shift+R removes a highlighted configured root immediately (including from All or Favorites), so it can undo Ctrl+R without changing scope. If no root is highlighted but a root scope is active, a small confirmation offers to remove that scope's root. Mouse controls are available for these actions. Modified Ctrl+Shift+R requires a terminal that reports Shift separately; the − Root button remains available otherwise.

New workspaces are always created, even when that exact path is already open. They stay unfocused by default. Names use the shortest path suffix that distinguishes currently open workspaces; repeated identical paths receive `#1`, `#2`, and so on. Existing workspace labels are changed only when the new workspace makes their prior shortest suffix ambiguous.

## Configuration

The configuration lives at `$(herdr plugin config-dir herdr-workspaces-manager)/config.json`. It is created on first picker launch and can be edited manually. Existing `~/ZedProjects` and `~/PycharmProjects` directories are added as initial roots when present. Example:

```json
{
  "roots": ["~/ZedProjects", "~/PycharmProjects"],
  "favorites": ["~/ZedProjects/special-project"],
  "max_depth": 20,
  "focus_created": false,
  "keybindings": {
    "select": "shift+enter,ctrl+t",
    "scope": "ctrl+g",
    "scope_backward": "ctrl+shift+g",
    "enter_directory": "tab",
    "parent_directory": "shift+tab",
    "favorite": "ctrl+f",
    "add_root": "ctrl+r",
    "remove_root": "ctrl+shift+r",
    "help": "?"
  },
  "excluded_names": [".*", "node_modules", "vendor", "target", "dist", "build", ".venv", "venv", "__pycache__", ".next", "coverage", "site-packages"]
}
```

Roots and favorites can also be changed inside the picker. Existing configs receive the new scope-navigation bindings automatically; configs using the old generated `ctrl+d` removal binding are also migrated when the picker next opens. Herdr's opening shortcut is configured separately in `[keys]`; the picker shortcuts above are comma-separated key chords. Enter creates or opens an editable batch review; Esc returns to the picker from a review or closes the picker itself. Discovery searches at most 20 levels deep by default, honors applicable `.gitignore`/global Git ignore rules, prunes hidden folders and configured artifact names, and does not follow directory symlinks. Favorites appear in their own scope, including paths outside your home directory; they are not injected into other scopes. Discovery runs in the background; at most 1,000 fuzzy matches are displayed at once. The plugin acts on the **local** Herdr session.

## Development

Run `task check` for formatting and Clippy, `task build` to compile the release binary, or `task release-check` for both before tagging. No runtime dependency manager is used.

## Releases

Set the same version in `Cargo.toml` and `herdr-plugin.toml`, then push a tag such as `v0.1.0`. `.github/workflows/release.yml` rejects a tag that does not match both files. It builds four native target binaries, packages each as `herdr-workspaces-manager-v<version>-<target>.tar.gz` with a `.sha256` companion, and publishes the assets to the GitHub release only when all builds succeed. The workflow can also be dispatched manually against an existing version tag. `target/` and installed `bin/` binaries are not committed.

## Herdr marketplace

The [Herdr plugin marketplace](https://herdr.dev/plugins/) automatically indexes public GitHub repositories whose default branch contains a valid `herdr-plugin.toml` and has the `herdr-plugin` GitHub topic. No separate submission is needed; the index refreshes about every 30 minutes. Marketplace listings are community-contributed, not endorsed or security-reviewed by Herdr. Installation remains `herdr plugin install dardT/herdr-workspaces-manager`.
