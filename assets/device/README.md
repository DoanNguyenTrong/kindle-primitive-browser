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

## Build the ARMEL candidate

From the browser checkout, run these commands with Zig 0.17.1 and cargo-zigbuild available.

```sh
cargo zigbuild --locked --release -p kb-kindle --target armv7-unknown-linux-musleabi
sh scripts/stage.sh armel
```

Staging runs [the helper build recipe](../../scripts/build-armel-fbink.sh) on its first use. The recipe verifies
each source archive's SHA-256 before unpacking it and saves a static ARMv7 helper with Kindle, bitmap and image support
under `build/device/armel/`. The app's fallback passes BMP frames to this helper. The package includes the notices
for the linked source. The corresponding code uses the upstream commit and its exact bitmap and image submodules.
[FBInk source](https://github.com/NiLuJe/FBInk/tree/92e127008145b2a22fba7c59815d810d716310dd) ·
[stb source](https://github.com/NiLuJe/stb/tree/b034a871ead7eeee035d9b7dd2fffcf0370b239d) ·
[font8x8 source](https://github.com/NiLuJe/font8x8/tree/fa74831b9076805271cdedc0ee12137aa548e524)

Verify the software fallback and screen behavior on a supported soft-float Kindle before listing the package.
