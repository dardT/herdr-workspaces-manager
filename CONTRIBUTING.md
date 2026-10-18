# Contributing

This plugin is a standalone Rust binary. Install Rust/Cargo, [Task](https://taskfile.dev/), and the [GitHub CLI](https://cli.github.com/) to build and publish it. Authenticate `gh` and ensure you have push and release permissions for `dardT/herdr-workspaces-manager` before publishing.

## Local checks

```sh
task release-check
```

This checks formatting and Clippy, then builds the release binary.

## Publish a release

Choose a **new** semantic version. Do not move an existing published tag: the install script downloads assets by the version in `herdr-plugin.toml`. Finish and push any intended runtime changes before tagging; release binaries are built from the tag, not from subsequent commits on `main`.

1. Start from an up-to-date `main`, then bump both manifests. Replace `0.1.2` below with the version you are releasing:

   ```sh
   git switch main
   git pull --ff-only origin main
   export VERSION=0.1.2
   python3 - <<'PY'
   import os
   import re
   from pathlib import Path

   version = os.environ["VERSION"]
   for filename in ("Cargo.toml", "herdr-plugin.toml"):
       path = Path(filename)
       updated, count = re.subn(
           r'(?m)^version = "[^"]+"$',
           f'version = "{version}"',
           path.read_text(),
           count=1,
       )
       if count != 1:
           raise SystemExit(f"Expected one package version in {filename}")
       path.write_text(updated)
   PY
   cargo check
   ```

   `cargo check` refreshes the local package version in `Cargo.lock`. Check the diff: it should contain the intended version in both manifests and the lockfile.

2. Validate, commit, and push the version bump:

   ```sh
   task release-check
   git diff --check
   git diff -- Cargo.toml Cargo.lock herdr-plugin.toml
   git add Cargo.toml Cargo.lock herdr-plugin.toml
   git commit -m "build: prepare v${VERSION} release"
   git push origin main
   ```

   Include any other release-related changes in the commit if needed. The release-freshness workflow rejects runtime changes made after the latest release tag without a version bump.

3. Tag **that exact commit** and push the tag to start `.github/workflows/release.yml`:

   ```sh
   git tag "v${VERSION}"
   test "$(git rev-parse "v${VERSION}")" = "$(git rev-parse HEAD)"
   git push origin "v${VERSION}"
   ```

4. Find the tag-triggered run, then watch it to completion (replace `<run-id>` with the displayed ID):

   ```sh
   gh run list --workflow release.yml --limit 5
   gh run watch <run-id> --exit-status
   ```

   The workflow checks tag/version consistency, builds four target binaries, publishes their SHA-256 files, and smoke-tests installation **without Cargo** on each target platform. A successful build alone is not enough; the full release workflow must pass.

5. Confirm the public release has **four `.tar.gz` archives and four `.sha256` files**, is neither a draft nor a prerelease, and points at the intended tag:

   ```sh
   gh release view "v${VERSION}" --json url,isDraft,isPrerelease,assets --jq '{url,isDraft,isPrerelease,assets:[.assets[].name]}'
   git ls-remote origin "refs/tags/v${VERSION}"
   ```

   If a build, upload, or smoke check fails, investigate before announcing the release. Fixes require a new version and tag rather than force-moving a published tag.
