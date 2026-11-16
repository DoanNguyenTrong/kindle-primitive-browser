# Install Primitive Browser on a Kindle

This guide is for people who download the release package and do not build it. It works on
macOS, Linux and Windows. For building from source, see the [README](../README.md).

**Supported today: Kindle Oasis 3 (firmware 5.18.2), jailbroken.** Other Kindles are not supported
yet. See [device support](DEVICES.md). Installing on an unsupported model is at your own risk.

## What you need

1. **A jailbroken Kindle** with a USB data cable.
2. **FBInk** installed at `/mnt/us/libkh/bin/fbink`. The FBInk tools come with KPM / KOReader on a
   jailbroken Kindle. The browser draws the screen with this program.
3. **The release package**, `primitive-browser-<version>.zip`, from the
   [Releases](https://github.com/DoanNguyenTrong/kindle-primitive-browser/releases) page, and its `.sha256` file.

## 1. Check the download

Compare the package's checksum with the value in the `.sha256` file.

- **macOS:** `shasum -a 256 primitive-browser-<version>.zip`
- **Linux:** `sha256sum primitive-browser-<version>.zip`
- **Windows (PowerShell):** `Get-FileHash primitive-browser-<version>.zip -Algorithm SHA256`

If the values differ, download the package again.

## 2. Unzip the package

Unzip `primitive-browser-<version>.zip`. You get one folder, `primitive-browser-<version>/`, with:

```text
browser/        the browser program, its icon and the start page
documents/      the launcher that shows Primitive Browser in the Kindle library
INSTALL.md      this guide
```

## 3. Copy the files onto the Kindle

Connect the Kindle by USB and wait until its storage appears. Copy the two folders onto the
**root** of the Kindle's storage, merging with what is already there:

- `browser/` goes to `<Kindle>/browser/`
- `documents/` goes to `<Kindle>/documents/`

Where the Kindle appears:

- **macOS:** `/Volumes/Kindle`
- **Linux:** `/media/<you>/Kindle` or `/run/media/<you>/Kindle`
- **Windows:** the Kindle's drive letter, for example `E:\`

The copy must keep the `documents/Primitive Browser.sh` file name. If Primitive Browser is already
installed, follow [Update](#update) instead, so that your start page is kept.

The release package has no installer script. The copy above is the install. (Developers who work
from the source tree can use `KINDLE=<path to the Kindle> sh deploy/install.sh` instead.)

## 4. Eject and open

Eject the Kindle from your computer (safely remove it) before you unplug it. Then on the Kindle, open
the library. **Primitive Browser** appears there with its icon. Open it.

The first launch starts at the page in `browser/start.txt` (Hacker News by default). Change that file
on the Kindle to choose a different start page.

## Update

Copy only these three files from the new package to the Kindle, replacing the old ones:
`browser/kb-kindle`, `browser/icon.png` and `documents/Primitive Browser.sh`. **Do not copy
`browser/start.txt`**, which is your start page. Your data is also kept: `browser/data/` (cookies,
bookmarks, cache, saved pages) and the log.

## Uninstall

Delete `documents/Primitive Browser.sh` and the `browser/` folder from the Kindle. Your other books
and files are not affected.

## If something goes wrong

- **The library shows a blank screen after you exit.** Create an empty file named `stop-framework` in
  the `browser/` folder on the Kindle. The browser will then stop and restart the Kindle's framework
  on exit. This is slower, but it is the fallback.
- **The browser does not open.** Check that FBInk is at `/mnt/us/libkh/bin/fbink`.
- **Log.** `browser/log-browser.txt` records what the launcher and the browser did. Include its last
  lines when you report a problem.
- **macOS adds `._` files** when you copy to a Kindle. They are harmless. You can delete them.

To report a problem, open an issue on the repository and include the Kindle model, the firmware
version (Settings → Device Info), the version of Primitive Browser (shown in the Menu title) and the
log's last lines.
