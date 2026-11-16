#!/bin/sh
# Point the Kindle's DNS at public resolvers, or put back the network's original DNS settings.
#
#   sh /mnt/us/set-dns.sh            use 1.1.1.1 and 8.8.8.8
#   sh /mnt/us/set-dns.sh restore    put back the original /etc/resolv.conf
#
# The original /etc/resolv.conf is saved to /mnt/us the first time this runs. Reconnecting to Wi-Fi may
# overwrite /etc/resolv.conf again; run the script again after that.
RESOLV=/etc/resolv.conf
ORIGINAL=/mnt/us/resolv.conf.original
NEW=/mnt/us/resolv.conf.new

if [ "$1" = "restore" ]; then
    if [ ! -f "$ORIGINAL" ]; then
        echo "no saved original at $ORIGINAL; nothing changed"
        exit 1
    fi
    SRC=$ORIGINAL
else
    if [ ! -f "$ORIGINAL" ]; then
        if ! cp "$RESOLV" "$ORIGINAL"; then
            echo "could not save the original DNS settings; nothing changed"
            exit 1
        fi
        echo "original saved to $ORIGINAL"
    fi
    printf 'nameserver 1.1.1.1\nnameserver 8.8.8.8\n' > "$NEW"
    SRC=$NEW
fi

echo "== before"
cat "$RESOLV"

if ! mount -o remount,rw /; then
    echo "could not make / writable; nothing changed"
    rm -f "$NEW"
    exit 1
fi
trap 'mount -o remount,ro / 2>/dev/null' EXIT

if cp "$SRC" "$RESOLV"; then
    sync
    echo "DNS updated"
else
    echo "could not write $RESOLV"
fi
rm -f "$NEW"

echo "== after"
cat "$RESOLV"
