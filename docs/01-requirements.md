# 1. Requirements

Status date: 2026-11-16. Released baseline: 0.1.0; current changes are Unreleased.
The [roadmap](04-roadmap.md) defines the next implementation milestones and release gates.

## 1.1 Purpose

A native browser for a jailbroken Kindle Oasis 3 (1264 × 1680 e-ink panel, firmware 5.18.2,
KOReader and the KPM launcher). It runs on the Kindle without a computer or rendering server
at use time. The Mac builds the executable and provides a desktop driver for development.

The product should support everyday browsing: addresses and search, readable pages, links,
forms, tabs, bookmarks, offline reading, and predictable recovery. Broader compatibility with
ordinary JavaScript-driven websites remains a product goal. The current engine and DOM
bridge have material gaps; passing the local fixtures does not establish general web support.

## 1.2 Acceptance criteria and evidence

**Desktop verified** means an automated local behavior check covers the stated scope.
**Partial** means a capability exists with a known limitation or incomplete evidence.
**Device pending** means hardware evidence must still be recorded before release. Historical
v0.1 device observations are in the changelog; they do not validate the current build.

| ID | User outcome | Current evidence / status |
|---|---|---|
| AC-1 | Load public HTTPS pages and follow redirects. | Implemented; v0.1 device smoke recorded. Current live-site/device validation pending. |
| AC-2 | Read HTML/CSS pages with responsive layouts. | Partial: Blitz 0.3 beta; selectable 412/600/800 CSS-px widths, Medium by default. CSS visibility and JS width checks pass locally. |
| AC-3 | Page scripts can update the DOM and handle clicks. | Partial: classic scripts, timers, Promises and an API subset; `dom_apis`, `js_platform_apis`, `js_button_keeps_page`. |
| AC-4 | Read content from streamed server-rendered pages. | Partial: hidden-chunk fallback; React/Next hydration and module-dependent apps are not guaranteed. |
| AC-5 | Follow links and section links; use Back/Forward and tabs without losing the reading position. | Desktop verified: navigation/fragment/tab scenarios in the validation document. Inactive tabs rebuild and do not retain live DOM edits. Device pending. |
| AC-6 | Complete ordinary multi-field GET/POST forms. | Partial: text fields and submission work; Done immediately submits. Multi-field flow and other controls are next on the roadmap. |
| AC-7 | Keep cookies between sessions. | Implemented HTTP cookie jar; session-boundary and device evidence still needed. JavaScript `document.cookie` is an in-memory approximation. |
| AC-8 | Revisit cached pages and open saved reading material offline. | Partial: cache fallback and saved HTML exist; fixture opens a saved document. Assets are not bundled; network-disabled device check pending. |
| AC-9 | Bookmark pages and choose reading view, text size, page width and theme. | Desktop verified for listed interactions; extraction is heuristic. Device appearance pending. |
| AC-10 | Use taps, long press, swipes and page buttons comfortably. | Driver and thresholds implemented; current hardware touch/gesture validation pending. |
| AC-11 | Recover from page errors and return safely to the Kindle library. | Panic guards/error pages implemented; seven launcher checks pass. Hardware restore/forced-kill validation pending. |
| AC-12 | See prompt feedback and acceptable screen update latency. | Device measurements pending; targets below. |
| AC-13 | Stay inside device memory budgets. | Limits configured; peak process RSS on device pending. |
| AC-14 | Enter an address or page-field text without the stock Kindle UI. | Custom keyboard works in desktop fixtures. Device validation pending; stock keyboard access is optional research. |

## 1.3 Functional scope

| ID | Requirement | Implementation and remaining work |
|---|---|---|
| FR-1 | HTTP(S), redirects, timeout and document size limit. | Implemented in `kb-net`; desktop 404/reload checks. |
| FR-2 | Persistent HTTP cookies. | `data/cookies.json`; restart/privacy behavior needs dedicated coverage. |
| FR-3 | HTML/CSS rendering with responsive layout. | Blitz; 600 CSS-px default, 412 and 800 selectable. Internal pages/chrome use device pixels. |
| FR-4 | Parallel resources, progress and deferred images. | Six concurrent sub-resources; top-first image order; 1.5 s image repaint throttle; Images switch skips network/decode for images. |
| FR-5 | JavaScript browser APIs. | QuickJS with the subset below; no standards-conformance claim. |
| FR-6 | Text entry and forms. | Text/search/url/email/tel/number/password inputs; rudimentary textarea path; GET/POST. Done commits and submits immediately. No checkbox/radio/select/file or multipart flow. |
| FR-7 | History and absolute scroll restoration. | Desktop verified, including repeated instant Back/Forward and fragment entries. Live cache includes source and reader state; reflow invalidates stale cached layouts. |
| FR-8 | New/switch/close tabs and tab switcher. | Desktop verified; closing the final tab resets to Home. Inactive tabs store navigation metadata; sessions are checkpointed in `session.tsv` and resumed after restart. Crash recovery uses the last written checkpoint; live DOM state is not persisted. |
| FR-9 | Address/search input with caret, suggestions, Shift and symbols. | Desktop verified. Suggestions come from bookmarks and recent visits. |
| FR-10 | Bookmarks, recent history, saved pages. | Persistent lists; bookmark ordering/removal and folding Home sections implemented. One default bookmark is seeded once. Saves are HTML-only. |
| FR-11 | Reading view and night mode. | Implemented; reading view survives live-cache restoration/reflow. Extraction can choose the wrong block. |
| FR-12 | Display settings. | Text steps 80–200%; Narrow/Medium/Wide; Day/Night; settings persist. |
| FR-13 | Notices, progress and recoverable errors. | Implemented; Menu/Display/Tabs use pinned dismissal controls. Device quality pending. |
| FR-14 | Host-list ad/tracker blocking. | Sub-resource host checks unit-tested; this is a small built-in list. |
| FR-15 | Kindle lifecycle, watchdog and bounded logs. | Launcher harness passes; actual firmware behavior pending. |
| FR-16 | Touch and physical-button input. | Implemented in `kb-kindle`; long press opens links in a new tab; address-bar swipe switches tabs. Hardware checks pending. |
| FR-17 | Persistent reading/Home/image settings. | `data/settings.tsv`; writes are best-effort and not atomic yet. |
| FR-18 | Find, downloads, print, passwords, sync and extensions. | Open. Find is planned; the other features are outside the current milestone. |

