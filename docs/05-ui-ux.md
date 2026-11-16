# Kindle interface and interaction guide

The interface should remain legible in grayscale, respond predictably to touch, and keep
reading interruptions short. `kb-app/src/ui.rs` defines the shared visual language;
`kb-app/src/browser/` owns interactions and composition (see [02-architecture.md](02-architecture.md) §2.12).

## Layout and controls

All chrome dimensions below are **device pixels**, for the 1264 × 1680 panel. Web layouts
use a separate CSS viewport: Narrow 412, Medium 600 (default), or Wide 800 CSS pixels.
Do not use web CSS coordinates for touch hit testing.

| Element | Geometry / rule |
|---|---|
| Address bar | 96 px high; 68 px pill; host emphasized, path secondary |
| Bottom toolbar | 104 px high; Back, Forward, tabs, bookmark, Menu |
| Sheets | 780 px high; pinned 72 px header; remaining 708 px scroll independently |
| Sheet dismissal | Done always visible; Menu also keeps Exit browser visible |
| Rows | 96 px minimum height; 24 px side padding; text labels for settings |
| Primary targets | Aim for at least 68 × 68 px; tab close targets are 68 px |
| Keyboard | Bottom 730 px; static caret, Shift, symbols, arrows, Clear and Cancel |
| Type | 32 px rows, 26 px secondary labels, 22 px captions, 44 px page titles |

Keep the five toolbar actions stable. Use icons with accessible names and plain-language
labels in sheets. Disabled navigation must be inert. Use text or shape to communicate a
state; gray shading alone is insufficient. Long titles must not overlap close controls.

## Reading flow

- A navigation shows the destination address and loading progress before waiting.
- Back/Forward restore an absolute position. The page's DOM, source, and reading view must
  agree after an instant Back; display changes must not revive stale layouts.
- In-document links jump immediately and retain the live DOM. Missing targets leave the
  reading position intact; an empty fragment returns to the top.
- Scrolling down hides chrome; scrolling up reveals it. A tap on blank page space toggles
  chrome. Page buttons move by about a screen with overlap.
- An open sheet owns scroll gestures. Its header remains fixed and hittable while its body
  moves. Dismissing the sheet returns to the page beneath it.
- Inactive tabs retain navigation metadata, not live JS execution. Returning rebuilds the
  document and restores the position; unsaved DOM edits are not promised across tab switches.

## E-ink behavior

Use immediate, static tap feedback; no decorative animation or blinking caret. Reuse cached
layers and refresh the changed rows. Pages and sheets use GL16 for text quality; short
keyboard/progress feedback may use DU. The device driver periodically requests GC16 to clear
ghosting. Day/Night changes request a full flash. These policies still need device review.

Show a useful plain-language error and a reachable recovery action. Keep loading progress
visible without painting each arriving image; current image refreshes are throttled to
1.5 seconds. Avoid notices covering the control the user is adjusting.

## Review each UI change

Inspect the affected desktop PNG at panel size and check long titles, empty state, both
themes, selected settings, and scrolled sheets. Verify hit testing through actual taps in the
driver. Then check the changed interaction on the device, recording failures and timings in
[the validation checklist](03-test-and-validation.md). Desktop screenshots cannot establish
physical target comfort, panel contrast, ghosting, or response time.

Known follow-ups: toolbar labels/accessibility audit, persistent sessions, multi-field
editing, and measured one-handed usability. The custom keyboard is the supported input path;
access to the stock Kindle keyboard is optional research.
