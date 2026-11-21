#!/bin/sh
# browser-entry.sh SOURCE OUT: write the Primitive Browser library entry (SOURCE, deploy/Browser.sh) with its icon in
# the header: the light form (# Icon:) and the dark form (# IconDark:), both after the Author line. The icons are the
# square logo on a white (light) or black (dark) tile, without any text. Every release and install writes the entry
# through this script, so the header is the same everywhere.
set -eu
cd "$(dirname "$0")/.."
src=${1:?usage: browser-entry.sh SOURCE OUT}
out=${2:?usage: browser-entry.sh SOURCE OUT}
light=assets/primitive-icon-tile.png
dark=assets/primitive-icon-tile-dark.png
[ -r "$light" ] && [ -r "$dark" ] || { echo "missing the browser icon (light or dark) in assets/" >&2; exit 1; }
icon="# Icon: data:image/png;base64,$(base64 < "$light" | tr -d '\n')"
icon_dark="# IconDark: data:image/png;base64,$(base64 < "$dark" | tr -d '\n')"
awk -v icon="$icon" -v icon_dark="$icon_dark" '{ print } /^# Author:/ { print icon; print icon_dark }' "$src" > "$out"
chmod 700 "$out"
