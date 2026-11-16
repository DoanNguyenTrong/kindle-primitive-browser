#!/bin/sh
# Copy Primitive Browser onto a mounted Kindle (USB mass storage, /Volumes/Kindle).
#
#   deploy/install.sh [path/to/kb-kindle-binary]
#
# Default binary: target/armv7-unknown-linux-musleabihf/release/kb-kindle
# (build it with: cargo zigbuild --release -p kb-kindle --target armv7-unknown-linux-musleabihf)
#
# Writes only under /Volumes/Kindle/browser and /Volumes/Kindle/documents.
# Existing start.txt and data/ (cookies, bookmarks, cache, saved pages) are kept.
set -eu

HERE=$(cd "$(dirname "$0")/.." && pwd)
KINDLE=/Volumes/Kindle
BIN=${1:-$HERE/target/armv7-unknown-linux-musleabihf/release/kb-kindle}
# Library launcher name. The launcher was installed as "KB Browser.sh" before the rename.
LAUNCHER="Primitive Browser.sh"
OLD_LAUNCHER="KB Browser.sh"

[ -d "$KINDLE/documents" ] || { echo "Kindle not mounted at $KINDLE" >&2; exit 1; }
[ -f "$BIN" ] || { echo "binary not found: $BIN" >&2; exit 1; }
file "$BIN" | grep -q "ARM" || { echo "not an ARM binary: $BIN" >&2; exit 1; }

mkdir -p "$KINDLE/browser"
cp "$BIN" "$KINDLE/browser/kb-kindle"
cp "$HERE/deploy/Browser.sh" "$KINDLE/documents/$LAUNCHER"
# The logo, for the launcher icon.
cp "$HERE/assets/primitive-icon.png" "$KINDLE/browser/icon.png"
# Our launcher under its old name would show a second entry in the library.
if [ -f "$KINDLE/documents/$OLD_LAUNCHER" ]; then
    rm -f "$KINDLE/documents/$OLD_LAUNCHER" "$KINDLE/documents/._$OLD_LAUNCHER"
    echo "removed old launcher: documents/$OLD_LAUNCHER"
fi
[ -f "$KINDLE/browser/start.txt" ] || echo "https://news.ycombinator.com/" > "$KINDLE/browser/start.txt"

# Remove only the AppleDouble metadata macOS creates for our own files on the FAT volume.
# (Nothing else in documents/ is touched.)
rm -f "$KINDLE/browser"/._* "$KINDLE/documents/._$LAUNCHER"
sync
echo "installed: $KINDLE/browser/kb-kindle ($(wc -c < "$KINDLE/browser/kb-kindle") bytes)"
echo "start page: $(cat "$KINDLE/browser/start.txt")"
echo "launcher: documents/$LAUNCHER"
