# 2. System architecture

Latest release: 0.2.1 (2026-11-17). This describes the working tree. Requirements it serves are in
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
| `kb-kindle` | binary | Device driver: evdev input, touch gestures, FBInk display, memory log, and the Kindle's own controls (frontlight, warmth, battery, Wi-Fi, clock) through `lipc` for the quick settings panel (`device.rs`) | `kb-app`, `libc` |

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
| `/mnt/us/browser/fonts/*.ttf` | Primitive Browser (copied from `assets/fonts/` by `deploy/install-ssh.sh`) | Noto Sans and Noto Serif (SIL Open Font License) |
| `/mnt/us/browser/bin/fbink` | Primitive Browser (copied from `assets/device/armv7/` by `deploy/install-ssh.sh`) | FBInk, used for the e-ink update calls |
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
encodes PNG, and calls FBInk with the waveform (`DU` for short keyboard/progress feedback, `GL16` for
pages, bars and sheets, `GC16` every eighth text update to clear ghosting).

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
- Sheets (menu, tabs, display): 780 px; a separate pinned 72 px header and a 708 px
  scrolling body. Painting, hit testing, and selector lookup use the same offsets. Done
  stays in the header; Menu also has Exit. Rows are at least 96 px.
- Side margin 20 px on bars and sheets, 24 px on pages; 8 px spacing unit.
- Keyboard: the key area scales with screen height through `kb_top(height)`; at the
  1680 px portrait height the keys start at 950 px and occupy the bottom 730 px.

Internal pages and chrome are HTML rendered by the same engine, at scale 1. Web pages are
laid out at the selected width (412/600/800 CSS px, default 600) and scaled to the panel
(`PAGE_WIDTHS`, `settings.page_width`). QuickJS receives that width, the scaled height, and
the same device pixel ratio; basic JS width queries agree with the renderer.

## 2.7 Storage layout

Root: `$KB_DATA` on the computer, `/mnt/us/browser/data` on the device.

| Path | Content | Written when |
|---|---|---|
| `cookies.json` | Cookie jar | after each document load, on exit |
| `cache/<hash>` | Cached responses: a header line (time, type, final URL), then the body | each cacheable load; capped at 200 MB, oldest evicted |
| `bookmarks.tsv` | URL and title per line | on bookmark change |
| `history.tsv` | Up to 300 visited pages, newest first | after each web load |
| `saved.tsv` and `saved/<key>.html` | Offline copies and their index | on Save |
| `session.tsv` | Active tab, URLs, per-tab history and scroll positions | after page loads, tab closes, and Exit |
| `settings.tsv` | Text size, page width, night mode (used standalone only), Images, folded Home sections | on change |
| `defaults_v2` | Marker: default bookmarks already added | once |

Device-only files in `/mnt/us/browser/`: `kb-kindle` (binary), `start.txt` (start page),
`log-browser.txt` (kept near 200 KB), `frame.png` (last frame), `framework-stopped` (UI mode marker, containing `keep` or `stop`), `kb.pid` (browser process while it runs).

## 2.8 Device lifecycle

Entry point: `documents/Primitive Browser.sh`, installed from `deploy/Browser.sh` (it replaces the launcher's
old name, `KB Browser.sh`). The logo is `assets/primitive-icon.png`, built into the binary for the
home screen and copied to `browser/icon.png` for the launcher.

1. Recover an earlier UI change from `framework-stopped`: `keep` restores the status bar;
   `stop`, an empty marker, or a legacy marker restores the framework.
2. Default mode keeps both the framework and window manager running and hides the status bar
   through `com.lab126.pillow`. This preserves framework orientation updates. A `stop-framework`
   flag in the browser folder selects the older stop/restart mode.
3. A watchdog outlives the launcher. If it disappears, the watchdog stops a remaining browser
   process, restores the marked UI mode, and opens the library.
4. The launcher retains its crash restart limit of three and restores UI through its exit trap.
5. On upstart systems, library activation is attempted immediately and retried up to 40 times
   with a 250 ms device pause between attempts. The shell falls back to a one-second sleep when
   `usleep` is unavailable.

Orientation polling prefers `com.lab126.winmgr accelerometer` (`U`/`D`) and retains FBInk as
an optional fallback; both values are logged. The recorded device build does not export
`currentRota` from `fbink -e`, so the framework is the usable source there. The browser clears
without requesting an extra exit flash before the framework draws the library.

### Under Primitive Launcher (the system layer)

