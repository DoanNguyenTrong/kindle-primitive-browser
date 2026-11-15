#!/bin/sh
# Copy the browser onto a mounted Kindle (USB mass storage, /Volumes/Kindle).
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

[ -d "$KINDLE/documents" ] || { echo "Kindle not mounted at $KINDLE" >&2; exit 1; }
[ -f "$BIN" ] || { echo "binary not found: $BIN" >&2; exit 1; }
file "$BIN" | grep -q "ARM" || { echo "not an ARM binary: $BIN" >&2; exit 1; }

mkdir -p "$KINDLE/browser"
cp "$BIN" "$KINDLE/browser/kb-kindle"
cp "$HERE/deploy/Browser.sh" "$KINDLE/documents/KB Browser.sh"
[ -f "$KINDLE/browser/start.txt" ] || echo "https://news.ycombinator.com/" > "$KINDLE/browser/start.txt"

# Remove only the AppleDouble metadata macOS creates for our own files on the FAT volume.
# (Nothing else in documents/ is touched.)
rm -f "$KINDLE/browser"/._* "$KINDLE/documents/._KB Browser.sh"
sync
echo "installed: $KINDLE/browser/kb-kindle ($(wc -c < "$KINDLE/browser/kb-kindle") bytes)"
echo "start page: $(cat "$KINDLE/browser/start.txt")"
echo "launcher: documents/KB Browser.sh"
