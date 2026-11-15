# 3. Test and validation pipeline

Version 0.1.0. Requirements are in [01-requirements.md](01-requirements.md); the design is in
[02-architecture.md](02-architecture.md). Each test level below states what it proves, how to
run it, and what result counts as a pass.

## 3.1 Levels at a glance

| Level | What it checks | Where it runs | How to run | Pass rule |
|---|---|---|---|---|
| L1 Unit | Pure logic: URL handling, blocklist, image detection, title and script rules, HTML preparation, PNG encoding | Computer | `cargo test --workspace --exclude kb-kindle` | All tests pass |
| L2 Desktop regression | Browser behaviour end to end: navigation, forms, DOM and JavaScript APIs, tabs, bookmarks, reading view, menus, keyboard, scrolling, hit testing | Computer, against local fixtures | `scripts/desktop-regression.sh` | 29 of 29 scenarios pass |
| L3 Live-site smoke | Real sites still load (Hacker News, Wikipedia) | Computer, needs internet | `scripts/desktop-regression.sh --network` | Both pass; a failure here is reported but may be the site, not the browser |
| L4 Launcher | The Kindle UI is stopped and restored in every exit path | Computer, with stub commands | `scripts/launcher-stub-test.sh` | 6 checks pass |
| L5 Device validation | The product on the Kindle: touch, display, performance, memory, durability | Kindle Oasis 3 | Checklist in §3.5 | All must-pass items pass; evidence attached |
| L6 Build and install | The device binary builds, is ARMv7, and matches the launcher | Computer | `cargo build --release --workspace`, `cargo zigbuild ... armv7-unknown-linux-musleabihf`, `sh deploy/install.sh` | Build succeeds; `file` reports ARM; install reports the expected size and start page |

Levels L1, L2 and L4 run on every change. L3 runs before a release. L5 runs before a release
and after any change to input, display or the launcher.

## 3.2 Computer tests

### 3.2.1 Driver commands

`kb-desktop` reads one command per line from stdin and writes the screen to `$KB_OUT` after
each action. Every action prints one line to stderr:

```
[label] action <ms>, frame <ms> | <title> | <url> | scroll <y> / <content height> | ...
```

Commands used by the suite:

| Command | Effect |
|---|---|
| `open <url or text>` | navigate (text becomes a search) |
| `tap <x> <y>`, `scroll <dy>`, `page <+1/-1>`, `longpress <x> <y>` | raw input in device pixels |
| `tapsel <css selector>`, `longsel <css selector>` | scroll the element into view, then tap or long press its centre |
| `tapkey <char>` | tap a keyboard key (`space` for the space bar) |
| `action <name>` | run a named browser action (`mark`, `menu`, `reader`, `night`, `textsize`, `save`, `saved`, `marks`, `tabs`, `newtab`, `closemenu` ...) |
| `back`, `fwd`, `reload`, `tabstep <±1>` | navigation |
| `text <selector>` | print an element's text (`[text] <sel>: ...`) |
| `count <selector>`, `visible <selector>` | matching and displayed element counts |
| `where <selector>`, `linkat <x> <y>` | debugging: screen centre of an element; link under a point |
| `quit` | end |

Environment: `KB_FONTS` (comma-separated font files), `KB_DATA` (data folder, use a fresh
temporary folder per test), `KB_OUT` (frame PNG), `KB_PROGRESS=1` (print load progress),
`KB_KEEP_FIRST=1` (keep the frame drawn right after an action).

### 3.2.2 Fixtures

`tests/fixtures/` holds original test pages and `server.py`, a local server that:

- serves the pages (unknown paths return 404);
- returns a small page for `/search`, so GET form submissions have a target;
- echoes the `city` field on `POST /submit`.

The fixture text is original and carries no third-party content.

### 3.2.3 The regression suite

`scripts/desktop-regression.sh` starts the fixture server on port 8780 (or `KB_PORT`), runs
each scenario in a fresh data folder, and checks the log for expected substrings. It stops
the server when it finishes. Scenarios:

| Group | Scenarios |
|---|---|
| Pages and navigation | `home`, `js_button_keeps_page`, `link_navigates`, `back_and_forward`, `reload`, `not_found_shows_error`, `long_press_opens_new_tab` |
| Forms and input | `form_get_submit`, `form_post_submit`, `keyboard_caret_insert` |
| DOM and JavaScript | `dom_apis`, `js_platform_apis` |
| Reading, saving, bookmarks, tabs | `reading_view`, `save_offline_and_reopen`, `bookmark_add_and_list`, `new_tab_and_switch`, `tabs_sheet_opens` |
| Menu and settings | `menu_scrolls`, `text_size_and_night`, `address_bar_hint_on_home` |
| Live sites (with `--network`) | `hacker_news`, `wikipedia_article` |

A scenario fails when the exit status is non-zero or an expected substring is missing. The
log of each failing scenario is kept in the printed path.

### 3.2.4 Unit tests

Run `cargo test --workspace --exclude kb-kindle`. The device crate is excluded because its
input code only runs on the device. Current coverage:

- `kb-net`: tracker blocklist (including subdomains), image detection by extension and
  path segment, rejection of non-HTTP schemes.
- `kb-engine`: title extraction, classic-script rule (ES modules skipped), text-field types.
- `kb-app`: search-or-URL normalisation, host and path split, page CSS placed in the head
  (standards mode kept), PNG signature, percent encoding.

Unit tests do not cover layout, script execution or the device. Those are covered by L2 and L5.

### 3.2.5 Known computer-test limits

