#!/bin/sh
# Name: Primitive Browser
# Author: Primitive AI
# DontUseFBInk
#
# Library launcher. Runs the browser over the Kindle's own UI: the status bar is hidden while
# the framework and window manager keep running. Exit restores the status bar and opens the library.
# If the file "stop-framework" exists in the browser folder, the whole framework is stopped instead
# (the older, slower way back to the library).
#
# A watchdog process outlives this launcher. If the launcher is killed (for example when the USB cable
# is unplugged), the watchdog stops the browser and gives the UI back. A marker file records what was
# changed, so the next launch gives the UI back first.
#
# Files live in $KB_DIR (default /mnt/us/browser): kb-kindle, start.txt, log, marker.

DIR=${KB_DIR:-/mnt/us/browser}
LOG=$DIR/log-browser.txt
MARK=$DIR/framework-stopped
PIDFILE=$DIR/kb.pid
MAX_RESTARTS=3
cd "$DIR" || exit 1

log() { echo "[launcher $(date '+%H:%M:%S')] $*" >> "$LOG"; }

# Set by Primitive Launcher, which has already stopped the framework and owns the screen. Then this script
# does not touch the stock UI at all: no stop, no restore, no library call (see the functions below).
LAUNCHED=${PRIMITIVE_LAUNCHER:-0}

# Keep the log small on the Kindle's flash: keep the last ~200 KB.
if [ -f "$LOG" ] && [ "$(wc -c < "$LOG")" -gt 204800 ]; then
    tail -c 102400 "$LOG" > "$LOG.tmp" && mv "$LOG.tmp" "$LOG"
fi
log "start"

if [ -e /etc/init.d/framework ]; then
    INIT=sysv
else
    INIT=upstart
fi

# The mode: "keep" (the default) or "stop" (when the flag file exists).
if [ -f "$DIR/stop-framework" ]; then
    MODE=stop
else
    MODE=keep
fi

# Sleep for a short time (microseconds where the device has usleep).
pause_us() {
    usleep "$1" 2>/dev/null || sleep 1
}

framework_start() {
    if [ "$INIT" = sysv ]; then
        (cd / && /etc/init.d/framework start)
    else
        (cd / && start lab126_gui)
    fi
}

framework_stop() {
    if [ "$INIT" = sysv ]; then
        /etc/init.d/framework stop
    else
        trap "" TERM
        stop lab126_gui
        pause_us 1250000
        trap - TERM
    fi
}

# Take the UI for the browser. The marker is written first, so a launcher killed here is still restored.
stop_ui() {
    [ "$LAUNCHED" = 1 ] && { log "under Primitive Launcher: the UI is already stopped"; return 0; }
    if [ "$MODE" = stop ]; then
        log "stopping framework ($INIT)"
        echo stop > "$MARK"
        framework_stop
    else
        # Only the status bar is hidden. The window manager keeps running: pausing it stopped the
        # framework from turning the screen, so the text did not follow the Kindle's turn.
        log "hiding the status bar"
        echo keep > "$MARK"
        lipc-set-prop com.lab126.pillow disableEnablePillow disable 2>/dev/null
    fi
}

# Give the UI back: undo what the launcher changed. A marker without a mode (from an earlier version)
# means the framework was stopped.
restore_ui() {
    [ "$LAUNCHED" = 1 ] && { rm -f "$MARK"; return 0; }
    [ -f "$MARK" ] || return 0
    case "$(head -n 1 "$MARK" 2>/dev/null)" in
        keep)
            log "restoring the status bar"
            lipc-set-prop com.lab126.pillow disableEnablePillow enable 2>/dev/null
            ;;
        *)
            log "restoring framework ($INIT)"
            t0=$(date +%s)
            framework_start
            log "framework start returned after $(( $(date +%s) - t0 ))s"
            ;;
    esac
    rm -f "$MARK"
}

# Open the library. With the UI kept running this succeeds at once. When the framework was stopped it
# may need a moment to come up, so retry every quarter second (up to 10 s) rather than wait first.
land_on_home() {
    [ "$LAUNCHED" = 1 ] && return 0
    [ "$INIT" = upstart ] || return 0
    if ! command -v lipc-set-prop >/dev/null 2>&1; then
        log "lipc-set-prop not found; the UI opens its default screen"
        return 0
    fi
    i=0
    while [ $i -lt 40 ]; do
        if lipc-set-prop com.lab126.appmgrd start app://com.lab126.booklet.home 2>/dev/null; then
            log "opened library home after $i tries"
            return 0
        fi
        pause_us 250000
        i=$((i + 1))
    done
    log "could not open library home"
}

# A UI left changed by an earlier run is given back first.
restore_ui

stop_ui

# Watchdog: outlives this launcher. Once the launcher is gone it stops the browser (if still running),
# gives the UI back and opens the library.
watchdog() {
    trap '' HUP INT
    lpid=$1
    while kill -0 "$lpid" 2>/dev/null; do sleep 1; done
    if [ -f "$PIDFILE" ]; then
        kb=$(cat "$PIDFILE" 2>/dev/null)
        [ -n "$kb" ] && kill "$kb" 2>/dev/null
        sleep 1
    fi
    if [ -f "$MARK" ]; then
        echo "[watchdog $(date '+%H:%M:%S')] launcher gone, giving the UI back" >> "$LOG"
        restore_ui
        land_on_home
    fi
}
watchdog $$ &

finish() {
    rm -f "$PIDFILE"
    if [ -f "$MARK" ]; then
        restore_ui
        land_on_home
    fi
}
trap 'finish; exit 0' EXIT INT TERM HUP

restarts=0
while :; do
    ./kb-kindle >> "$LOG" 2>&1 &
    echo $! > "$PIDFILE"
    wait $!
    rc=$?
    rm -f "$PIDFILE"
    if [ "$rc" -eq 0 ]; then
        log "browser exited normally"
        break
    fi
    restarts=$((restarts + 1))
    log "browser crashed (rc=$rc), restart $restarts of $MAX_RESTARTS"
    if [ "$restarts" -ge "$MAX_RESTARTS" ]; then
        log "too many crashes; giving up"
        break
    fi
    sleep 1
done
