#!/bin/sh
# Remove the device's hosts-file override for truyendich.space.
#
# A hosts entry that sends truyendich.space to 127.0.0.1 or ::1 (this device) stops the site from loading
# in Primitive Browser. This script deletes only those lines and leaves every other entry in /etc/hosts.
#
# Run it from KOReader's terminal, with the script copied to the Kindle's storage (/mnt/us):
#   sh /mnt/us/fix-hosts-truyendich.sh
#
# A backup of /etc/hosts is written to /mnt/us first. The system partition is made writable only for the
# copy and is made read-only again afterwards, including when the script stops early.
HOSTS=/etc/hosts
NEW=/mnt/us/hosts.new
SITE='truyendich\.space'
STAMP=$(date +%Y%m%d-%H%M%S 2>/dev/null || echo backup)
BACKUP=/mnt/us/hosts.backup-$STAMP

echo "== entries for truyendich.space in $HOSTS"
if ! grep -n "$SITE" "$HOSTS"; then
    echo "no truyendich.space entry; nothing to do"
    exit 0
fi

if ! cp "$HOSTS" "$BACKUP"; then
    echo "could not write the backup to $BACKUP; nothing changed"
    exit 1
fi
echo "backup written: $BACKUP"

# Keep every line except the loopback overrides for this site.
grep -Ev "^(127\.0\.0\.1|::1)[[:space:]].*$SITE" "$HOSTS" > "$NEW"
if [ ! -s "$NEW" ]; then
    echo "the new hosts file would be empty; nothing changed"
    rm -f "$NEW"
    exit 1
fi

if ! mount -o remount,rw /; then
    echo "could not make / writable; nothing changed"
    rm -f "$NEW"
    exit 1
fi
trap 'mount -o remount,ro / 2>/dev/null' EXIT

if cp "$NEW" "$HOSTS"; then
    sync
    echo "override removed"
else
    echo "could not write $HOSTS; restore it from $BACKUP if needed"
fi
rm -f "$NEW"

echo "== entries for truyendich.space now"
grep -n "$SITE" "$HOSTS" || echo "none"
