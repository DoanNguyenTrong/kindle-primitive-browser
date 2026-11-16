#!/bin/sh
# Build and deploy a device release of Primitive Browser.
#
#   scripts/build-device.sh
#
# Every device build is a new patch version: 0.1.0 -> 0.1.1 -> 0.1.2, and so on. The version is
# the workspace version in Cargo.toml and is shown in the menu title. The bump is undone if the build or the install fails, so a failed deploy does not
# use up a version number.
set -eu

ROOT=$(cd "$(dirname "$0")/.." && pwd)
export PATH="$HOME/.rustup/toolchains/stable-aarch64-apple-darwin/bin:/opt/homebrew/bin:$PATH"
MANIFEST="$ROOT/Cargo.toml"

current=$(python3 - "$MANIFEST" <<'PY'
import re, sys
text = open(sys.argv[1]).read()
print(re.search(r'^version = "([0-9]+\.[0-9]+\.[0-9]+)"', text, re.M).group(1))
PY
)
next=$(python3 -c "import sys; a,b,c=sys.argv[1].split('.'); print(f'{a}.{b}.{int(c)+1}')" "$current")

set_version() {
    python3 - "$MANIFEST" "$1" <<'PY'
import re, sys
path, version = sys.argv[1], sys.argv[2]
text = open(path).read()
text, n = re.subn(r'^version = "[0-9]+\.[0-9]+\.[0-9]+"', f'version = "{version}"', text, count=1, flags=re.M)
assert n == 1, "workspace version line not found"
open(path, "w").write(text)
PY
}

echo "version: $current -> $next"
set_version "$next"
undo() { set_version "$current"; echo "build or install failed: version left at $current"; }
trap undo ERR

cd "$ROOT"
cargo build --release --workspace >/dev/null
cargo zigbuild --release -p kb-kindle --target armv7-unknown-linux-musleabihf >/dev/null
sh deploy/install.sh target/armv7-unknown-linux-musleabihf/release/kb-kindle
trap - ERR
echo "deployed Primitive Browser v$next"
