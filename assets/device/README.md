# Device tools (Kindle Oasis 3, armv7)

Copied from the Kindle, so the browser does not depend on KOReader or KPM being installed. `deploy/install-ssh.sh`
sends `fbink` to `/mnt/us/browser/bin` when its SHA-256 differs. The release package holds the same file.

The SSH server (Dropbear and its zlib) is the launcher's: see the launcher's `assets/device/README.md`.

| File | Version | Source | Used for | License |
|---|---|---|---|---|
| `armv7/fbink` | FBInk, git `92e1270` (reports itself as 1.25.0 for Kindle) | https://github.com/NiLuJe/FBInk/tree/92e1270 | e-ink updates when in-process drawing is not used (fallback, turned screen) | GPL-3.0, text in `licenses/FBINK-GPL-3.0.txt` |

## Source offer

The FBInk binary is GPL-3.0. Its corresponding source is the FBInk repository at commit `92e1270`, linked above.
Anyone who receives this binary can get that source from the link. If the binary is changed, the changed source must
be offered under GPL-3.0 as well.

Checksums (SHA-256):


    1063036 bytes  7045ed4243100eb1832463d501d6bc7addf44c5c52093a9f4a4f7512ac09d6dd  armv7/fbink
