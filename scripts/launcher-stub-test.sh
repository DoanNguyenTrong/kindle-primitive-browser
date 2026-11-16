#!/bin/sh
# Tests the device launcher (deploy/Browser.sh) without a Kindle.
#
# The Kindle's commands (upstart `start` and `stop`, `lipc-set-prop`, `killall`) and the browser binary
# are replaced by stubs in a temporary folder. The stubs record their calls. The checks are:
#   1. a normal run hides the status bar, runs the browser, then restores the bar and opens the
#      library, leaving the framework and window manager running;
#   2. a launcher killed mid-run is recovered by the watchdog: the UI is given back and the library opens;
#   3. a marker from an earlier run is given back at the next launch, before the UI is changed again;
#   4. with the stop-framework flag file, the framework is stopped and started again (the fallback);
#   5. a marker from the older launcher (no mode, the framework was stopped) restores the framework.
#
# Usage: scripts/launcher-stub-test.sh            (needs only /bin/sh, sleep and kill)
set -u

ROOT=$(cd "$(dirname "$0")/.." && pwd)
LAUNCHER=$ROOT/deploy/Browser.sh
T=$(mktemp -d)
mkdir -p "$T/bin" "$T/dir"
CALLS=$T/calls.txt
fail=0

for cmd in start stop lipc-set-prop killall; do
    cat >"$T/bin/$cmd" <<STUB
#!/bin/sh
echo "$cmd \$*" >> "$CALLS"
exit 0
STUB
    chmod +x "$T/bin/$cmd"
done
cat >"$T/dir/kb-kindle" <<'STUB'
#!/bin/sh
sleep 3
exit 0
STUB
chmod +x "$T/dir/kb-kindle"
cp "$LAUNCHER" "$T/dir/launcher.sh"

# The calls the launcher made, one line, as the test compares them.
flat() { tr '\n' ' ' < "$CALLS" 2>/dev/null | sed 's/ $//'; }

check() {
    if [ "$1" = "$2" ]; then
        echo "PASS $3"
    else
        echo "FAIL $3"
        echo "     expected: '$2'"
        echo "     got:      '$1'"
        fail=1
    fi
}

HIDE="lipc-set-prop com.lab126.pillow disableEnablePillow disable"
SHOW="lipc-set-prop com.lab126.pillow disableEnablePillow enable"
LIBRARY="lipc-set-prop com.lab126.appmgrd start app://com.lab126.booklet.home"

echo "== 1. normal run (keep the framework)"
(cd "$T/dir" && PATH="$T/bin:$PATH" KB_DIR="$T/dir" sh launcher.sh)
sleep 1
check "$(flat)" "$HIDE $SHOW $LIBRARY" "status bar hidden, then given back and the library opened"
check "$(test -f "$T/dir/framework-stopped" && echo yes || echo no)" "no" "marker cleared after exit"

echo "== 2. launcher killed mid-run"
rm -f "$CALLS"
(cd "$T/dir" && PATH="$T/bin:$PATH" KB_DIR="$T/dir" sh launcher.sh) &
LP=$!
sleep 1.5
kill -9 "$LP" 2>/dev/null
sleep 6
check "$(flat)" "$HIDE $SHOW $LIBRARY" "watchdog gives the UI back and opens the library after the launcher is killed"
check "$(test -f "$T/dir/framework-stopped" && echo yes || echo no)" "no" "marker cleared by the watchdog"

echo "== 3. marker left by an earlier run"
rm -f "$CALLS"
echo keep > "$T/dir/framework-stopped"
# Run in the background and wait at most 30 s, so a launcher that hangs fails the test
# instead of blocking it.
(cd "$T/dir" && PATH="$T/bin:$PATH" KB_DIR="$T/dir" sh launcher.sh) &
LP=$!
waited=0
while kill -0 "$LP" 2>/dev/null && [ "$waited" -lt 30 ]; do
    sleep 1
    waited=$((waited + 1))
done
if kill -0 "$LP" 2>/dev/null; then
    kill -9 "$LP" 2>/dev/null
    wait "$LP" 2>/dev/null
    check "hung" "exited" "launcher exits within 30 s"
else
    wait "$LP" 2>/dev/null
    check "exited" "exited" "launcher exits within 30 s"
fi
check "$(flat)" "$SHOW $HIDE $SHOW $LIBRARY" "stale marker is given back before the UI is changed again"
check "$(test -f "$T/dir/framework-stopped" && echo yes || echo no)" "no" "stale marker cleared after recovered run"

echo "== 4. stop-framework flag (the fallback)"
rm -f "$CALLS"
touch "$T/dir/stop-framework"
(cd "$T/dir" && PATH="$T/bin:$PATH" KB_DIR="$T/dir" sh launcher.sh)
sleep 1
rm -f "$T/dir/stop-framework"
check "$(flat)" "stop lab126_gui start lab126_gui $LIBRARY" "framework stopped, started again, library opened"
check "$(test -f "$T/dir/framework-stopped" && echo yes || echo no)" "no" "marker cleared after the stop-framework run"

echo "== 5. marker from the older launcher (no mode)"
rm -f "$CALLS"
: > "$T/dir/framework-stopped"
(cd "$T/dir" && PATH="$T/bin:$PATH" KB_DIR="$T/dir" sh launcher.sh)
sleep 1
check "$(head -n 1 "$CALLS")" "start lab126_gui" "old marker restores the framework first"

rm -rf "$T"
if [ $fail -eq 0 ]; then
    echo "launcher tests passed"
    exit 0
fi
echo "launcher tests FAILED"
exit 1
