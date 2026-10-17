#!/usr/bin/env bash
set -euo pipefail

ROOT="${HERDR_PLUGIN_ROOT:-$(cd "$(dirname "$0")/.." && pwd)}"
VERSION="$(awk -F '"' '$1 ~ /^version[[:space:]]*=/ { print $2; exit }' "$ROOT/herdr-plugin.toml")"

if [[ -z "$VERSION" ]]; then
  echo "herdr-workspaces-manager: missing version in herdr-plugin.toml" >&2
  exit 1
fi

build_from_source() {
  if ! command -v cargo >/dev/null 2>&1; then
    echo "herdr-workspaces-manager: no matching release binary and Cargo is not installed" >&2
    exit 1
  fi
  echo "herdr-workspaces-manager: building from source"
  (cd "$ROOT" && cargo build --release --locked)
  mkdir -p "$ROOT/bin"
  install -m 0755 "$ROOT/target/release/herdr-workspaces-manager" "$ROOT/bin/herdr-workspaces-manager"
}

# Use a repository override or the clone's origin when available; source archives
# lack Git metadata, so fall back to the canonical public repository.
REPO="${HERDR_WORKSPACES_MANAGER_REPO:-}"
if [[ -z "$REPO" ]] && command -v git >/dev/null 2>&1; then
  origin="$(git -C "$ROOT" remote get-url origin 2>/dev/null || true)"
  case "$origin" in
    https://github.com/*) REPO="${origin#https://github.com/}" ;;
    git@github.com:*) REPO="${origin#git@github.com:}" ;;
    ssh://git@github.com/*) REPO="${origin#ssh://git@github.com/}" ;;
  esac
  REPO="${REPO%.git}"
fi
REPO="${REPO:-dardT/herdr-workspaces-manager}"

case "$(uname -s)-$(uname -m)" in
  Darwin-arm64)  TARGET="aarch64-apple-darwin" ;;
  Darwin-x86_64) TARGET="x86_64-apple-darwin" ;;
  Linux-x86_64)  TARGET="x86_64-unknown-linux-gnu" ;;
  Linux-aarch64 | Linux-arm64) TARGET="aarch64-unknown-linux-gnu" ;;
  *) TARGET="" ;;
esac

if [[ ! "$REPO" =~ ^[A-Za-z0-9_.-]+/[A-Za-z0-9_.-]+$ ]] || [[ -z "$TARGET" ]]; then
  echo "herdr-workspaces-manager: repository or platform unavailable; using source build"
  build_from_source
  exit 0
fi

ASSET="herdr-workspaces-manager-v${VERSION}-${TARGET}.tar.gz"
BASE="https://github.com/${REPO}/releases/download/v${VERSION}"
TMP="$(mktemp -d)"
trap 'rm -rf "$TMP"' EXIT

if ! curl -fsSL --retry 3 --retry-delay 2 -o "$TMP/$ASSET" "$BASE/$ASSET" ||
   ! curl -fsSL --retry 3 --retry-delay 2 -o "$TMP/$ASSET.sha256" "$BASE/$ASSET.sha256"; then
  echo "herdr-workspaces-manager: release download unavailable; using source build"
  build_from_source
  exit 0
fi

# Do not run a binary when its downloaded archive fails verification.
if command -v shasum >/dev/null 2>&1; then
  (cd "$TMP" && shasum -a 256 -c "$ASSET.sha256")
else
  (cd "$TMP" && sha256sum -c "$ASSET.sha256")
fi
tar -xzf "$TMP/$ASSET" -C "$TMP" herdr-workspaces-manager
mkdir -p "$ROOT/bin"
install -m 0755 "$TMP/herdr-workspaces-manager" "$ROOT/bin/herdr-workspaces-manager"
echo "herdr-workspaces-manager: installed v${VERSION} (${TARGET})"
