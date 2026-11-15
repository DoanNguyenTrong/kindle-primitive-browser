# 2. System architecture

Version 0.1.0. This describes the code as committed. Requirements it serves are in
[01-requirements.md](01-requirements.md).

## 2.1 Overview

The browser is one native program per target. The same engine code runs on the computer
(for tests, through `kb-desktop`) and on the device (through `kb-kindle`). Only the input
and display drivers differ.

```
 +------------------------- kb-kindle (device driver) -------------------------+
 |  evdev touch + page buttons    FBInk display (libkh)    deploy/Browser.sh  |
 +--------------------------------------+-------------------------------------+
                                        |
 +--------------------- kb-desktop (computer driver, stdin) -------------------+
                                        |
                                        v
 +------------------------------ kb-app (browser) -----------------------------+
 |  tabs, history, bookmarks, saved pages, settings, toasts, keyboard, sheets   |
 |  ui.rs: one layout system for bars, sheets, lists, home, errors, keyboard    |
 +-------------------+------------------------------------+---------------------+
                     |                                    |
                     v                                    v
 +-------------------------------+        +-----------------------------------+
 | kb-engine (page)              |        | kb-net (network and storage)      |
 |  Blitz: parse, style, layout  |<------>|  ureq + rustls: HTTP(S)           |
 |  paint -> 8-bit gray          |        |  cookie jar, disk cache, blocklist|
 |  hit testing, scroll, forms   |        |  SyncNet: sub-resource loader     |
 |  reader view, streamed chunks |        +-----------------------------------+
 +---------------+---------------+
                 |
                 v
 +-------------------------------+
 | kb-js (page scripts)          |
 |  QuickJS (rquickjs 0.14)      |
 |  native DOM ops + prelude.js  |
 +-------------------------------+
```

## 2.2 Crates

| Crate | Kind | Responsibility | Direct dependencies |
|---|---|---|---|
| `kb-net` | library | HTTP(S) fetch and POST, cookie jar, disk cache with size cap, tracker blocklist, sub-resource loader with image deferral | `blitz-traits`, `ureq` |
| `kb-js` | library | QuickJS runtime per page; native DOM operations; prelude with browser APIs; time and memory limits | `blitz-dom`, `blitz-html`, `blitz-traits`, `rquickjs` |
| `kb-engine` | library | One page: parse, run scripts, lay out, paint to gray, scroll, hit test, forms, reader view | `blitz-html`, `blitz-dom`, `blitz-paint`, `blitz-traits`, `anyrender_vello_cpu`, `parley`, `kb-net`, `kb-js`, `url` |
| `kb-app` | library | The browser: navigation, history, tabs, bookmarks, saved pages, settings, notices, the keyboard and sheets, and the layout system in `ui.rs` | `kb-engine`, `kb-net`, `url`, `png` |
| `kb-desktop` | binary | Computer driver: reads commands from stdin, writes PNG frames, reports timings; used by the test suite | `kb-app` |
| `kb-kindle` | binary | Device driver: evdev input, touch gestures, FBInk display, memory log | `kb-app`, `libc` |

Workspace root: `Cargo.toml` (members, shared dependency versions, the vendored patch).

## 2.3 External dependencies

Versions are the ones in `Cargo.lock`.

| Dependency | Version | Used for | Licence (upstream) |
|---|---|---|---|
| blitz-html, blitz-dom, blitz-paint, blitz-traits | 0.3.0-beta.4 (exact pin) | HTML parsing, style, layout, paint, DOM | MIT or Apache-2.0 |
| Stylo, Taffy, Parley, Vello CPU (transitive) | as pulled by Blitz | CSS engine, flex/grid layout, text, rasteriser | MPL-2.0 / MIT / Apache-2.0 (transitive) |
| anyrender_vello_cpu | 0.18.0 | Paint backend for Blitz | MIT or Apache-2.0 |
| parley | 0.12.0 (exact pin) | Text layout and font collection | MIT or Apache-2.0 |
| ureq | 3.5.2 (features: rustls, gzip, cookies, json) | HTTP client | MIT or Apache-2.0 |
| rustls (transitive) | as pulled by ureq | TLS with bundled Mozilla roots | Apache-2.0 / MIT / ISC |
| rquickjs | 0.14.0 | Rust binding for QuickJS | MIT |
| rquickjs-sys | 0.14.0, **vendored** (see 2.5) | QuickJS C sources and bindings | MIT |
| QuickJS (C, inside rquickjs-sys) | bundled | JavaScript engine | MIT |
| png | 0.17 | PNG encoding of frames | MIT or Apache-2.0 |
| url | 2.5.8 | URL parsing and joining | MIT or Apache-2.0 |
| libc | 0.2 | evdev ioctl and device calls (device build only) | MIT or Apache-2.0 |

Runtime files used on the device but not shipped in this repository:

| File | Provided by | Notes |
|---|---|---|
| `/mnt/us/koreader/fonts/noto/*.ttf` | KOReader | Noto Sans and Noto Serif (SIL Open Font License) |
| `/mnt/us/libkh/bin/fbink` | KOReader package manager | FBInk, used for the e-ink update calls |
| `upstart` `start`/`stop` or `/etc/init.d/framework` | Kindle firmware | Stops and restarts the Kindle UI framework |