Primitive Launcher starts the entry with `PRIMITIVE_LAUNCHER=1` and stays running while the
browser is open. It owns the power button, sleep and wake, the lock screen, quick settings, the
power menu and the theme
([system architecture](https://github.com/PrimitiveOS/kindle-primitive/blob/main/docs/system-architecture.md)).
`Browser.sh` then leaves the Kindle's own UI alone: no framework stop, no status bar change, no
library call on exit.

- **Entry.** `deploy/Browser.sh` declares `# Wake: redraw` in its first ten lines and, under the
  launcher, ignores `SIGUSR1` in its own shell (`trap '' USR1`). The browser sets its own handler
  (`kb-kindle`, `on_redraw`).
- **Redraw.** `SIGUSR1` is the launcher's request for the whole screen. The loop takes it before
  any event that waited while the browser was frozen under quick settings: it reads the theme
  again (`set_system_theme`) and draws the whole screen with the page waveform, without a flash
  (`redraw_all_calm`). The launcher has just flashed the screen for a theme switch, and a second
  flash would show the old theme for a moment.
- **Theme.** The whole browser, bars and pages, follows `dark_mode` in
  `/mnt/us/launcher/settings.txt` (`KL_DIR` if set). The Display sheet has no Day and Night, and
  the `night` value in `settings.tsv` is kept for standalone use.
- **Quick settings.** The swipe down from the top edge is the launcher's. The browser ignores it
  and never opens its own panel.
- **Sleep.** The browser has no sleep or screensaver code: the launcher draws the sleep picture
  and puts the browser's screen back on wake. Without the launcher, sleep is left to the Kindle,
  as before.

[Device notes](device-notes.md) record the observed startup/rotation behavior, native touch
coordinates, status-bar handling, and library return timings. Whole-process-group termination
and comprehensive release validation still need separate checks.

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
  instant Back. This is two live content pages in steady state, plus cached chrome pages.
  Building a replacement can transiently allocate another document; resources, fonts and
  paint buffers also contribute to RSS. The device budget still needs measurement.


## 2.11 Navigation and page ownership

The active tab owns the current document. Inactive `TabSave` entries contain URL, title,
history index, URLs and absolute device-pixel scroll positions. Switching tabs discards live
content/cache from the previous tab, rebuilds the target using disk-cache policy, and restores
its absolute position. Creating a tab stashes the old navigation state once. Closing the last
tab resets its history to Home; closing cards keeps the tab switcher open.

`PreviousPage` bundles a live document with its URL, original HTML, optional reader HTML and
rendering mode. A cache restore swaps that complete state with the current page. Back/Forward
use `Page::scroll_to`, never add a saved offset to an already-scrolled document. Display/image
reflow invalidates the previous layout. The original document source remains available while
reading view renders its own HTML, so Save and subsequent reflow use the matching material.

A URL with the same HTTP(S) origin, path and query is eligible for fragment navigation.
The browser records the new history entry, updates location reads, and jumps in the existing
DOM. IDs are compared as attributes after percent decoding, without interpolating fragment
text into a selector; legacy named anchors and empty/top fragments are supported. Unknown
fragments preserve position. Full loads retain the requested fragment if the response URI
omits it. History traversal restores the recorded position instead of recomputing the jump.
Hashchange/popstate dispatch and script-driven location navigation are not implemented.

Tab navigation metadata is checkpointed in `session.tsv` after page loads, tab closes and
Exit. Both drivers can resume its active tab using disk-cache policy; the desktop bypasses
resume when an explicit start address is supplied. The session codec sanitizes field separators
and clamps active/history indices during resume. It does not persist live DOM or JS state.
Writes are best-effort and non-atomic; crash recovery uses the last written checkpoint. Atomic
writes, surfaced storage failures and complete offline archives remain roadmap work.

## 2.12 `kb-app` module layout

`kb-app` is the browser's logic and its own interface. The crate root only declares modules and
re-exports the API the drivers use. `Browser` is defined in `browser/mod.rs`; each other file in
`browser/` holds one part of `impl Browser`, grouped by responsibility.

| Module | Responsibility |
|---|---|
| `lib.rs` | Module list and the public re-exports (`Browser`, `Update`, `Wave`, layout constants, `normalize`, `width_name`, `encode_png`) |
| `browser/mod.rs` | `Browser` state, construction, and the read-only accessors used by the drivers |
| `browser/nav.rs` | Opening pages, history (Back and Forward), fragments within a page, reflow after a setting change |
| `browser/loading.rs` | Loading and building documents: fetch with progress, forms (POST), reader view, recording visits |
| `browser/session.rs` | Session text codec, checkpoint writes, and startup resume |
| `browser/tabs.rs` | Tabs: switching, creating, closing; each tab keeps its own history, scroll and source |
| `browser/input.rs` | Taps, long presses, scrolling, the press highlight, hit testing for bars, sheets, keyboard and page |
| `browser/actions.rs` | Actions named by `kb:` links: menu, Display settings (text size, width, day or night when standalone), toolbar |
| `browser/library.rs` | Bookmarks, visit history, saved pages |
| `browser/keyboard.rs` | On-screen keyboard: key layers, typing, caret, address suggestions |
| `browser/frame.rs` | Composing the screen: bars, sheets, notices, cached layers, the notice timer, PNG output |
| `browser/internal.rs` | Internal pages: home, bookmarks, saved pages |
| `browser/driver.rs` | Helpers for the desktop driver and the regression suite (find, tap and read elements on screen) |
| `browser/types.rs` | Shared types: updates, cached layers, page and tab state, sheets |
| `settings.rs` | Reading settings, the choices they offer, and `settings.tsv` |
| `storage.rs` | Data files: bookmarks, history, settings, saved pages (tab-separated) |
| `html.rs` | Escaping, and the page-level CSS added from the settings |
| `addr.rs` | Typed input to URL, URL splitting, same-document comparison |
| `layout.rs` | Layout constants in device pixels, and notice placement |
| `ui.rs` | The stylesheet and the HTML for every screen the browser draws itself (§2.6), including the quick settings panel (standalone) |
| `device.rs` | `DeviceControl` trait and `DeviceStatus`: the device-agnostic data behind the quick settings panel; `NoDevice` for the desktop |

Each module starts with a `//!` line that states its job. Tests sit beside the code they check.
