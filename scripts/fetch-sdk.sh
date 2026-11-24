#!/usr/bin/env bash
# Fetches Akai's SDK into vendor/ at the version in sdk.pin, and moves an SDK already there to that version.
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

# After sdk.pin has changed, the SDK already in vendor/ is still at the version before.
move_sdk_to_the_pinned_version() {
  [[ "$(git -C "$SDK_DIR" rev-parse HEAD)" == "$SDK_COMMIT" ]] && return
  echo "Moving Akai's SDK in vendor/ to the version in sdk.pin ..."
  git -C "$SDK_DIR" fetch --quiet origin
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
move_sdk_to_the_pinned_version
require_pinned_sdk