## 1.4 Web platform coverage (FR-5)

The implementation is in `crates/kb-js/src/prelude.js` and the native DOM bridge. Names here
mean subset implementations, not full browser semantics.

- Document lookup, creation, title/body/head, attributes, `classList`, `dataset`, style writes,
  text, HTML insertion, tree edits, selectors, sibling traversal, click listeners.
- `Event`, `CustomEvent`, `EventTarget`, load events; basic timers, Promise jobs and scheduling.
  `setInterval` runs once. DOM input/change/submit semantics are incomplete.
- Promise-shaped `fetch` with synchronous network work; basic `URL` and `URLSearchParams`.
- `localStorage` and `sessionStorage` share in-memory storage for a page; no disk persistence.
- Basic AbortController, crypto helpers, text/base64 utilities, performance, streams and console.
- `IntersectionObserver` reports visible once. Mutation/Resize/Performance observers are stubs.
- Navigator and location snapshots; fragment navigation updates location reads. JS viewport
  width/height/pixel ratio match the layout; `matchMedia` supports basic width conditions.

Known gaps: ES modules/import; XMLHttpRequest; WebSockets/SSE; workers/service workers;
full Node/Event/Range/Selection APIs; canvas/WebGL/audio/video; full Web Components;
accurate computed style and observers; `document.write`; `innerHTML` serialization (reads
return text); persistent origin-scoped storage; a real History API and script-driven
navigation. Fragment jumps do not dispatch `hashchange`/`popstate` events.

## 1.5 Non-functional targets

Targets remain provisional until measured on the Kindle. See the validation document for
methods. A configured limit is distinct from a measured whole-process resource budget.

| ID | Requirement | Target / current status |
|---|---|---|
| NFR-1 | Tap feedback after touch ends | under 150 ms; device pending |
| NFR-2 | Chrome/sheet refresh | under 400 ms; device pending |
| NFR-3 | Article scroll repaint | under 1.5 s; device pending |
| NFR-4 | First screen of a ~100 KB text document | under 6 s; device pending |
| NFR-5 | Peak process RSS | under 250 MB with normal tab navigation; device pending |
| NFR-6 | JS heap | 48 MiB per live page configured; not a total process cap |
| NFR-7 | Disk cache | 200 MiB configured, oldest entries evicted; device durability pending |
| NFR-8 | Runaway script | interrupt budget of 3 s per script/timer pass; dedicated durability check pending; synchronous I/O can exceed that duration |
| NFR-9 | Restore Kindle UI on exit/crash | seven harness checks; hardware pending |
| NFR-10 | Privacy | no telemetry; network requests come from navigation/pages; privacy review remains separate from this milestone |
| NFR-11 | Routine e-ink refresh | partial text-quality refreshes; periodic full clear every eight text updates; hardware pending |
| NFR-12 | Saved/cached material offline | HTML/cache implemented; full asset retention and network-disabled verification pending |

Other configured bounds: 6 MiB document, 4 MiB sub-resource, 2 MiB fetched script. Multiple
live documents, paint buffers, fonts, images and worker responses also consume memory.

## 1.6 Constraints and open decisions

- ARMv7 musl static executable; hardware has approximately 512 MB RAM (confirm on device).
- Fonts come from KOReader files, without fontconfig; display uses the device's FBInk.
- macOS/Apple silicon development; Rust plus zig/cargo-zigbuild for the device build.
- English UI; Vietnamese content/fonts supported. Further UI language support is undecided.
- Default Medium layout is a product choice to validate on the 7-inch panel; Narrow remains
  available for sites whose mobile layout is more usable.
- No server-side component, extensions, sync, password manager, video, WebGL, multiple windows,
  or pinch zoom in the current milestone. Stock keyboard integration is not a release gate.
