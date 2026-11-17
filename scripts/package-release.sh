#!/bin/sh
# Package a release of Primitive Browser for people who do not build it.
#
#   scripts/package-release.sh
#
# Needs the ARMv7 device binary first:
#   cargo zigbuild --release -p kb-kindle --target armv7-unknown-linux-musleabihf
#
# Writes dist/primitive-browser-<version>.zip and a matching .sha256 file. The zip holds two folders,
# browser/ and documents/, which are copied onto the root of the Kindle. See docs/INSTALL.md.
set -eu

ROOT=$(cd "$(dirname "$0")/.." && pwd)
cd "$ROOT"

VERSION=$(python3 - Cargo.toml <<'PY'
import re, sys
text = open(sys.argv[1]).read()
print(re.search(r'^version = "([0-9]+\.[0-9]+\.[0-9]+)"', text, re.M).group(1))
PY
)
BIN=target/armv7-unknown-linux-musleabihf/release/kb-kindle
NAME=primitive-browser-$VERSION

[ -f "$BIN" ] || { echo "build the device binary first (see the comment at the top of this script)" >&2; exit 1; }
if command -v file >/dev/null 2>&1; then
    file "$BIN" | grep -q ARM || { echo "not an ARM binary: $BIN" >&2; exit 1; }
fi
command -v zip >/dev/null 2>&1 || { echo "zip is needed to make the package" >&2; exit 1; }

STAGE=$(mktemp -d)
trap 'rm -rf "$STAGE"' EXIT
mkdir -p "$STAGE/$NAME/browser" "$STAGE/$NAME/documents"

cp "$BIN" "$STAGE/$NAME/browser/kb-kindle"
cp assets/primitive-icon.png "$STAGE/$NAME/browser/icon.png"
printf 'https://news.ycombinator.com/\n' > "$STAGE/$NAME/browser/start.txt"

# The fonts the browser draws with, and the device tools it runs: FBInk (screen updates) and Dropbear
# (SSH) with the zlib it needs. Their sources and licenses are in assets/device/README.md.
mkdir -p "$STAGE/$NAME/browser/fonts" "$STAGE/$NAME/browser/bin" "$STAGE/$NAME/browser/lib"
cp assets/fonts/*.ttf assets/fonts/NOTO-LICENSE.txt "$STAGE/$NAME/browser/fonts/"
cp assets/device/armv7/fbink assets/device/armv7/dropbear "$STAGE/$NAME/browser/bin/"
cp assets/device/armv7/libz.so.1 "$STAGE/$NAME/browser/lib/"
cp assets/device/README.md "$STAGE/$NAME/browser/bin/README.md"

# The launcher carries its icon in the header, as install.sh writes it.
ICON_LINE="# Icon: data:image/png;base64,$(base64 < assets/primitive-launcher-icon.png | tr -d '\n')"
awk -v icon="$ICON_LINE" '{ print } /^# Author:/ { print icon }' deploy/Browser.sh > "$STAGE/$NAME/documents/Primitive Browser.sh"
chmod 700 "$STAGE/$NAME/documents/Primitive Browser.sh"

cp docs/INSTALL.md "$STAGE/$NAME/INSTALL.md"

mkdir -p dist
rm -f "dist/$NAME.zip" "dist/$NAME.zip.sha256"
(cd "$STAGE" && zip -qr "$ROOT/dist/$NAME.zip" "$NAME")
(cd dist && { shasum -a 256 "$NAME.zip" 2>/dev/null || sha256sum "$NAME.zip"; } > "$NAME.zip.sha256")

echo "packaged dist/$NAME.zip"
cat "dist/$NAME.zip.sha256"
