# Changelog

## v0.2.0

### Added

- Open a project in the current Herdr workspace with **Ctrl+T**. The picker splits an eligible pane in the invoking tab and opens a new tab when the configurable pane grid is full (two columns by two rows by default).
- Open batches in either new workspaces (**Enter**) or panes and tabs in the current workspace (**Ctrl+T**), with a placement preview and a review step before creation.
- Search `/Users/Shared` on macOS, including accessible symlinked project directories, and match absolute (`/`) or home-relative (`~/`) path prefixes.
- Opt into searching hidden directories with `include_hidden`, while keeping `.git` metadata excluded. Browse parent directories up to the filesystem root.

### Changed

- **Ctrl+Space** is the sole default batch-selection shortcut, replacing Shift+Enter and Ctrl+T. Plain Space remains available for search. Existing generated keybindings migrate automatically; custom bindings are preserved.
- Successfully opened panes and tabs now appear in Recent alongside created workspaces, including items opened before a partial batch failure.

### Fixed

- Resolve the invoking workspace and tab from Herdr's plugin-popup context, which does not provide the normal pane environment variables. This fixes Ctrl+T failing with “No invoking Herdr workspace available.”
