#!/usr/bin/env bash
# Packs Akai's SDK, at the version in sdk.pin, as the one archive an installer of MP Forge carries.
# MP Forge unpacks it on the musician's computer the first time it is opened.
#
# It is packed from what Akai committed, so nothing built or fetched on this computer goes along, and with the
# line ends Akai wrote, whatever Git is set to do with them here.
set -euo pipefail

PROJECT_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
SDK_DIR="$PROJECT_ROOT/vendor/mpc-plugin-builder-sdk"
CARRIED_DIR="$PROJECT_ROOT/app/carried"
FOLDER_IN_THE_ARCHIVE="akai-sdk"

# shellcheck source=../sdk.pin
source "$PROJECT_ROOT/sdk.pin"

"$PROJECT_ROOT/scripts/fetch-sdk.sh"
mkdir -p "$CARRIED_DIR"
git -C "$SDK_DIR" -c core.autocrlf=false archive --format=tar.gz --prefix="$FOLDER_IN_THE_ARCHIVE/" "$SDK_COMMIT" \
  >"$CARRIED_DIR/$FOLDER_IN_THE_ARCHIVE.tar.gz"
echo "Packed Akai's SDK ${SDK_COMMIT:0:7} into app/carried/$FOLDER_IN_THE_ARCHIVE.tar.gz"
