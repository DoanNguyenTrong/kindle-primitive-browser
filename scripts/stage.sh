#!/bin/sh
# Stage the installed files of Primitive Browser for the store package (build/stage): the same places as on the Kindle
# (browser/ and documents/). The packager (kindle-primitive-build) adds the manifest, the licence and the install notes.
#
#   sh scripts/stage.sh [armhf|armel] [STAGE_DIR]
set -eu
ROOT=$(cd "$(dirname "$0")/.." && pwd)
cd "$ROOT"
ARCH=${1:-armhf}
case "$ARCH" in
    armhf) TARGET=armv7-unknown-linux-musleabihf; HELPERS=assets/device/armv7 ;;
    armel) TARGET=armv7-unknown-linux-musleabi; HELPERS=build/device/armel ;;
    *) echo "unsupported package architecture: $ARCH" >&2; exit 1 ;;
esac
BIN=${CARGO_TARGET_DIR:-target}/$TARGET/release/kb-kindle
[ -x "$BIN" ] || { echo "build the device binary first: $BIN" >&2; exit 1; }
file "$BIN" | grep -q 'ELF.*ARM' || { echo "not an ARM ELF binary: $BIN" >&2; exit 1; }
if [ "$ARCH" = armel ]; then
    recipe=scripts/build-armel-fbink.sh
    if [ ! -x "$HELPERS/fbink" ] || [ ! -r "$HELPERS/MUSL-LICENSE.txt" ] || [ ! -r "$HELPERS/FBINK-CREDITS.txt" ] ||
        [ ! -r "$HELPERS/STB-LICENSE.txt" ] || [ ! -r "$HELPERS/QIMAGETRANSFORM-LICENSE.txt" ] ||
        [ "$recipe" -nt "$HELPERS/fbink" ]; then
        sh "$recipe"
    fi
fi

if [ "$#" -ge 2 ]; then
    STAGE=$2
    mkdir "$STAGE"
else
    STAGE=build/stage
    rm -rf "$STAGE"
fi
mkdir -p "$STAGE/browser/fonts" "$STAGE/browser/bin/licenses" "$STAGE/documents"
cp "$BIN" "$STAGE/browser/kb-kindle"
cp assets/primitive-icon.png "$STAGE/browser/icon.png"
cp assets/fonts/*.ttf assets/fonts/NOTO-LICENSE.txt "$STAGE/browser/fonts/"
install -m 755 "$HELPERS/fbink" "$STAGE/browser/bin/fbink"
cp assets/device/README.md "$STAGE/browser/bin/README.md"
cp assets/device/licenses/*.txt "$STAGE/browser/bin/licenses/"
if [ "$ARCH" = armel ]; then
    cp "$HELPERS/MUSL-LICENSE.txt" "$HELPERS/FBINK-CREDITS.txt" "$HELPERS/STB-LICENSE.txt" \
        "$HELPERS/QIMAGETRANSFORM-LICENSE.txt" "$STAGE/browser/bin/licenses/"
fi
# The library entry carries its icon in the header, after the Author line.
ICON_LINE="# Icon: data:image/png;base64,$(base64 < assets/primitive-icon.png | tr -d '\n')"
awk -v icon="$ICON_LINE" '{ print } /^# Author:/ { print icon }' deploy/Browser.sh > "$STAGE/documents/Primitive Browser.sh"
chmod 755 "$STAGE/documents/Primitive Browser.sh"
echo "staged $STAGE"