- The suite checks behaviour, not pixels. Visual changes (sizes, colours, icons) are checked
  by looking at the frames written to `KB_OUT` and recorded in the change log.
- The desktop engine is faster than the device. Timings on the computer are a lower bound.
- Live-site results depend on the sites, which change.

## 3.3 Launcher tests

`scripts/launcher-stub-test.sh` replaces the Kindle's `start`, `stop` and the browser with
stubs. It checks that:

1. a normal run stops the UI, then starts it again, and removes the marker;
2. a launcher killed during a run is recovered by the watchdog (the UI is restarted and the
   marker removed);
3. a marker left by an earlier run restores the UI before the UI is stopped again.

These checks prove the logic. They do not prove the Kindle's behaviour; see §3.5, item D-9.

## 3.4 Build and install checks

Before a device test, the build must pass in this order:

1. `cargo build --release --workspace` (no warnings from the browser crates);
2. `cargo test --workspace --exclude kb-kindle`;
3. `scripts/desktop-regression.sh` (29 of 29);
4. `cargo zigbuild --release -p kb-kindle --target armv7-unknown-linux-musleabihf`;
5. `file target/armv7-unknown-linux-musleabihf/release/kb-kindle` reports
   `ELF 32-bit LSB executable, ARM, EABI5` and `statically linked`;
6. `scripts/launcher-stub-test.sh`;
7. `sh deploy/install.sh` reports the binary size and the start page.

## 3.5 Device validation checklist (L5)

Record each item as PASS, FAIL or NOT RUN, with the evidence named. Items marked **must**
block a release.

### Setup

- **D-0 (must)** The binary and launcher on the device match the build: `deploy/install.sh`
  output, and `diff` of the launcher. Evidence: the install output.

### Display and touch

- **D-1 (must)** The home screen appears after launch, with the truyendich bookmark tile.
  Evidence: `browser/frame.png`, the first `ready` line in the log.
- **D-2 (must)** Tapping the bookmark tile opens the page. Evidence: log `open` line with the
  page title.
- **D-3 (must)** On the truyendich page, the mobile-only "continue reading" control is shown
  and tapping it does what it should. Evidence: photo of the screen; log `tap` lines.
- **D-4 (must)** A tap on a link navigates; a tap on a button that calls JavaScript does not
  navigate. Evidence: log lines.
- **D-5 (must)** Scrolling a page moves the content in the direction of the swipe, and the
  bars hide and reappear. Evidence: log `scroll` lines, photo.
- **D-6 (must)** The menu sheet scrolls by swiping on it, and the page underneath does not
  move. Evidence: photo.
- **D-7** The keyboard appears at the bottom; typing inserts at the caret; Go loads the
  address. Evidence: photo of the field with the caret.
- **D-8** Long press on a link opens it in a new tab. A horizontal swipe on the address bar
  switches tabs. Evidence: log lines.
- **D-9 (must)** Exiting from the menu returns to the Kindle library, with the UI fully
  restored. Evidence: photo of the library; log `restoring framework`.
- **D-10 (must)** After a forced kill of the browser (pick a method the owner is comfortable
  with), the UI is restored within 10 seconds. Evidence: photo, log.
- **D-11** The page-turn buttons scroll one screen. Evidence: log `page` lines.

### Performance (record numbers)

- **P-1** Tap feedback: 20 taps; record the time from the touch to the highlight (camera
  timing or log `touch down` to `frame in`). Target NFR-1.
- **P-2** Chrome change (open and close the menu, toggle bars): 10 samples of `action` plus
  `frame` times from the log. Target NFR-2.
- **P-3** Scroll: 10 samples on a long article. Target NFR-3.
- **P-4** First screen of the bookmarked page and of Hacker News: time from `open` to
  `ready`. Target NFR-4.

### Memory and durability

- **M-1** Resident memory: the `rss` value in the action log after opening 10 pages in one
  session. Target NFR-5 (under 250 MB peak).
- **M-2** A page with a looping script (write a test page with `while (true) {}` in a
  script): the page is interrupted within 3 seconds and the browser keeps running. NFR-8.
- **M-3** Cookies: set a cookie on a test site, restart the browser, confirm the cookie is
  sent. FR-2.
- **M-4** Saved page opens with Wi-Fi off. NFR-12.
- **M-5** The e-ink panel shows no visible full-screen flash on routine updates, except the
  periodic clearing flash. NFR-11 (visual).

### Pass criteria

- Every **must** item passes.
- P-1 median under 150 ms, and P-2 median under 400 ms, to meet NFR-1 and NFR-2.
- M-1 peak under 250 MB.
- Any item that fails is recorded in the change log with its evidence.

## 3.6 Evidence to keep

For each device test run, keep under `docs/evidence/<date>/` (not committed when it contains
photos of personal reading material):

- `log-browser.txt` as copied from the Kindle;
- `frame.png` (last frame);
- `probe.txt` when the stock-keyboard question is being checked;
- the checklist with PASS, FAIL or NOT RUN per item, the device firmware, and the commit hash
  of the build.

## 3.7 Reporting

A test run report states: the commit, the command lines used, the pass count per level, the
failing items with evidence, and the open questions. Failures are never hidden by lowering a
target; a target is changed only with a recorded reason in the change log.

## 3.8 Known gaps in the pipeline

- No automated test drives the touch panel or the e-ink panel; both are checked by hand.
- No automated performance test on the device; the log supplies the numbers manually.
- No cross-check against a second browser engine for layout or script behaviour.
- The fixtures cover common paths; sites with unusual markup are only covered by L3 smoke
  tests and the device checklist.
