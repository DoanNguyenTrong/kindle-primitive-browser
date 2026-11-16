#!/bin/sh
# Tests the device launcher (deploy/Browser.sh) without a Kindle.
#
# The Kindle's `start` and `stop` (upstart) commands and the browser binary are replaced
# by stubs in a temporary folder. The checks are:
#   1. a normal run stops the UI, runs the browser, restarts the UI, and clears the marker;
#   2. a launcher killed mid-run is recovered by the watchdog: the browser is stopped and
#      the UI is restarted;
#   3. a marker left by an earlier run is restored at the next launch.
#
# Usage: scripts/launcher-stub-test.sh            (needs only /bin/sh, sleep and kill)
set -u

ROOT=$(cd "$(dirname "$0")/.." && pwd)
LAUNCHER=$ROOT/deploy/Browser.sh
T=$(mktemp -d)
mkdir -p "$T/bin" "$T/dir"
CALLS=$T/calls.txt
fail=0

cat >"$T/bin/start" <<STUB
#!/bin/sh
echo "start \$*" >> "$CALLS"
STUB
cat >"$T/bin/stop" <<STUB
#!/bin/sh
echo "stop \$*" >> "$CALLS"
STUB
cat >"$T/dir/kb-kindle" <<'STUB'
#!/bin/sh
sleep 3
exit 0
STUB
chmod +x "$T/bin/start" "$T/bin/stop" "$T/dir/kb-kindle"
cp "$LAUNCHER" "$T/dir/launcher.sh"

check() {
    if [ "$1" = "$2" ]; then
        echo "PASS $3"
    else
        echo "FAIL $3 (expected '$2', got '$1')"
        fail=1
    fi
}

echo "== 1. normal run"
(cd "$T/dir" && PATH="$T/bin:$PATH" KB_DIR="$T/dir" sh launcher.sh)
sleep 1
check "$(tr '\n' ' ' < "$CALLS" | sed 's/ $//')" "stop lab126_gui start lab126_gui" "UI stopped, then restarted"
check "$(test -f "$T/dir/framework-stopped" && echo yes || echo no)" "no" "marker cleared after exit"

echo "== 2. launcher killed mid-run"
rm -f "$CALLS"
(cd "$T/dir" && PATH="$T/bin:$PATH" KB_DIR="$T/dir" sh launcher.sh) &
LP=$!
sleep 1.5
kill -9 "$LP" 2>/dev/null
sleep 6
check "$(tr '\n' ' ' < "$CALLS" | sed 's/ $//')" "stop lab126_gui start lab126_gui" "watchdog restarts the UI after the launcher is killed"
check "$(test -f "$T/dir/framework-stopped" && echo yes || echo no)" "no" "marker cleared by the watchdog"

echo "== 3. marker left by an earlier run"
rm -f "$CALLS"
touch "$T/dir/framework-stopped"
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
first=$(head -n 1 "$CALLS")
check "$first" "start lab126_gui" "stale marker restores the UI before stopping it again"
check "$(test -f "$T/dir/framework-stopped" && echo yes || echo no)" "no" "stale marker cleared after recovered run"

rm -rf "$T"
if [ $fail -eq 0 ]; then
    echo "launcher tests passed"
    exit 0
fi
echo "launcher tests FAILED"
exit 1
