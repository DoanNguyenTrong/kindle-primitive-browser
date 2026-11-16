# Changelog

All notable changes to this project are recorded here. The format follows
[Keep a Changelog](https://keepachangelog.com/en/1.1.0/). Versions follow
[Semantic Versioning](https://semver.org/) once a release is tagged; until then the
workspace version is `0.1.0`.

## [Unreleased]

### Added
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
- Desktop regression suite `scripts/desktop-regression.sh` (42 scenarios on local fixtures,
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
