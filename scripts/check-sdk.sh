#!/usr/bin/env bash
# Proves the pinned Akai SDK works on this machine:
# builds each of its example plugins for both MPC chip types, then runs each through the host simulator.
# Exit code 0 means every example built and passed. Logs and rendered audio land in build/sdk-check/.
set -euo pipefail

PROJECT_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
SDK_DIR="$PROJECT_ROOT/vendor/mpc-plugin-builder-sdk"
HOST_SIMULATOR="$SDK_DIR/.sdk-tools/host_sim"
REPORT_DIR="$PROJECT_ROOT/build/sdk-check"
EVERY_EXAMPLE=(SimpleSynth StereoDelay DrumSynth)
# Name examples as arguments to check only those; with no arguments, all are checked.
EXAMPLES=("${@:-${EVERY_EXAMPLE[@]}}")

# shellcheck source=../sdk.pin
source "$PROJECT_ROOT/sdk.pin"

stop_with() {
  echo "check-sdk: $1" >&2
  exit 1
}

fetch_sdk_if_missing() {
  [[ -d "$SDK_DIR/.git" ]] && return
  echo "Fetching Akai's SDK into vendor/ ..."
  git clone --quiet "$SDK_REPOSITORY" "$SDK_DIR"
  git -C "$SDK_DIR" checkout --quiet "$SDK_COMMIT"
}

require_pinned_sdk() {
  local checked_out
  checked_out="$(git -C "$SDK_DIR" rev-parse HEAD)"
  [[ "$checked_out" == "$SDK_COMMIT" ]] ||
    stop_with "SDK is at $checked_out but sdk.pin expects $SDK_COMMIT"
}

build_example() {
  python3 "$SDK_DIR/tools/build.py" "$SDK_DIR/examples/$1" >"$REPORT_DIR/$1.build.log" 2>&1
}

native_library_of() {
  find "$SDK_DIR/examples/$1/build/native" -maxdepth 1 -type f \
    \( -name '*.dylib' -o -name '*.so' -o -name '*.dll' \) | head -n 1
}

simulate_example() {
  "$HOST_SIMULATOR" "$(native_library_of "$1")" test --wav "$REPORT_DIR/$1.wav" \
    >"$REPORT_DIR/$1.simulation.log" 2>&1
}

simulator_verdict_of() {
  grep '^Result:' "$REPORT_DIR/$1.simulation.log" | sed 's/^Result: *[A-Z]* *//'
}

check_example() {
  if ! build_example "$1"; then
    echo "FAIL  $1  did not build (see build/sdk-check/$1.build.log)"
    return 1
  fi
  if ! simulate_example "$1"; then
    echo "FAIL  $1  $(simulator_verdict_of "$1") (see build/sdk-check/$1.simulation.log)"
    return 1
  fi
  echo "PASS  $1  $(simulator_verdict_of "$1")"
}

check_every_example() {
  local failures=0
  for example in "${EXAMPLES[@]}"; do
    check_example "$example" || failures=$((failures + 1))
  done
  return "$failures"
}

main() {
  fetch_sdk_if_missing
  require_pinned_sdk
  mkdir -p "$REPORT_DIR"
  echo "Checking Akai SDK ${SDK_COMMIT:0:7} ..."
  if check_every_example; then
    echo "All ${#EXAMPLES[@]} examples built and passed the host simulator."
  else
    stop_with "at least one example failed"
  fi
}

main
