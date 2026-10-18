# Contributing

This is a Rust plugin. Install Rust/Cargo and [Task](https://taskfile.dev/); run `task check` before submitting changes.

## Publish a release

Publishing requires repository access and an authenticated [GitHub CLI](https://cli.github.com/). Choose a **new** version (for example, `0.1.2`), update it in `Cargo.toml` and `herdr-plugin.toml`, then run:

```sh
export VERSION=0.1.2
cargo check                     # Refreshes Cargo.lock
task release-check              # Format, Clippy, release build
git diff --check
git add Cargo.toml Cargo.lock herdr-plugin.toml
git commit -m "build: prepare v${VERSION} release"
git push origin main
git tag "v${VERSION}"
git push origin "v${VERSION}"
```

The pushed tag triggers builds and no-Cargo installation checks for all four supported platforms. Confirm the **entire** workflow succeeds and the release contains four archives with matching SHA-256 files (replace `RUN_ID` with the ID from the list):

```sh
gh run list --workflow release.yml --limit 5
gh run watch RUN_ID --exit-status
gh release view "v${VERSION}" --json assets --jq '.assets[].name'
```

Never move a published tag; use a new version for fixes. The installer downloads binaries by the version in `herdr-plugin.toml`.
