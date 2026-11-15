# Kindle Oasis 3 browser

A native web browser that runs on a jailbroken Kindle Oasis 3 (firmware 5.18.2 with
KOReader installed). Pages are rendered on the device, with real CSS and JavaScript,
and the result is shown in 8-bit grayscale for e-ink.

Nothing runs on the Mac at use time. The Mac only builds the program and copies it over USB.

## What works

| Area | Status |
|---|---|
| Page rendering | HTML and CSS via Blitz (Stylo, Taffy), including flex and grid |
| Networking | HTTPS (rustls with bundled roots), redirects, parallel sub-resources |
| Navigation | Links, back/forward, reload, scroll position restored per history entry |
| Address entry | On-screen keyboard rendered by the browser itself |
| JavaScript | QuickJS with a DOM subset (see below); classic scripts only |
| Cookies | Persistent jar in `data/cookies.json` |
| Cache | Disk cache, 200 MB cap, oldest evicted first; back/forward use cached copies up to 7 days old; offline fallback |
| Bookmarks | Mark/unmark toolbar button, Marks page |
| Reader mode | "Read" rebuilds the article as clean serif text (heuristic) |
| Offline | "Save" keeps a copy; Saved page lists and opens it |
| Ads and trackers | Sub-resources from a built-in list of ad and tracker hosts are not fetched |
| Lifecycle | Stops the Kindle UI while running, restores it on exit, restarts the browser up to 3 times after a crash |

## What doesn't work

- ES modules (`<script type="module">`) are skipped. There is no module loader.
- `innerHTML` reads return text only (no serialisation).
- No `XMLHttpRequest`, WebSockets, `document.write`, or service workers.
- `fetch` is synchronous under a Promise, so a slow request blocks the page.
- Form fields can't be typed into yet (no text input on pages), so login forms won't work.
- Sites that need JavaScript to render their content (many news and app-like sites)
  show whatever the scripts managed to build in a few seconds, which can be little.
- No video, no WebGL, no canvas, no CSS animations.

## Layout

```
crates/kb-net       HTTP(S), cookies, cache, tracker blocklist, sub-resource loader
crates/kb-engine    Blitz page: parse, scripts, layout, paint to gray, links, reader mode
crates/kb-js        QuickJS + DOM bindings (native ops + prelude.js)
crates/kb-app       Browser state: history, bookmarks, saved pages, toolbar, keyboard
crates/kb-desktop   Mac driver (stdin commands, writes PNG) for testing
crates/kb-kindle    Kindle driver (evdev touch and page buttons, FBInk display)
deploy/Browser.sh   Library launcher (framework stop/restore, crash watchdog)
deploy/install.sh   Copies the binary and launcher onto a mounted Kindle
vendor/             rquickjs-sys with musl bindings (see vendor/README.md)
assets/fonts/       Noto fonts for Mac test runs (SIL OFL, see NOTO-LICENSE.txt)
```

## Build and install

Requirements on the Mac: Rust (stable) with the `armv7-unknown-linux-musleabihf`
target, `zig` and `cargo-zigbuild` (all installable with Homebrew).

```sh
export PATH="/opt/homebrew/opt/rustup/bin:$PATH"
cargo zigbuild --release -p kb-kindle --target armv7-unknown-linux-musleabihf
sh deploy/install.sh          # Kindle mounted at /Volumes/Kindle
```

Then eject the Kindle safely and open **KB Browser** from the library.

On the Kindle:
- `browser/start.txt` sets the start page (default: Hacker News).
- `browser/data/` holds cookies, bookmarks, the cache and saved pages.
- `browser/log-browser.txt` is the log (kept to about 200 KB).
- The toolbar has Back, Fwd, Reload, the address bar, Mark, Marks, Read, Save and Exit.
- Swipe up or down to scroll, swipe left or right for forward or back, or use the
  page-turn buttons for a full page.

## Mac testing

`kb-desktop` runs the same engine on the Mac, reading commands from stdin and
writing each frame to `$KB_OUT`:

```sh
KB_FONTS=$(ls assets/fonts/*.ttf | paste -sd, -) KB_DATA=$(mktemp -d) \
  KB_OUT=frame.png target/release/kb-desktop https://news.ycombinator.com/
# commands: open <url|text> | tap x y | scroll dy | page ±1 | back | fwd | reload | action <name> | quit
```

## Limits and notes

- Timing on the Kindle is roughly 5× slower than the Mac for the same page.
- Memory: JavaScript heap is capped at 24 MB, and each sub-resource at 2 MB.
- Scripts that run longer than 3 seconds are interrupted.
- Verify cookies, cache and saved pages on the device before relying on them.

## Documentation

Requirements, architecture, the test and validation pipeline and the changelog are in
[`docs/`](docs/README.md).

## Tests

```sh
cargo test --workspace --exclude kb-kindle        # unit tests
scripts/desktop-regression.sh                     # 20 browser scenarios on local fixtures
scripts/launcher-stub-test.sh                     # device launcher logic, with stub commands
```
