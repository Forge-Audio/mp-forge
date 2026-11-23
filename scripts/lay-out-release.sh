#!/usr/bin/env bash
# Lays out the files of one release of MP Forge: the installers, the one extra file a Mac updates from, and
# latest.json, which a copy of MP Forge reads to learn of a newer version and where its update is.
#
#   scripts/lay-out-release.sh <folder the builds left their files in> <folder for the release> <version> <owner/repository>
#
# Every file is given the name GitHub would give it anyway, with a dot for each space, because latest.json has
# to say where each one will be once the release is published. A file or a signature that is missing stops the
# whole thing: a copy of MP Forge reads all of latest.json before it trusts any of it.
set -euo pipefail

BUILT_DIR="$1"
RELEASE_DIR="$2"
VERSION="$3"
REPOSITORY="$4"

DOWNLOADS="https://github.com/$REPOSITORY/releases/download/v$VERSION"
# A Mac's update is the app itself, packed. The builder leaves the version out of its name.
MAC_UPDATE_ENDING=".app.tar.gz"
PLATFORMS="$(mktemp)"
trap 'rm -f "$PLATFORMS"' EXIT

stop() {
  echo "$1" >&2
  exit 1
}

# Finds the one built file whose name ends this way, and the name it is to have in the release. Both are left
# in the two names below, because a stop inside a $(...) would end only that, and not the laying out.
BUILT_FILE=""
NAME_IN_THE_RELEASE=""
find_built_file_ending() {
  BUILT_FILE="$(find "$BUILT_DIR" -type f -name "*$1")"
  [[ -n "$BUILT_FILE" && "$(wc -l <<<"$BUILT_FILE")" -eq 1 ]] || stop "Expected one built file ending in $1, and found: ${BUILT_FILE:-none}"
  NAME_IN_THE_RELEASE="$(basename "$BUILT_FILE")"
  if [[ "$1" == "$MAC_UPDATE_ENDING" ]]; then
    NAME_IN_THE_RELEASE="${NAME_IN_THE_RELEASE%"$MAC_UPDATE_ENDING"}_${VERSION}_universal$MAC_UPDATE_ENDING"
  fi
  NAME_IN_THE_RELEASE="${NAME_IN_THE_RELEASE// /.}"
}

# Puts the built file whose name ends this way in the release.
add_file_ending() {
  find_built_file_ending "$1"
  cp "$BUILT_FILE" "$RELEASE_DIR/$NAME_IN_THE_RELEASE"
}

# Puts the built file whose name ends this way in the release, and offers it as the update for copies of
# MP Forge of the kinds that follow, signed as the build signed it.
offer_file_ending() {
  local kind
  add_file_ending "$1"
  shift
  [[ -s "$BUILT_FILE.sig" ]] || stop "$(basename "$BUILT_FILE") was not signed. Is the secret TAURI_SIGNING_PRIVATE_KEY set?"
  for kind in "$@"; do
    jq -n --arg kind "$kind" --arg url "$DOWNLOADS/$NAME_IN_THE_RELEASE" --rawfile signature "$BUILT_FILE.sig" \
      '{ ($kind): { signature: ($signature | rtrimstr("\n")), url: $url } }' >>"$PLATFORMS"
  done
}

write_latest_json() {
  jq -s --arg version "$VERSION" '{ version: $version, platforms: add }' "$PLATFORMS" >"$RELEASE_DIR/latest.json"
}

mkdir -p "$RELEASE_DIR"
add_file_ending .dmg
offer_file_ending "$MAC_UPDATE_ENDING" darwin-aarch64 darwin-x86_64
offer_file_ending -setup.exe windows-x86_64
offer_file_ending .AppImage linux-x86_64
offer_file_ending .deb linux-x86_64-deb
write_latest_json
echo "Laid out MP Forge $VERSION in $RELEASE_DIR:"
ls "$RELEASE_DIR"
