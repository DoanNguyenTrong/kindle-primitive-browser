# Device tools (Kindle Oasis 3, armv7)

Copied from the Kindle, so the browser does not depend on KOReader or KPM being installed. `deploy/install-ssh.sh`
sends them to `/mnt/us/browser/bin` (and `libz.so.1` to `/mnt/us/browser/lib`) when their SHA-256 differs. The
release package holds the same files.

| File | Version | Source | Used for | License |
|---|---|---|---|---|
| `armv7/fbink` | FBInk, git `92e1270` (reports itself as 1.25.0 for Kindle) | https://github.com/NiLuJe/FBInk/tree/92e1270 | e-ink updates when in-process drawing is not used (fallback, turned screen) | GPL-3.0, text in `licenses/FBINK-GPL-3.0.txt` |
| `armv7/dropbear` | Dropbear 2026.92 | https://github.com/mkj/dropbear/tree/DROPBEAR_2026.92 | SSH server (Diagnostics, Start SSH) | MIT-style and others, text in `licenses/DROPBEAR-LICENSE.txt` |
| `armv7/libz.so.1` | zlib 1.3.2 | https://zlib.net (and https://github.com/madler/zlib) | Dropbear's dependency | zlib license, text in `licenses/ZLIB-LICENSE.txt` |

## Source offer

The FBInk binary is GPL-3.0. Its corresponding source is the FBInk repository at commit `92e1270`, linked above.
Anyone who receives this binary can get that source from the link. If the binary is changed, the changed source must
be offered under GPL-3.0 as well.

The Dropbear and zlib binaries are built from the upstream sources at the versions above. Dropbear's banner says
"(KOReader)", so the build was made by KOReader; the build recipe and any patches are KOReader's. Building these
from upstream for this device is still to do.

Checksums (SHA-256):

    1063036 bytes  7045ed4243100eb1832463d501d6bc7addf44c5c52093a9f4a4f7512ac09d6dd  armv7/fbink
     191576 bytes  0e4340bcc9f098f87f0a27bee3f9cb0ee34a40a53e76e7c6367a2644f7898297  armv7/dropbear
      63172 bytes  79a78432f05a2dff2db4e518a13827c7979aeed10b1fa7cdc9aa0e350e78416b  armv7/libz.so.1
