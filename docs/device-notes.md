# Device notes: Kindle Oasis 3 (firmware 5.18.2)

Facts established from the device log (`/mnt/us/browser/log-browser.txt`), kept here so they are not
rediscovered. Each entry says where it was seen.

## Screen orientation

- **The orientation source is the framework's property `com.lab126.winmgr accelerometer`.** Values:
  `U` (upright), `D` (upside down). `R` and `L` are landscape, which the Kindle does not use here.
  The browser reads it once a second (`display_turn_watch` in `crates/kb-kindle/src/main.rs`).
- **It only answers while the window manager (`awesome`) runs.** With `awesome` paused, the property
  returned no value in every read (1 `None`, 0 values). With it running, every read gave a value
  (3 of 3). Do not pause `awesome`.
- **FBInk's `-e` output does not give a rotation value in this build.** `fbink -e` does not print
  `currentRota`, so the framebuffer turn is always `None` in the log. FBInk's draw lines still print
  `rotation: N (…)` for each draw. The framebuffer value is logged beside the framework value only for
  comparison. It is not used for touch or buttons.
- **Screen turns observed:** `rota 0` at start, `rota 2` (upside down) and back to `rota 0` (log, session
  of 15:36 on the v0.1.12 build).

## Touch and buttons

- **Touch coordinates are in the panel's native upright frame** (x 0–1263, y 0–1679, device
  `/dev/input/event8`, `cyttsp`) whatever the turn. The browser turns each touch by the current turn
  (`turn_point`). Verified: raw (955,617) becomes screen (308,1062) upside down.
- **Page buttons:** keycode 109 (`KEY_PAGEDOWN`) is forward and 104 (`KEY_PAGEUP`) is back, upright.
  Upside down they trade places (`page_buttons_swapped`).

## Launcher and the UI

- **Default mode ("keep"):** the status bar is hidden with `lipc-set-prop com.lab126.pillow
  disableEnablePillow disable`. The framework keeps running. On exit the status bar is shown again and
  the library is opened with `lipc-set-prop com.lab126.appmgrd start app://com.lab126.booklet.home`.
  Measured: the library opened on the first try after exit (`opened library home after 0 tries`).
- **Fallback mode:** create the file `stop-framework` in the browser folder. The launcher then stops
  `lab126_gui` at start and starts it again on exit. Measured: `start lab126_gui` took about 9 s
  (launcher log, 14:59:49 to 14:59:58).
- **The status bar** is the framework's `default_status_bar` pillow. It is hidden while the browser
  runs. Its content on this device has not been checked.

## Tried and dropped

- **Native status bar and swipe-down panel over the browser** (v0.1.13, flag file `show-status-bar`:
  the launcher did not hide the status bar). Result on the device: no status bar was visible, and a
  swipe down from the top edge did not open the Kindle's panel while the browser ran. The browser's own
  quick settings panel stays in use. The experiment's code was removed.

## Gyro (not used now)

- The accelerometer's interrupt device is `/dev/input/event7` (`bma_interrupt`), with `ABS_PRESSURE`
  orientation codes. Upright was 15, 17 and 19. Upside down was 16, 18 and 20. The codes alternated
  between 16 and 17 when the Kindle was held tilted, for example lying on a bed. That is why the turn now
  comes from the framework property instead.
