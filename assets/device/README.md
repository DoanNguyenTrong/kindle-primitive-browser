# Device tools (Kindle Oasis 3, armv7)

Copied from the Kindle, so the browser does not depend on KOReader or KPM being installed. `deploy/install-ssh.sh`
sends them to `/mnt/us/browser/bin` (and `libz.so.1` to `/mnt/us/browser/lib`) when their SHA-256 differs.

| File | Version | Upstream | Used for | License |
|---|---|---|---|---|
| `armv7/fbink` | FBInk 1.25.0 for Kindle | https://github.com/NiLuJe/FBInk | e-ink updates when in-process drawing is not used (fallback, turned screen, dithering) | GPL-3.0 |
| `armv7/dropbear` | Dropbear 2026.92 | https://matt.ucc.asn.au/dropbear/ | SSH server (Diagnostics, Start SSH) | MIT-style (see the Dropbear source) |
| `armv7/libz.so.1` | zlib 1.3.2 | https://zlib.net | Dropbear's dependency | zlib license |

Checksums (SHA-256):

    58938160f5cf1c9dac5fef45741eb9584bf698e6f939e8b4e9d4b637849df4dd  armv7/fbink
    0e4340bcc9f098f87f0a27bee3f9cb0ee34a40a53e76e7c6367a2644f7898297  armv7/dropbear
    79a78432f05a2dff2db4e518a13827c7979aeed10b1fa7cdc9aa0e350e78416b  armv7/libz.so.1

These are the builds KOReader ships for this model (Dropbear's banner says "(KOReader)"). Building them from the
upstream sources for this device, and adding the license texts here, are still to do.
