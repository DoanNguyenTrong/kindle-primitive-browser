#!/bin/sh
# Copy Primitive Browser onto the Kindle over SSH (Wi-Fi), so the USB cable is not needed.
#
#   deploy/install-ssh.sh [path/to/kb-kindle-binary]
#
# Needs the SSH server running on the Kindle: open Diagnostics and tap Start SSH. The root password is
# asked for once, because every copy below shares one connection. Nothing secret is stored in this repository.
#
# Environment: KINDLE_HOST (default 192.168.1.28), KINDLE_SSH_PORT (default 2222).
#
# Each file is written under a temporary name and then renamed into place, so the browser that is running
# is not disturbed. The new binary is checked against the local copy (SHA-256) after it is written.
set -eu

HERE=$(cd "$(dirname "$0")/.." && pwd)
HOST=${KINDLE_HOST:-192.168.1.28}
PORT=${KINDLE_SSH_PORT:-2222}
BIN=${1:-$HERE/target/armv7-unknown-linux-musleabihf/release/kb-kindle}
# Library launcher name. The launcher was installed as "KB Browser.sh" before the rename.
LAUNCHER="Primitive Browser.sh"
OLD_LAUNCHER="KB Browser.sh"
ICON_PNG="$HERE/assets/primitive-launcher-icon.png"
LOGO_PNG="$HERE/assets/primitive-icon.png"

[ -f "$BIN" ] || { echo "binary not found: $BIN" >&2; exit 1; }
file "$BIN" | grep -q "ARM" || { echo "not an ARM binary: $BIN" >&2; exit 1; }
[ -f "$ICON_PNG" ] || { echo "icon not found: $ICON_PNG" >&2; exit 1; }
[ -f "$LOGO_PNG" ] || { echo "logo not found: $LOGO_PNG" >&2; exit 1; }

# The launcher carries its icon in the header, as install.sh does.
ICON_LINE="# Icon: data:image/png;base64,$(base64 < "$ICON_PNG" | tr -d '\n')"
LAUNCHER_TMP=$(mktemp)
awk -v icon="$ICON_LINE" '{ print } /^# Author:/ { print icon }' "$HERE/deploy/Browser.sh" > "$LAUNCHER_TMP"

# One shared connection: a single password prompt for all the steps below.
CTL="/tmp/kbssh-$$"
SSH="ssh -p $PORT -o ControlMaster=auto -o ControlPath=$CTL -o ControlPersist=120 -o StrictHostKeyChecking=accept-new root@$HOST"
cleanup() {
    ssh -p "$PORT" -o ControlPath="$CTL" -O exit "root@$HOST" 2>/dev/null || true
    rm -f "$LAUNCHER_TMP"
}
trap cleanup EXIT INT TERM

echo "connecting to root@$HOST:$PORT"
$SSH 'mkdir -p /mnt/us/browser /mnt/us/documents'

# upload LOCAL REMOTE: write to REMOTE.new, flush, then rename it over REMOTE.
upload() {
    $SSH "cat > '$2.new' && sync && mv '$2.new' '$2'" < "$1"
    echo "copied: $2"
}
upload "$BIN" /mnt/us/browser/kb-kindle
upload "$LAUNCHER_TMP" "/mnt/us/documents/$LAUNCHER"
upload "$LOGO_PNG" /mnt/us/browser/icon.png

# Copy LOCAL to REMOTE when the device's copy differs (checked by SHA-256), so an unchanged install copies
# nothing. Used for the fonts, the device tools (FBInk, Dropbear) and the library they need.
sync_file() {
    local_file_sum=$(shasum -a 256 "$1" | cut -c1-64)
    remote_file_sum=$($SSH "sha256sum '$2' 2>/dev/null" | cut -c1-64 || true)
    if [ "$local_file_sum" != "$remote_file_sum" ]; then
        upload "$1" "$2"
    fi
}

# The fonts the browser draws with (Noto, SIL Open Font License), with their license.
$SSH 'mkdir -p /mnt/us/browser/fonts'
for font in "$HERE"/assets/fonts/*.ttf "$HERE/assets/fonts/NOTO-LICENSE.txt"; do
    sync_file "$font" "/mnt/us/browser/fonts/$(basename "$font")"
done

# The device tools: FBInk draws the e-ink updates, Dropbear is the SSH server. Dropbear needs zlib, in lib/.
# The sources and licenses are in assets/device/README.md.
$SSH 'mkdir -p /mnt/us/browser/bin /mnt/us/browser/lib'
sync_file "$HERE/assets/device/armv7/fbink" /mnt/us/browser/bin/fbink
sync_file "$HERE/assets/device/armv7/dropbear" /mnt/us/browser/bin/dropbear
sync_file "$HERE/assets/device/armv7/libz.so.1" /mnt/us/browser/lib/libz.so.1
$SSH 'chmod 755 /mnt/us/browser/bin/fbink /mnt/us/browser/bin/dropbear 2>/dev/null || true'

local_sum=$(shasum -a 256 "$BIN" | cut -c1-64)
remote_sum=$($SSH 'sha256sum /mnt/us/browser/kb-kindle' | cut -c1-64)
if [ "$local_sum" != "$remote_sum" ]; then
    echo "checksum mismatch: local $local_sum, device $remote_sum" >&2
    exit 1
fi
echo "installed: /mnt/us/browser/kb-kindle ($(wc -c < "$BIN") bytes, sha256 $local_sum)"
echo "launcher: documents/$LAUNCHER"
echo "restart Primitive Browser on the Kindle to load the new build."
