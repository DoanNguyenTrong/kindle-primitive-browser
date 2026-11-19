#!/bin/sh
# Stage the installed files of Primitive Browser for the store package (build/stage): the same places as on the Kindle
# (browser/ and documents/). The packager (kindle-primitive-build) adds the manifest, the licence and the install notes.
#
#   sh scripts/stage.sh armhf      (after: cargo zigbuild --release -p kb-kindle --target armv7-unknown-linux-musleabihf)
set -eu
ROOT=$(cd "$(dirname "$0")/.." && pwd)
cd "$ROOT"
ARCH=${1:-armhf}
BIN=target/armv7-unknown-linux-musleabihf/release/kb-kindle
[ "$ARCH" = armhf ] || { echo "only armhf is built for now" >&2; exit 1; }
[ -f "$BIN" ] || { echo "build the device binary first (see the comment above)" >&2; exit 1; }

STAGE=build/stage
rm -rf "$STAGE"
mkdir -p "$STAGE/browser/fonts" "$STAGE/browser/bin/licenses" "$STAGE/documents"
cp "$BIN" "$STAGE/browser/kb-kindle"
cp assets/primitive-icon.png "$STAGE/browser/icon.png"
cp assets/fonts/*.ttf assets/fonts/NOTO-LICENSE.txt "$STAGE/browser/fonts/"
cp assets/device/armv7/fbink "$STAGE/browser/bin/"
cp assets/device/README.md "$STAGE/browser/bin/README.md"
cp assets/device/licenses/*.txt "$STAGE/browser/bin/licenses/"
# The library entry carries its icon in the header, after the Author line.
ICON_LINE="# Icon: data:image/png;base64,$(base64 < assets/primitive-icon.png | tr -d '\n')"
awk -v icon="$ICON_LINE" '{ print } /^# Author:/ { print icon }' deploy/Browser.sh > "$STAGE/documents/Primitive Browser.sh"
echo "staged $STAGE"
