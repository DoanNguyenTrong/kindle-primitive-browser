# 1. Requirements

Status date: 2026-11-16. Version: 0.1.0 (see [CHANGELOG](CHANGELOG.md)).

## 1.1 Purpose

A web browser for a jailbroken Amazon Kindle Oasis 3 (1264 x 1680 e-ink panel, 300 ppi,
firmware 5.18.2, KOReader and the KPM launcher installed). The browser runs natively on the
device. No server runs on a computer during use.

The product goal is a **fully functional browser**: it must load and run ordinary
JavaScript-driven websites, support normal navigation and the usual browser features, and
be usable with one hand on the touch screen. A partial browser that renders static pages
is not the goal.

## 1.2 Definition of "fully functional"

The browser is fully functional when every acceptance criterion in this section passes on
the device and in the desktop regression suite. Each criterion is marked with its current
status:

- **Met**: implemented and verified by an automated test or a recorded device check.
- **Partial**: implemented, with known gaps listed in the requirement.
- **Open**: not implemented, or not yet verified.

| ID | Acceptance criterion | Status |
|---|---|---|
| AC-1 | Loads public HTTPS pages and follows redirects. | Met (desktop); device HTTPS verified once in M1 |
| AC-2 | Renders HTML and CSS with flexbox, grid and media queries (phone width). | Partial: Blitz 0.3 beta, see FR-3 |
| AC-3 | Runs classic page scripts; timers, events and DOM edits work. | Partial: see FR-5 |
| AC-4 | Runs a React/Next.js-style streamed page far enough to show its content. | Partial: a streamed-chunk fallback exists; hydration is not guaranteed |
| AC-5 | Navigation: links, back, forward, reload, history, new tab, tab switch, close tab. | Met (desktop suite) |
| AC-6 | Forms: text input, GET and POST submission, Done/Go on the keyboard. | Met for text fields; partial for other input types (see FR-6) |
| AC-7 | Cookies persist across sessions. | Met (desktop test, one session boundary) |
| AC-8 | Cache for offline back/forward; saved pages open offline. | Met (desktop) |
| AC-9 | Bookmarks, reading view, text size, night mode. | Met (desktop); reading view is a heuristic |
| AC-10 | Touch: tap, long press, scroll, swipe, page-turn buttons. | Partial: implemented; device touch mapping not yet confirmed |
| AC-11 | Survives failures: a crashing page does not stop the browser; the Kindle UI is always restored. | Met in tests; device restore needs confirmation (see §5) |
| AC-12 | Latency: a tap shows feedback at once, and the screen updates within the target in §3.1. | Open: not yet measured on the device |
| AC-13 | Memory stays inside the budgets in §3.2 on the device. | Open: not yet measured on the device |
| AC-14 | Text entry with the built-in keyboard. | Open: blocked on whether the stock keyboard can be reached from a native app |

## 1.3 Functional requirements

Each functional requirement (FR) has a status and the evidence behind it.

| ID | Requirement | Status | Evidence / gap |
|---|---|---|---|
| FR-1 | Fetch http(s) documents with redirects, timeouts and a size cap. | Met | `kb-net::fetch`, unit tests, regression `reload`, `not_found_shows_error` |
| FR-2 | Keep cookies between sessions. | Met | `kb-net` cookie jar saved to `data/cookies.json` |
| FR-3 | Lay out and paint HTML/CSS. Web pages use a 412 CSS-px layout width, scaled to the panel. | Partial | Blitz 0.3.0-beta.4 is a beta: some CSS features are missing. Visibility of phone-only blocks is verified. |
| FR-4 | Fetch sub-resources in parallel; show progress; images load after the text and nearest the top first. | Met | `SyncNet`, `Page::image_order`, progress pump |
| FR-5 | Run page JavaScript (QuickJS): DOM API, events, timers, Promises, fetch, storage, common browser globals. | Partial | See §1.4 for the list of what is missing |
| FR-6 | Forms: text, search, url, email, tel, number, password fields; GET and POST submit; submit buttons. | Partial | Checkboxes, radios, selects, file inputs, date pickers and multipart forms are not supported |
| FR-7 | Navigation: history with scroll restore, one page kept for instant back, reload, error pages with retry. | Met | regression `back_and_forward`, `reload`; desktop `[back]` timings |
| FR-8 | Tabs: new, switch, close, card switcher, swipe along the address bar. | Met (desktop); swipe gesture unverified on device | regression `new_tab_and_switch`, `tabs_sheet_opens` |
| FR-9 | Address bar with search fallback; on-screen keyboard with caret, suggestions, shift and symbols. | Met (desktop) | regression `keyboard_caret_insert` |
| FR-10 | Bookmarks (default truyendich bookmark, seeded once), history of recent pages, saved pages for offline. | Met | regression `bookmark_add_and_list`, `save_offline_and_reopen` |
| FR-11 | Reading view (article extraction) and night mode. | Partial | Reading view is a heuristic: it can pick the wrong block on unusual layouts |
| FR-12 | Text size control (zoom). | Met (desktop, visual check) | regression `text_size_and_night` |
| FR-13 | Notices, progress strip, error pages with plain-language reasons. | Met | toasts and progress verified visually on desktop |
| FR-14 | Ad and tracker blocking by host list. | Met | `kb-net::is_blocked` unit tests |
| FR-15 | Device lifecycle: stop and restore the Kindle UI; crash watchdog; log kept small. | Met in the test harness; device restore after an unplug is not confirmed | `deploy/Browser.sh`, stubbed tests in §4.4 |
| FR-16 | Input: tap, long press (opens a link in a new tab), swipe scroll, horizontal swipe for back/forward, page-turn buttons. | Partial | Touch thresholds are implemented; not confirmed on the touch panel |
| FR-17 | Night mode and settings persist between sessions. | Met | `data/settings.tsv` |
| FR-18 | Find in page, print, downloads manager, password manager, sync, extensions. | Open | Not implemented |

