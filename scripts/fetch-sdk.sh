#!/usr/bin/env bash
# Fetches Akai's SDK into vendor/ at the version in sdk.pin, unless it is there already.
# Exit code 0 means the SDK in vendor/ is at exactly that version.
set -euo pipefail

PROJECT_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
SDK_DIR="$PROJECT_ROOT/vendor/mpc-plugin-builder-sdk"

# shellcheck source=../sdk.pin
source "$PROJECT_ROOT/sdk.pin"

fetch_sdk_if_missing() {
  [[ -d "$SDK_DIR/.git" ]] && return
  echo "Fetching Akai's SDK into vendor/ ..."
  git clone --quiet "$SDK_REPOSITORY" "$SDK_DIR"
  git -C "$SDK_DIR" checkout --quiet "$SDK_COMMIT"
}

require_pinned_sdk() {
  local checked_out
  checked_out="$(git -C "$SDK_DIR" rev-parse HEAD)"
  [[ "$checked_out" == "$SDK_COMMIT" ]] && return
  echo "fetch-sdk: SDK is at $checked_out but sdk.pin expects $SDK_COMMIT" >&2
  exit 1
}

fetch_sdk_if_missing
require_pinned_sdk
