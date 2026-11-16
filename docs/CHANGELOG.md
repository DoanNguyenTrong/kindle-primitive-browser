# Changelog

All notable changes to this project are recorded here. The format follows
[Keep a Changelog](https://keepachangelog.com/en/1.1.0/). Versions follow
[Semantic Versioning](https://semver.org/) once a release is tagged; until then the
workspace version is `0.1.0`.

## [Unreleased]

### Added
- Long-list tests: 40 seeded bookmarks (the `SEED` option of the regression runner) cover the
  address suggestions (scroll, and a tap far down the list), filtering by typing, the home grid
  scrolled to its last tile, and the bookmark manager (scroll, and remove a row far down).
  `gallery_long_lists` saves the scrolled states as screenshots.
- Address bar selection: opening the address on a page selects the URL, so the first key
  replaces it instead of appending. While it is selected, the suggestions list the bookmarks and
  recent pages. Tapping a character places the caret and clears the selection.
- Test pipeline `scripts/test-all.sh`: build, unit tests, desktop regression, launcher tests and
  the Kindle build, with the evidence kept in `artifacts/` and a summary in `artifacts/REPORT.md`.
- Screenshots in the repository: each regression scenario has a folder (`script.txt`, `log.txt`,
  `final.png`); the `gallery_pages` scenario captures every page and sheet as named shots;
  `artifacts/desktop/INDEX.md` lists them. The driver gained a `shot <name>` command.
- Menu redesigned as a four-column grid of tiles (PAGE, SAVE, BROWSER), with a state caption on
  each tile. Night mode is removed from the menu (it is in Display), and the menu no longer has a
  Done button; tap the page above the menu to close it. Exit browser stays in the title strip.
- Dependable reading/navigation milestone with explicit acceptance criteria in
  `docs/04-roadmap.md`, plus a Kindle interaction and e-ink guide in `docs/05-ui-ux.md`.
- Article section links: encoded Unicode IDs, legacy named anchors, empty/top fragments,
  missing-target behavior, and Back/Forward positions without rebuilding the live document.
- Pinned sheet headers: Done stays visible in Menu, Display and Tabs; Exit browser remains
  visible in Menu after scrolling. Tab close targets are 68 px with title space reserved.
- Desktop `assert` commands for URL, title, scroll range, tab count, Back/Forward state and
  visible controls; failures terminate the scenario.
- Documentation set in `docs/`: requirements, system architecture, test and validation
  pipeline, and this changelog.
- Desktop regression suite `scripts/desktop-regression.sh` (61 scenarios on local fixtures,
  including 13 navigation/state/sheet cases; opt-in live-site checks with `--network`).
- Display sheet (menu > Display): text size steps (80% to 200%) with A- and A+, page width
  (Narrow, Medium, Wide) and Day or Night. Changes apply at once and the sheet stays open.
- Page width setting (`page_width` in `settings.tsv`). Web pages lay out at 600 CSS px
  (Medium) by default, with 412 px (Narrow) and 800 px (Wide) available; device usability
  remains to be measured.
- Bookmark manager (`kbp://bookmarks`): each bookmark shows its title and address, with open,
  move up, move down and remove buttons.
- Home sections fold away: tap the BOOKMARKS or RECENT heading to show or hide the list. The
  choice is kept in `settings.tsv`.
- Images switch in the menu (PAGE > Images, On by default). Off answers image requests
  without the network or the decoder, so text-only reading uses less memory; the page is
  rebuilt when it changes. Videos are not played: the engine has no video support, with
  images on or off.
- Launcher test `scripts/launcher-stub-test.sh` (stub commands; seven checks).
- Unit tests for URL handling, the blocklist, image detection, title and script rules, HTML
  preparation and PNG encoding.
- Desktop driver commands: `tapsel`, `longsel`, `tapkey`, `text`, `where`, `linkat`,
  `visible`, `count`, `tabstep`, `longpress`.
- Device log now records resident memory (`rss`) after each action.

### Changed
- Renamed to **Primitive Browser**: library launcher `documents/Primitive Browser.sh` (the
  installer removes the old `KB Browser.sh`), user agent `PrimitiveBrowser/0.1`, and the logo
  (`assets/primitive-icon.png`, from `primitive-ai-website/public/prai-icon.png`) on the home
  screen. The logo is built into the binary, so it shows offline.
- `data:` URLs now load (base64 and percent-encoded). Before, they were answered empty, so inline
  images on web pages were blank.
- Address bar while typing: the URL field is in the same 96 px bar as the page's address bar,
  with Cancel where the reload button sits. The Go button in the header is removed; the keyboard's
  Go key submits. The suggestions are a separate list between the bar and the keys, and they
  scroll with a swipe (the bar and the keys stay in place). The list keeps up to 60 matches,
  bookmarks first (the list was limited to 6).
- The driver's `visible` check now means the element's centre is on the part of the screen the
  user sees: below the address bar and above the toolbar, the open sheet's body, or the keyboard's
  list above the keys. It used to count any element with a box, including elements scrolled away
  or under the keys.
- Home screen redesigned: bookmarks as a four-column grid of tiles (the menu's tile style), recent
  pages as rows with a title and a URL, and each section has a text control, Show or Hide, in
  place of the chevron. Empty states are padded. "Manage" opens the bookmark page. The two
  bottom buttons are removed (the menu has Bookmarks and Saved pages).
- Display and Tabs sheets no longer have a Done button. Tap the page above a sheet to close it.
- `kb-app/src/lib.rs` (about 2,100 lines) is split into modules by responsibility: `browser/`
  (state, navigation, loading, tabs, input, actions, library, keyboard, frame, internal pages, driver),
  `settings.rs`, `storage.rs`, `html.rs`, `addr.rs` and `layout.rs`. The behaviour is unchanged:
  all 61 regression scenarios and the unit tests pass. The layout is described in
  `docs/02-architecture.md` §2.12.
- Web pages use the selected 412/600/800 CSS-px width and scale to the panel; default is
  Medium (600). JavaScript viewport dimensions, pixel ratio and basic width queries agree
  with the renderer instead of always reporting 412 px. Chrome and internal pages keep
  device pixels.
- Address bar is 96 px (was 120 px); bottom toolbar is 104 px (was 130 px).
- Interface redesigned to a single layout system (`crates/kb-app/src/ui.rs`): grey pill
  address field with a site badge, icon-only bottom toolbar with a tab count and a
  three-dot menu, grouped menu with icons and dividers, card-grid tab switcher, and
  folding bookmark/recent sections on Home.
- Touch: a tap is a touch under 800 ms with less than 40 px of movement; a long press is a
  touch held 800 ms or more; a vertical swipe scrolls from 80 px; horizontal swipes need
  180 px. A tap during a document download no longer cancels it.
- Image repaints during loading are limited to once every 1.5 s.
- Cache budget raised to 200 MB; JavaScript heap raised to 48 MB; sub-resource limit 4 MB;
  document limit 6 MB.
- Default bookmark is added once for existing devices (`defaults_v2` marker), so removing
  it does not bring it back.
- Menu and tab sheets scroll.

### Fixed
- JavaScript-heavy pages (React and Next.js, jQuery) failed or went blank. Found on tinhte.vn
  from the device log and fixed at the cause, each with a regression check (`dom_api_for_pages`):
  - `innerHTML` assignments did nothing: the document had no HTML parser for fragments. The
    Blitz HTML parser is now installed.
  - `getAttribute` returned `undefined` for a missing attribute, so every `appendChild` was taken
    for a fragment and dropped (introduced earlier in this release). It returns `null`.
  - Text nodes were hidden from scripts (`childNodes`, `firstChild`, `nextSibling`), so React's
    hydration never matched the server markup (React error 418) and the page was cleared.
  - Missing DOM: `document.nodeType`, `URL`, `referrer`, `currentScript`, fragments, `cloneNode`
    (native, deep), `attributes`, `ownerDocument`, `isEqualNode`, `dispatchEvent`, siblings,
    `HTML*Element`/`Text`/`SVGElement` classes for `instanceof`, `performance.clearMarks`, layout
    stubs. jQuery 1.11 failed on the first of these (`createElement` of undefined).
  - `fetch` ignored its method, headers and body, did not resolve relative addresses, and had no
    response content type. It now sends them through a new `kb_net::request`.
  - A script answered with an HTML page (an error page) was run as JavaScript; it is skipped.
  - Removed nodes reached by later script calls made Blitz panic, which replaced the whole page
    with an error page. The DOM calls and each page-building step are contained, and Blitz is
    patched to ignore removed nodes (`vendor/blitz-dom-0.3.0-beta.4`, see `vendor/README.md`).
  - Errors from event handlers, timers and `console.error` now log their first stack frames.
- Slow page loads (about 8 s on the device for a script-heavy page): external scripts were
  fetched one at a time. They are now fetched six at a time and still run in document order
  (regression `scripts_run_in_document_order`). On the Mac, tinhte.vn took 14 to 24 s before and 4 to 6 s after (network-dependent).
- Saved pages did not load their own scripts and stylesheets (the device log showed
  `kbp://saved/...: unsupported scheme`). The saved-page metadata is `url title`, but the file
  writer turned the tab between them into a space, so the lookup found no URL and the page fell
  back to its internal address. The metadata now splits on the first space, which reads the
  entries already saved on the device (regression `saved_page_runs_its_scripts`).
- A page restored with Back took its internal address as its base for relative links, so its
  links stopped resolving. Internal addresses no longer change the base; only web addresses do
  (unit test `internal_location_keeps_the_web_base_for_relative_links`, regression
  `saved_page_links_after_restore`).
- Pages that check for IndexedDB failed with `ReferenceError: IDBIndex is not defined`. The
  IndexedDB names now exist, and opening a database fails quietly (regression
  `indexeddb_feature_checks_do_not_throw`).
- Address suggestions showed the bookmark icon for recent pages too. Recent pages now have their
  own icon.
- JavaScript error reports now include the first three stack frames, so the device log shows
  where an error was thrown (script URL and line).
- Tapping a suggestion while typing an address did nothing: suggestion rows are browser actions,
  but the keyboard sent them to the key handler, which ignored them. They now open the page
  (regression `suggestion_tap_navigates`).
- Menu rows for Bookmarks, Saved pages and other `kbp://` pages did nothing: the action table
  ignored them. Sheet links to internal and web pages now open them (regression
  `menu_items_navigate` checks every tile).
- Page scripts saw a stale `location.href` after a fragment jump: `href` is a getter, which
  `Object.assign` does not copy. It is now set explicitly (regression `fragment_updates_location_href`).
- Switching tabs cleared the page but not its HTML, so a cancelled load left the previous tab's
  source under the new address; "Save for offline" or a text-size change could then store or
  rebuild the wrong page. The source and reader state are now cleared on switch.
- The reader view laid out at device width, so the page-width setting did nothing while reading.
  It now uses the selected web width.
- Reflow, text-size and reader rebuilds recorded the page in history again. Visit recording
  now happens once per navigation (`record_web_visit`).
- The launcher stub test could block forever if the launcher hung; it now fails after 30 s.
- Back/Forward added saved offsets to already-scrolled live pages; restoration now sets an
  absolute position and swaps the document with its matching source and reader state.
- Opening a new tab stashed the original tab twice, discarding its history. Tabs now retain
  history and reading positions; switching clears live content from the other tab. Closing
  the final tab resets to one Home tab and closing cards keeps the switcher usable.
- Display changes could rebuild the wrong source after instant Back, lose reading view, or
  restore a cached layout using old settings. Source/view state travels together and reflow
  invalidates the previous layout.
- Unit tests no longer fail to compile when Settings gains fields: fixtures use full defaults.
- Keyboard caret regression now submits to a local form fixture instead of a live search
  service. The saved-page regression opens the saved document and checks its title.
- Regression output directories are created when `KB_TEST_OUT` is supplied. Launcher checks
  now wait for the stale-marker recovery run and verify its marker is cleared (seven checks),
  avoiding cleanup while that run is still writing logs.
- Following a link with the bars hidden showed no address bar, and the progress strip landed
  on page content. The address bar now shows the new URL before the load starts, and the
  progress strip sits under it.
- Exit browser could leave the Kindle on the app drawer, which showed a blank screen. The
  launcher now waits for the framework to start and then opens the library home.
- The home screen had two search boxes (the address bar and a search pill). The pill is removed.
- Taps on a scrolled page hit the element at the same height at the top of the page instead
  of the one under the finger (hit testing now adds the scroll offset).
- The top of a page was hidden under the address bar; pages are now painted below it while
  the bars are shown, and touches are mapped back to page coordinates.
- Mobile-only page sections (for example a continue-reading control) were hidden because the
  layout width was desktop-sized. Fixed by the phone-width layout.
- Streamed server-rendered content (hidden `S:n` chunks) is moved into place when a page's
  own swap script fails.
- A tap on a slightly moving finger was read as a swipe; a slow tap was read as a long press
  and opened a new tab.
- Module scripts (`type="module"`) were being run as classic scripts, which produced errors.
- New tab kept the previous tab's address.
- Kindle UI could be left stopped after the browser was killed; the launcher now has a
  watchdog that restores it, and restores a stale stop on the next launch.
- Missing DOM methods (`getElementsByClassName`, `classList`, `dataset`, `matches`,
  `closest`, `contains`, `insertAdjacentHTML`, sibling properties) and browser APIs
  (`AbortController`, `crypto`, `URL`, `TextEncoder`/`TextDecoder`, `btoa`/`atob`,
  `performance`, `matchMedia`, `ReadableStream`, `IntersectionObserver`, `DOMContentLoaded`
  and `load` events).
- Menu on the device flashed and disappeared. Every menu choice closed the sheet, and each
  sheet redraw used the fast DU waveform, which leaves ghosting and erased text on large
  grey areas. Now toggles (night mode, text size, bookmark, save for offline, clear cache,
  clear history) keep the menu open and show the new state; only navigation closes it.
  Sheet, bar and notice redraws use GL16 (text quality) instead of DU.
- The menu had no visible exit: the Exit row was below the fold of the 780 px sheet. Exit is
  now a button in the menu's title strip, always on screen.
- Device log: every sheet tap and every chrome action is logged with the sheet state, for
  example `[ui] sheet=Menu tap (x,y) -> Some("kb:night")` and `[ui] action kb:night (sheet=…)`.

### Known issues
- A site's own poll widget (tinhte.vn) still reports `Could not find job from response`: its
  batching client does not find its job in the batch reply. The article renders; only the poll
  is affected. Not investigated further.
- Layout queries from scripts (`getBoundingClientRect`, `offsetWidth`, ...) report zero: the page
  layout is not exposed to scripts. Code that only measures and continues works.
- The desktop driver's `tapsel` cannot target an inline link (a link inside a sentence): the
  element has no box of its own. Taps on the device still work for inline links.
- Stock Kindle keyboard access remains unknown; the browser uses its own keyboard.
- Latency, memory and e-ink behaviour are not yet measured on the device.
- Touch gestures and the progress strip have not been confirmed on the touch panel.
- JavaScript coverage is partial: no ES modules, XMLHttpRequest, WebSockets, workers, canvas,
  WebGL or video (see requirements §1.4).
- Form controls other than text fields (checkbox, radio, select, file) are not supported.
- Done/Go commits a page field and submits immediately; multi-field editing, form events,
  and POST reload/history behavior need improvement. POST coverage uses text fields.
- Saved copies retain HTML only; offline assets, atomic data writes and restart/session
  restoration are future milestones.
- Fragment navigation updates location reads but does not dispatch hashchange/popstate.
- Blink-level CSS fidelity is limited by Blitz 0.3.0-beta.4.

## [0.1.0] - 2026-11-16

First complete build of the browser for the Kindle Oasis 3, and the desktop build used to
test it. Work done in milestones and rounds, summarised:

### Milestone M0: feasibility (2026-11-15)
- Rendering spike: Blitz renders HTML and CSS to 8-bit gray; the ARMv7 static build works.
- Display on the device through the device's FBInk (`libkh`); a grayscale PNG shows on the
  panel.
- Network on the device: TLS with bundled roots; fetch, render and display in one run.

### Milestone M1: desktop browser (2026-11-15)
- Workspace split into `kb-net`, `kb-engine`, `kb-app`, `kb-desktop`, `kb-kindle`.
- Links, scroll, history, toolbar, on-screen keyboard for the address.
- Parallel sub-resource loading, so a Wikipedia article loads in about a second on the
  desktop.

### Milestone M2: device lifecycle (2026-11-15)
- Launcher stops the Kindle UI while the browser runs and restores it on exit.
- Crash watchdog restarts the browser up to three times.
- Refresh policy: text-quality waveform for pages, periodic full flash to clear ghosting.

### Milestone M3: daily use (2026-11-15)
- Persistent cookies; disk cache with offline fallback; bookmarks; reading view; saved
  pages for offline; tracker blocklist.

### Milestone M4: JavaScript (2026-11-15)
- QuickJS (rquickjs 0.14) with the DOM bridge and browser-side prelude; scripts run at load;
  click listeners with `preventDefault`; timers; synchronous `fetch` under a Promise.
- Vendored `rquickjs-sys` with musl bindings so the device build links.

### Milestone M5: hardening (2026-11-15)
- JavaScript heap cap and interrupt after 3 s; sub-resource and document size caps; panic
  guard around each event; plain-language error pages.

### Interface rounds (2026-11-15 to 2026-11-16)
- Cached interface layers so a tap re-renders only what changed; partial refresh with the
  fast waveform for bars and keyboard.
- Tap highlight before the action runs; progress strip while loading; deferred images;
  notices above the toolbar.
- On-screen keyboard after the stock layout structure (layout only, not code); caret;
  suggestions; input mode for page fields with Done; GET and POST form submission.
- Tabs, menu and reading settings (text size, night mode); long press to open in a new tab.

### Device-facing fixes (2026-11-16)
- Launcher watchdog and stale-stop recovery after the browser was killed.
- Phone-width layout, streamed-chunk fallback, and the scroll hit-test fix (see Fixed above).
