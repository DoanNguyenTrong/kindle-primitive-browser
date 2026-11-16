# Primitive Browser

A web browser for the Kindle Oasis 3.

A native browser for a jailbroken Kindle Oasis 3 (firmware 5.18.2, KOReader/KPM installed).
HTML, CSS and a JavaScript subset run on the device; the screen is painted in 8-bit grayscale.
The Mac builds and tests the program. No computer or rendering server is needed during use.

Released baseline: **0.1.0**. The current **Unreleased navigation milestone** improves reading
position, tabs, article section links and sheet controls. Desktop and ARMv7 builds pass;
recorded hardware observations are in [device notes](docs/device-notes.md), while full release
validation remains pending. See the [goal and roadmap](docs/04-roadmap.md).

## Current capabilities

| Area | Scope |
|---|---|
| Rendering | Blitz HTML/CSS, including flex/grid; selectable 412/600/800 CSS-px layouts, default 600 |
| Networking | HTTPS with bundled TLS roots, redirects, six parallel sub-resources, deferred images |
| Navigation | Links/section links, Back/Forward with absolute scroll restore, one live previous-page cache |
| Tabs | New/switch/close; per-tab history and positions; inactive documents rebuild from cache; session metadata resumes after restart |
| Input | Custom keyboard with caret, Shift/symbols and suggestions; basic text-field GET/POST forms |
| Reading | Heuristic reading view, 80–200% text size, page width, Day/Night, Images switch |
| Library | Bookmarks with reorder/remove, recent visits, saved HTML, folding Home sections |
| Storage | HTTP cookies, 200 MiB disk cache with offline fallback, persistent display settings |
| E-ink and lifecycle | Cached UI layers, partial refresh, pinned sheet headers, launcher restore/watchdog |

## Compatibility limits

The current runtime is a browser API subset. It does not provide general compatibility with
modern web apps: no ES modules, XMLHttpRequest, WebSockets, workers, service workers, canvas,
WebGL, audio or video. `fetch` blocks synchronously underneath its Promise; observers and
several browser APIs are stubs. Web storage is in-memory. See [requirements](docs/01-requirements.md).

Done/Go on a page field immediately submits its form. Multi-field editing, checkbox/radio/
select controls and complete form event semantics need work. Saved pages contain HTML;
assets are not bundled. Tab metadata resumes from the last saved session; live DOM edits and
unsaved progress after a crash are not retained.
Device speed, memory, touch comfort and panel quality still need measurement.

## Build and install

On macOS, install Rust stable, the `armv7-unknown-linux-musleabihf` target, zig and cargo-zigbuild.
With the Homebrew Rust installation used for this project:

```sh
export PATH="/opt/homebrew/opt/rustup/bin:/opt/homebrew/bin:$PATH"
cargo build --release --workspace
cargo zigbuild --release -p kb-kindle --target armv7-unknown-linux-musleabihf
sh deploy/install.sh     # requires Kindle mounted at /Volumes/Kindle
```

Eject safely and open **Primitive Browser** in the library. `browser/start.txt` sets the start page;
`browser/data/` stores browsing data; `browser/log-browser.txt` holds the bounded log.

The bottom toolbar contains Back, Forward, tabs, bookmark and Menu. Swipe vertically to
scroll, horizontally for Back/Forward, or along the address bar to switch tabs. Page buttons
move a screen. Long press a link to open a new tab. Tap above a sheet to dismiss it. Menu
keeps Exit visible; exiting saves the tabs before returning to the Kindle library. On startup,
saved tabs take priority over `start.txt`, which is the fallback for a new session.

The launcher keeps the Kindle framework running and hides its status bar while browsing.
Exit restores the status bar and opens the library. If that mode leaves a blank library,
create `/mnt/us/browser/stop-framework` to use the older stop/restart fallback.

## Develop and test

```sh
cargo test --workspace --exclude kb-kindle
cargo build --release -p kb-desktop
scripts/desktop-regression.sh       # 67 local scenarios with screenshots; --network adds two live-site checks
scripts/test-all.sh                 # every check, with evidence kept in artifacts/ (see REPORT.md)
scripts/launcher-stub-test.sh       # ten launcher checks with stubs
```

Drive the same engine on the Mac and inspect the frame PNG:

```sh
KB_FONTS=$(ls assets/fonts/*.ttf | paste -sd, -) KB_DATA=$(mktemp -d) \
  KB_OUT=frame.png target/release/kb-desktop kbp://home
# open <url|text> | tap x y | scroll dy | page ±1 | back | fwd | reload
# action menu | action tabs | tapsel <selector> | tapkey <char> | quit
```

Use a fresh `KB_DATA` to isolate tests. The driver also supports assertions for URL, title,
scroll range, tab count, navigation availability and visible controls. Full commands and
hardware release checks are in [test and validation](docs/03-test-and-validation.md).

## Project layout

```text
crates/kb-net       HTTP, cookie jar, disk cache, blocking, resource loading
crates/kb-engine    HTML/CSS layout, scripts, grayscale paint, hit testing, forms, reader
crates/kb-js        QuickJS + native DOM bridge and browser-side prelude
crates/kb-app       Browser state, tabs, history, storage, shared UI and keyboard
crates/kb-desktop   Stdin/PNG driver for local testing
crates/kb-kindle    evdev touch/page buttons and FBInk display
scripts/           Desktop regression and launcher checks
deploy/            Device launcher, installer and probes
vendor/            QuickJS bindings patch for ARMv7 musl
```

Start with the [documentation index](docs/README.md), [roadmap](docs/04-roadmap.md) and
[UI/UX guide](docs/05-ui-ux.md). The [changelog](docs/CHANGELOG.md) separates released history
from current implementation; device evidence is kept distinct from desktop results.