Fonts for the computer build are copied into `assets/fonts/` (Noto, OFL; licence text in
`NOTO-LICENSE.txt`).

## 2.4 Threading and data flow

- **Main thread**: the driver loop, the browser state, layout, paint, and all DOM access.
  Blitz documents and QuickJS contexts are not `Send`, so they stay on this thread.
- **Input thread** (device only): reads `/dev/input` events and sends them over a channel.
- **Document loader threads**: one per main-document request. The main thread waits with a
  60 ms poll, so the UI keeps running and can show progress.
- **Sub-resource threads**: at most six requests in flight (`SyncNet`). Blitz receives each
  result through its own channel, drained on the main thread.
- **Progress and cancellation**: during a page's resource wait, the pump callback reports
  progress and returns whether to keep waiting. On the device, a tap during the resource
  phase ends the wait early, and the page is laid out with what has arrived. A tap during
  the document download is queued, not cancelled.

Frame path: state change → `Browser::frame()` → cached layers composed into one buffer →
`Update` (the buffer plus the changed rows and a waveform) → the driver crops the rows,
encodes PNG, and calls FBInk with the waveform (`DU` for chrome and keyboard, `GL16` for
page content, `GC16` every eighth text update to clear ghosting).

## 2.5 Vendored code and patches

- `vendor/rquickjs-sys-0.14.0/`: a copy of the crates.io release. The only change is that
  `armv7-unknown-linux-musleabihf` bindings are a copy of the `gnueabihf` bindings. This
  lets the device build (static, musl) link. Declared in the workspace `[patch.crates-io]`.
  See `vendor/README.md`.
- `spike/`: the first feasibility program (milestone M0). Excluded from the workspace. Kept
  for history.

## 2.6 Layout system (`kb-app/src/ui.rs`)

All screens drawn by the browser itself use one stylesheet built from constants:

- Bars: address bar 96 px (pill 68 px), bottom toolbar 104 px with five equal cells.
- Sheets (menu, tabs): 780 px, a 72 px title, rows of 96 px, 1 px dividers.
- Side margin 20 px on bars and sheets, 24 px on pages; 8 px spacing unit.
- Keyboard: keys at the bottom 730 px of the screen (`KB_Y` = 950).

Internal pages and chrome are HTML rendered by the same engine, at scale 1. Web pages are
laid out at 412 CSS px and scaled to the panel (`WEB_CSS_WIDTH`).

## 2.7 Storage layout

Root: `$KB_DATA` on the computer, `/mnt/us/browser/data` on the device.

| Path | Content | Written when |
|---|---|---|
| `cookies.json` | Cookie jar | after each document load, on exit |
| `cache/<hash>` | Cached responses: a header line (time, type, final URL), then the body | each cacheable load; capped at 200 MB, oldest evicted |
| `bookmarks.tsv` | URL and title per line | on bookmark change |
| `history.tsv` | Up to 300 visited pages, newest first | after each web load |
| `saved.tsv` and `saved/<key>.html` | Offline copies and their index | on Save |
| `settings.tsv` | Text size and night mode | on change |
| `defaults_v2` | Marker: default bookmarks already added | once |

Device-only files in `/mnt/us/browser/`: `kb-kindle` (binary), `start.txt` (start page),
`log-browser.txt` (kept near 200 KB), `frame.png` (last frame), `framework-stopped` (marker
while the Kindle UI is stopped), `kb.pid` (browser process while it runs).

## 2.8 Device lifecycle

Entry point: `documents/KB Browser.sh`, installed from `deploy/Browser.sh`.

1. A marker file from an earlier run means the Kindle UI may still be stopped: it is
   restored first.
2. The UI framework is stopped (`stop lab126_gui` under upstart; `/etc/init.d/framework stop`
   under SysV).
3. A **watchdog** process is started. It waits until the launcher exits, then stops the
   browser process if it is still running, restarts the UI framework, and clears the marker.
4. The browser runs under the launcher, which restarts it after a crash (up to three times).
5. On exit, the launcher restores the framework through its exit trap.

Killing the launcher leaves the watchdog to restore the framework. Whether the Kindle
library closes the whole process group, which would also stop the watchdog, is not yet
confirmed on the device (see the validation document).

## 2.9 Error handling

- Page code panics are caught around each event; the browser keeps running and logs
  `recovered from a panic`.
- Script errors are logged with `[js]` and do not stop the page.
- Network failures become an error page with a plain-language reason and retry.
- A failed POST shows an error page; history is not changed.
- A load cancelled by the user during the document download is undone (history entry
  removed).

## 2.10 Known architectural limits

- Layout uses Blitz 0.3.0-beta.4. Its CSS coverage is partial, and it is pinned exactly.
- Hit testing and paint use document coordinates plus the scroll offset; coordinates are
  converted at the `Page` boundary only.
- Sheets and keyboard layers are separate pages, so they do not share layout with the page
  underneath.
- Scripts run synchronously on the main thread. A script that fetches a script file blocks
  until that file arrives.
- The one-page cache keeps a whole live page (DOM and JavaScript heap, up to 48 MB) for
  instant Back. Memory use is therefore two pages at most.
