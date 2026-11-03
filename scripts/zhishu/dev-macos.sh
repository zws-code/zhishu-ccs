#!/bin/sh
# Run `tauri dev` with an Apple-issued code signature on macOS.
#
# Without this, every rebuild gets a new ad-hoc code identity and macOS asks
# for the login keychain password again whenever the app reads a saved
# credential. Self-signed identities also use cdhash Keychain partitions.
# See docs/zhishu/development/macos-dev-signing.md for certificate setup.
set -eu

root=$(CDPATH= cd -- "$(dirname -- "$0")/../.." && pwd)

# Cargo reuses this entry point to sign and launch each rebuilt executable.
if [ "${1:-}" = "--run" ]; then
  shift
  if [ "$#" -eq 0 ]; then
    echo "dev-macos: missing executable path" >&2
    exit 1
  fi
  node "$root/scripts/zhishu/macos-dev-sign.mjs" "$1"
  exec "$@"
fi

identity=$(node "$root/scripts/zhishu/macos-dev-sign.mjs" --identity)
host=$(rustc -vV | sed -n 's/^host: //p')
if [ -z "$host" ]; then
  echo "dev-macos: unable to determine the Rust host target" >&2
  exit 1
fi

runner_var="CARGO_TARGET_$(printf '%s' "$host" | tr 'a-z-' 'A-Z_')_RUNNER"

exec env \
  "$runner_var=/bin/sh $root/scripts/zhishu/dev-macos.sh --run" \
  "MACOS_DEV_SIGNING_IDENTITY=$identity" \
  pnpm tauri dev "$@"
