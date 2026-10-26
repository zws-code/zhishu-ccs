#!/bin/sh
# Cargo target runner used by scripts/dev-macos.sh.
#
# Cargo invokes this with the built executable and its arguments. Re-signing
# with an Apple-issued identity keeps both the designated requirement and
# Keychain team partition stable across rebuilds.
set -eu

if [ "$#" -eq 0 ]; then
  echo "macos-dev-sign-runner: missing executable path" >&2
  exit 1
fi

root=$(CDPATH= cd -- "$(dirname -- "$0")/.." && pwd)
node "$root/scripts/macos-dev-sign.mjs" "$1"

exec "$@"
