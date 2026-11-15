#!/bin/sh
# Name: KB Browser
# Author: kindle-browser
# DontUseFBInk
#
# Library launcher. Stops the Kindle UI framework while the browser runs, runs
# the browser under a crash watchdog, and restores the framework on every way out.
#
# Restore is guaranteed by a watchdog process: if this launcher is killed (for
# example when the USB cable is unplugged or the process is reaped), the watchdog
# still stops the browser and starts the framework again. A marker file records a
# framework that was left stopped, so the next launch restores it first.
#
# Files live in $KB_DIR (default /mnt/us/browser): kb-kindle, start.txt, log.

DIR=${KB_DIR:-/mnt/us/browser}
LOG=$DIR/log-browser.txt
MARK=$DIR/framework-stopped
PIDFILE=$DIR/kb.pid
MAX_RESTARTS=3
cd "$DIR" || exit 1

log() { echo "[launcher $(date '+%H:%M:%S')] $*" >> "$LOG"; }

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
        usleep 1250000 2>/dev/null || sleep 1
        trap - TERM
    fi
}

# Start the framework again, once, and clear the marker.
restore_framework() {
    if [ -f "$MARK" ]; then
        log "restoring framework ($INIT)"
        framework_start
        rm -f "$MARK"
    fi
}

# A framework left stopped by an earlier run (killed launcher) is restored first.
restore_framework

# Watchdog: outlives this launcher. Once the launcher is gone it stops the browser
# (if still running) and restores the framework. Survives the launcher being killed.
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
        echo "[watchdog $(date '+%H:%M:%S')] launcher gone, restoring framework" >> "$LOG"
        framework_start
        rm -f "$MARK"
    fi
}

stop_framework_and_mark() {
    log "stopping framework ($INIT)"
    touch "$MARK"
    framework_stop
}

stop_framework_and_mark
watchdog $$ &

finish() {
    rm -f "$PIDFILE"
    restore_framework
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
