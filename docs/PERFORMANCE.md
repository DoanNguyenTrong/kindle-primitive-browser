# Memory and page-turn measurements

These numbers come from the desktop build on macOS (arm64), measured on 2026-11-16. They are **not
device evidence**. The Oasis has less memory and a slower, e-ink display. Use these numbers to
decide what to measure on the device, not as the answer.

## Peak memory (`/usr/bin/time -l`, maximum resident set size)

| Case | Peak |
|---|---|
| Browser with only the home page | 32 MB |
| Large text page (846 KB, 4,000 paragraphs, 150 small images), images off | 109 MB |
| The same page, images on | 177 MB |

What this says: about 77 MB goes to the text page itself (DOM, layout, script heap and caches).
About 68 MB more goes to 150 small images, which is roughly 0.45 MB per image on the desktop. That
per-image cost is the first thing to measure on the Oasis. Images are off by default for web pages,
so the common case avoids it.

## Page turns (driver timings, `page 1`, large page)

| Measurement | Desktop | Oasis (device log) |
|---|---|---|
| Engine action (`action`) | 0 ms | 0.7 ms |
| Frame (`frame`: render of the screen, in the browser) | 8 ms | 480–570 ms for a full-screen turn |

The `frame` value is the time the browser takes to render the screen (`Browser::frame`): the paint of
the 1264 x 1680 viewport and the conversion to gray. It does not include the e-ink update (`fbink`,
about 165 ms for a full screen on the device, whatever the waveform). Most of a device page turn is
therefore the render, not the screen update. The device log now prints `[render] settle, paint, gray`
for each render, to show which of the two dominates.

Frames are written as an 8-bit BMP in RAM (`/var/tmp/kb-frame.bmp`), replacing the PNG on the storage
partition. That change did not reduce the full-screen turn, because the render was the cost.

## Still to do on the device

- Repeat both tables on the Oasis with the same fixture, using `/usr/bin/time`-equivalent figures
  from the `[kb] action … rss` log lines.
- Measure peak memory with 0, 20 and 150 images, to confirm the per-image cost.
- Test a real large article (Gutenberg, once that site is back) with images off and on.
