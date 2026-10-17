#!/bin/sh
set -eu

root="${HERDR_PLUGIN_ROOT:-$(CDPATH= cd -- "$(dirname -- "$0")/.." && pwd)}"

# Local linked checkouts prefer the freshly built binary over an older installed copy.
if [ -x "$root/target/release/herdr-workspaces-manager" ]; then
  exec "$root/target/release/herdr-workspaces-manager" "$@"
fi
if [ -x "$root/bin/herdr-workspaces-manager" ]; then
  exec "$root/bin/herdr-workspaces-manager" "$@"
fi

echo "herdr-workspaces-manager: binary not found (run herdr/install.sh or task build)" >&2
exit 1
