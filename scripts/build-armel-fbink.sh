#!/bin/sh
# Cross-build the Kindle image fallback with no dynamic-loader dependency.
set -eu

root=$(cd "$(dirname "$0")/.." && pwd)
output=${1:-"$root/build/device/armel"}
mkdir -p "$output"
output=$(cd "$output" && pwd)
work=$(mktemp -d "${TMPDIR:-/tmp}/primitive-fbink.XXXXXXXX")
pending=
trap 'rm -rf "$work"; if [ -n "$pending" ]; then rm -f "$pending"; fi' 0
trap 'exit 1' HUP INT TERM
export LC_ALL=C SOURCE_DATE_EPOCH=0

extract_source() {
    name=$1
    url=$2
    digest=$3
    destination=$4
    archive=$work/$name.tar
    curl -fsSL --proto '=https' --proto-redir '=https' --retry 2 --max-time 90 "$url" -o "$archive"
    if command -v sha256sum >/dev/null; then
        actual=$(sha256sum "$archive")
    else
        actual=$(shasum -a 256 "$archive")
    fi
    [ "${actual%% *}" = "$digest" ] || { echo "Source checksum mismatch: $name" >&2; exit 1; }
    mkdir -p "$destination"
    tar -xf "$archive" --strip-components=1 -C "$destination"
}

extract_source fbink https://codeload.github.com/NiLuJe/FBInk/tar.gz/92e127008145b2a22fba7c59815d810d716310dd \
    c2e08111e158dca6006229e62c643e907113a3c45b1e211698027f76d77051c2 "$work/fbink"
# These revisions are the gitlinks in FBInk's source tree at the revision above.
extract_source stb https://codeload.github.com/NiLuJe/stb/tar.gz/b034a871ead7eeee035d9b7dd2fffcf0370b239d \
    4855f6ec3c7418b6cbe2e623966ddce6253527939d86628f0affa5329cc4f1bb "$work/fbink/stb"
extract_source font8x8 https://codeload.github.com/NiLuJe/font8x8/tar.gz/fa74831b9076805271cdedc0ee12137aa548e524 \
    e48e7d5bb47fba400b0b28db30b09aed0cf499679b3d69274bc321d8852fd07e "$work/fbink/font8x8"

# FBInk's minimal Kindle CLI requires bitmap support. The browser also needs its BMP decoder.
(
    cd "$work/fbink"
    zig cc -target arm-linux-musleabi -mcpu=generic+v7a -mfloat-abi=soft -I. -O2 -DNDEBUG \
        -DFBINK_FOR_KINDLE -DFBINK_MINIMAL -DFBINK_WITH_DRAW -DFBINK_WITH_BITMAP -DFBINK_WITH_IMAGE \
        '-DFBINK_VERSION="1.25.0-g92e1270"' -Wno-unknown-warning-option -Wno-pass-failed \
        -static -Wl,--build-id=none,--strip-all -o "$work/fbink-cli" \
        fbink.c fbink_cmd.c cutef8/utf8.c cutef8/dfa.c qimagescale/qimagescale.c -lm
)

zig_lib=$(zig env | sed -n 's/^    \.lib_dir = "\(.*\)",$/\1/p')
[ -n "$zig_lib" ] || { echo "Could not locate Zig's musl copyright notice." >&2; exit 1; }
install -m 644 "$zig_lib/libc/musl/COPYRIGHT" "$output/MUSL-LICENSE.txt"
install -m 644 "$work/fbink/CREDITS" "$output/FBINK-CREDITS.txt"
install -m 644 "$work/fbink/stb/LICENSE" "$output/STB-LICENSE.txt"
install -m 644 "$work/fbink/qimagescale/QIMAGETRANSFORM_LICENSE.txt" "$output/QIMAGETRANSFORM-LICENSE.txt"
pending=$(mktemp "$output/.fbink.XXXXXXXX")
install -m 755 "$work/fbink-cli" "$pending"
mv "$pending" "$output/fbink"
printf 'Built %s\n' "$output/fbink"
