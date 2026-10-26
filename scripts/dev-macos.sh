#!/bin/sh
# Run `tauri dev` with an Apple-issued code signature on macOS.
#
# Without this, every rebuild gets a new ad-hoc code identity and macOS asks
# for the login keychain password again whenever the app reads a saved
# credential. Self-signed identities also use cdhash Keychain partitions.
# See docs/macos-dev-signing.md for certificate setup.
set -eu

root=$(CDPATH= cd -- "$(dirname -- "$0")/.." && pwd)
identity=$(node "$root/scripts/macos-dev-sign.mjs" --identity)
host=$(rustc -vV | sed -n 's/^host: //p')
if [ -z "$host" ]; then
  echo "dev-macos: unable to determine the Rust host target" >&2
  exit 1
fi

runner_var="CARGO_TARGET_$(printf '%s' "$host" | tr 'a-z-' 'A-Z_')_RUNNER"

exec env \
  "$runner_var=/bin/sh $root/scripts/macos-dev-sign-runner.sh" \
  "MACOS_DEV_SIGNING_IDENTITY=$identity" \
  pnpm tauri dev "$@"
