# Product goal and implementation roadmap

Updated: 2026-11-17. Baseline: v0.1.0; latest release **v0.2.1** (2026-11-17).

## Product goal

Make the Oasis 3 useful for daily web browsing: open an address or search, follow links,
fill ordinary forms, read comfortably, keep a place across navigation, and recover from
failures. The complete experience runs on the Kindle. General-purpose web compatibility
is the long-term direction; the current QuickJS bridge supports a subset of browser APIs.

Progress is measured by user tasks, reproducible fixtures, and device evidence. A desktop
pass does not prove touch accuracy, e-ink quality, memory use, or speed on the Kindle.

## Current milestone: dependable reading and navigation

Implementation and desktop validation are complete, and this scope shipped in v0.2.1. **Formal device
validation is not fully recorded:** the owner's tests on the Oasis 3 covered SSH deploy, page turns and the
refresh modes; the D-5 to D-13 checklist and memory and latency figures are still to be written up.

| ID | Outcome | Acceptance / evidence |
|---|---|---|
| NAV-1 | Back and Forward return to the exact reading position. | `history_restores_absolute_scroll`: alternate between two scrolled documents repeatedly; positions stay 500 and 700 device pixels. |
| NAV-2 | A new tab preserves the original tab's history and reading position. | `tabs_preserve_history_and_scroll`; inactive tabs retain metadata, then rebuild from cache. |
| NAV-3 | Closing the active tab leaves the next tab usable; closing the final tab leaves one Home tab. | `close_active_and_last_tab`; the switcher stays open and its count updates. |
| NAV-4 | Instant Back restores the matching document source and reading view. Display changes use current settings. | `cached_source_matches_document`, `reader_survives_back_and_reflow`, `cached_layout_uses_current_settings`. |
| NAV-5 | Article section links work without downloading or recreating the current document. | `fragment_preserves_dom_and_history`, initial/named/missing fragment checks, engine test for encoded Unicode IDs. Back/Forward restore position; changing a query loads a new document. |
| UX-1 | Done and Exit stay reachable while a sheet scrolls. | `menu_controls_stay_visible`, `tab_controls_stay_visible`; headers have independent layout, painting, and hit testing. |
| UX-2 | Page scripts see the selected layout width. | Fixture checks `innerWidth`, `matchMedia` and CSS visibility at 600 and 800 CSS pixels. |
| QA-1 | Existing v0.1 scenarios continue to pass. | 42 local scenarios at v0.1.0; 86 local scenarios, 64 unit tests, ten launcher checks, release and ARMv7 builds at v0.2.1. See the validation report. |

Before the next release, run the device checklist, especially D-5 through D-13, and record memory
and latency.

## Next: forms that work as a complete interaction

Status: implemented and covered by desktop fixtures (`forms_*`, `select_*`, `form_events_*`). Device validation is pending.

The current keyboard's Done/Go commits a text field and immediately submits its form.
That makes multi-field forms awkward and is the next priority.

- Separate **Done** (commit the field) from explicit **Submit/Go**; keep cancellation clear.
- Support textareas, disabled/read-only fields, password masking, checkbox/radio controls,
  select options, and successful-control serialization in document order.
- Deliver `input`, `change`, and cancelable `submit` events to page scripts.
- Avoid automatically replaying POST requests during reload/history navigation.
- Exit gate: local multi-field GET/POST, validation, cancel, and keyboard fixtures pass;
  complete one representative search form and one login-like local fixture on the Kindle.

## Then: durable sessions and offline reading

Status: writes are atomic, and find in page is done. Offline archives with base URL and assets are not started.

- Basic session checkpoint/resume is implemented. Harden crash durability and malformed-file
  handling, improve checkpoint timing, and add an explicit clear-session option.
- Write session, bookmark, and saved-page metadata atomically and report disk failures.
- Make offline saves retain their original base URL, reading layout, and required assets.
- Add find in page with a count and next/previous controls that suit e-ink.
- Exit gate: restart, interrupted-write, and network-disabled tests retain data correctly;
  verify on-device with Wi-Fi off. Saved HTML alone is not a complete offline archive.

## Then: measured performance and broader compatibility

- Record first screen, scroll, sheet refresh, and peak RSS on the Oasis before optimizing.
  Use the targets in the requirements; retain before/after measurements for each change.
- Bound concurrent work, cancel obsolete page resources, and reduce unnecessary paint and
  disk writes where measurements identify a cost.
- Build a representative site matrix (reading, reference, search, forms, JS-heavy pages)
  with exact tasks and dated results. Prioritize failures from that matrix.
- Investigate persistent origin-scoped web storage, browser event semantics, asynchronous
  fetch, XMLHttpRequest, and module loading as separate compatibility projects.
- Reassess the rendering/runtime architecture if the target tasks exceed the current DOM
  bridge. Require an ARMv7 build and device memory evidence before committing to a new engine.

## Working agreement

For each increment: reproduce the problem, add a meaningful behavior check when warranted,
implement the smallest coherent change, inspect affected frames, run the relevant checks,
and update requirements, architecture, and changelog. Keep implementation evidence separate
from device evidence. Do not mark a capability complete because an action was merely logged.