## 1.4 Web platform coverage (FR-5 detail)

Implemented in the JavaScript layer (`crates/kb-js/src/prelude.js`):

- `document`: `getElementById`, `querySelector(All)`, `getElementsByClassName/TagName/Name`,
  `createElement`, `createTextNode`, `title`, `body`, `head`, `documentElement`, `cookie`.
- `Element`: attributes, `classList`, `dataset`, `style` (set), `textContent`, `innerHTML`
  (set), `insertAdjacentHTML`, `matches`, `closest`, `contains`, sibling properties, `append`,
  `appendChild`, `insertBefore`, `removeChild`, `remove`, `addEventListener`, `click`.
- Events: `Event`, `CustomEvent`, `EventTarget`, `DOMContentLoaded` and `load`.
- Timers: `setTimeout`, `setInterval` (runs once), `clearTimeout`, `requestAnimationFrame`,
  `requestIdleCallback`, `queueMicrotask`.
- Network: `fetch` (synchronous underneath, Promise-shaped), `URL`, `URLSearchParams`.
- Storage: `localStorage`, `sessionStorage` (in-memory, not persisted across sessions).
- Utilities: `AbortController`, `AbortSignal`, `crypto.getRandomValues`, `crypto.randomUUID`,
  `TextEncoder`, `TextDecoder`, `btoa`, `atob`, `performance.now`, `structuredClone`,
  `ReadableStream` (minimal), `console`.
- Observers: `IntersectionObserver` reports every target as visible once;
  `MutationObserver`, `ResizeObserver`, `PerformanceObserver` never fire.
- Environment: `navigator`, `location`, `history` (no-op), `matchMedia` (width queries),
  `innerWidth`, `innerHeight`, `devicePixelRatio`.

Not implemented (open):

- ES modules (`type="module"`), dynamic `import()`, `import` syntax.
- `XMLHttpRequest`, WebSockets, Server-Sent Events, Web Workers, Service Workers, WebRTC.
- `document.write`, `Range`, `Selection`, the full `Node` and `Event` hierarchies.
- Canvas 2D, WebGL, WebAudio, video and audio elements, Web Components beyond plain elements.
- `Intl` beyond what QuickJS ships; `localStorage` persistence across sessions.
- `innerHTML` read (returns text only).
- Full observer semantics (MutationObserver, ResizeObserver).

## 1.5 Non-functional requirements

Targets are proposals until measured on the device. The measurement method is in
[03-test-and-validation.md](03-test-and-validation.md).

| ID | Requirement | Target (proposed) | Status |
|---|---|---|---|
| NFR-1 | Tap feedback (highlight) appears | under 150 ms after the touch ends | Open (device) |
| NFR-2 | Chrome-only change (bars, sheets) refreshes | under 400 ms | Open (device) |
| NFR-3 | Page scroll repaint | under 1.5 s on a typical article | Open (device) |
| NFR-4 | Text-only page of about 100 KB, first screen | under 6 s on the device | Open (device); desktop about 1 s |
| NFR-5 | Resident memory | under 250 MB peak with two pages open | Open (device) |
| NFR-6 | JavaScript heap per page | 48 MB cap, enforced | Met (configuration) |
| NFR-7 | Disk cache | 200 MB cap, evicts oldest first | Met (configuration, unit-tested logic) |
| NFR-8 | A page script that loops forever | interrupted within 3 s | Met (desktop test pending) |
| NFR-9 | Kindle UI restored after exit or crash | always | Met in the harness; confirm on device |
| NFR-10 | No data leaves the device except HTTP requests the user makes | always | Met by design: no telemetry code exists |
| NFR-11 | E-ink: no full-screen flash on routine updates | flash at most every 8 text refreshes | Met by design; visual check on device pending |
| NFR-12 | Works without internet for saved pages and cached pages | yes | Met (desktop) |

## 1.6 Constraints

- Hardware: Kindle Oasis 3, i.MX ARMv7 (32-bit), about 512 MB RAM (to confirm on device),
  flash shared with the book library. Touch panel 1264 x 1680.
- Software on the device: KOReader provides fonts (`/mnt/us/koreader/fonts/noto`) and FBInk
  (`/mnt/us/libkh/bin/fbink`). The browser ships its own binary; it does not need glibc.
- Build host: macOS on Apple silicon. Cross-compilation uses zig (through cargo-zigbuild).
- No system fonts and no fontconfig on the device. Fonts are loaded from files.
- The stock Kindle keyboard is part of the Amazon UI framework. Its reachability from a
  native process is unknown (see §5).

## 1.7 Out of scope for version 0.1

Extensions, sync, accounts, password manager, printing, downloads manager, video playback,
WebGL, multiple windows, touch zoom with pinch, handwriting input, and any server-side
component.

## 1.8 Open questions for the owner

1. Can the stock Kindle keyboard be shown by a native app? Needs the output of
   `browser/probe.txt` from the device (`KB Probe` in the library).
2. Is a 412 CSS-px phone width the right default, or should a desktop width be selectable?
3. Which languages must the UI support beyond English? (Vietnamese text renders; UI strings
   are English only.)
