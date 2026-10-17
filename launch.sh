#!/bin/sh
set -eu
"${HERDR_BIN_PATH:-herdr}" plugin pane open --plugin herdr-workspaces-manager --entrypoint picker
