# Device support

Primitive Browser runs on jailbroken Kindles. Today it is built and tested for one model. This page
says what is supported, what is hard-coded for that model, and the plan for the others.

## Support matrix

| Device | Status | Evidence |
|---|---|---|
| Kindle Oasis 3, firmware 5.18.2 | **Supported** | Hardware: install, browsing, touch, page buttons, orientation, quick settings, exit, launcher |
| Other Kindle Oasis, Paperwhite, Voyage, Basic, Scribe, Touch and earlier | **Not supported yet** | None |

"Supported" means tested on hardware with a recorded result in [device notes](device-notes.md) and
[test and validation](03-test-and-validation.md). Nothing else is supported.

## What is specific to the Oasis 3 today

The first release was written against one device. These values are fixed in the code and must
become per-device data before another model can run.

| Value | Where | Oasis 3 value |
|---|---|---|
| Screen size | `crates/kb-kindle/src/main.rs` (`NATIVE_W`, `NATIVE_H`) | 1264 × 1680 |
| Layout, in device pixels | `crates/kb-app/src/layout.rs`; CSS sizes in `crates/kb-app/src/ui.rs` | Bars, sheets and keyboard sized for 1264 px wide |
| Touch device | `crates/kb-kindle/src/main.rs` (`find_input("cyttsp")`) | `cyttsp5`, touch range 0–1263 × 0–1679 |
| Page-turn buttons | `crates/kb-kindle/src/main.rs` (`KEY_PAGEUP`, `KEY_PAGEDOWN`) | keycodes 104 and 109 |
| Frontlight, warmth | `crates/kb-kindle/src/device.rs` | `com.lab126.powerd` `flIntensity`, `currentAmberLevel`, range 0–24 |
| Battery and charger | `crates/kb-kindle/src/device.rs` (`BATTERY_SYSFS`, `CHARGING_SYSFS`, `CHARGER_STATUS_SYSFS`) | `max77796-battery` and `max77796-charger` |
| Screen orientation | `crates/kb-kindle/src/main.rs` (`display_turn`) | `com.lab126.winmgr accelerometer`, with FBInk as fallback |
| Framework and window manager | `deploy/Browser.sh` | `pillow` status bar, `lab126_gui` upstart job |
| FBInk | `crates/kb-kindle/src/main.rs` (`FBINK`) | `/mnt/us/libkh/bin/fbink` |

The Oasis 3 touch controller reports in its upright frame, and the browser turns each touch when the
screen is upside down. Another model may report differently, so turning must be tested per device.

## Where the other models differ

KOReader supports the other Kindles and keeps their hardware facts in
`frontend/device/kindle/device.lua`, one block per model. Its power and frontlight paths are the
starting point for the device table. Examples read from that file:

- **Oasis 2 and Oasis 3** use `max77796-*` battery, charger and frontlight nodes.
- **Basic 2** uses `bd7181x_bat` nodes.
- **Touch and older models** use `yoshi`, `luigi` and `wario` battery nodes under `/sys/devices/system`,
  and some Touch models need a touch coordinate scale.

Per-model values (screen size and DPI, touch device name and ranges, page-button keycodes, power-supply
paths, frontlight range) must come from the device or from KOReader's table, and be checked on
hardware. They are not guessed.

## Plan for other models

Each step is testable on the desktop build before a Kindle is involved.

1. **Device profile (M1).** One table of per-model values in `kb-kindle`: screen size, touch name,
   page-button codes, power-supply paths, frontlight range, and the framework init type. Detect the
   model at start from the device, and fall back to the Oasis 3 profile only when it matches.
   The current Oasis 3 values stay as the default profile.
2. **Layout for any screen (M2).** The layout is written in pixels for 1264 × 1680. Two options,
   to decide:
   - **Scale:** lay out at the reference size and scale the picture to the panel. Simple, and every
     layout stays the same. Text becomes too small on the smallest screens.
   - **Responsive:** let bars, tiles and the keyboard reflow to the panel's width and height, and
     scale text by the panel's DPI. More work, and better on small screens.
3. **Desktop checks at each size (M3).** The desktop driver renders any size, so each profile gets
   screenshots and regression scenarios before hardware tests.
4. **Hardware testing (M4).** Each model needs a tester with that device. Its results go into
   `docs/device-notes.md`, and only then is the model listed as supported.
5. **Builds per architecture (M5).** The current build targets ARMv7 hard-float
   (`armv7-unknown-linux-musleabihf`). Older Kindles may use another ABI, so check each model before
   building for it.

## Decisions needed

- **Layout approach** for M2: scale, or responsive.
- **First model** after the Oasis 3, and who can test it.
- **Older models** with the SysV init and no `lab126_gui` job: support them, or leave them out?
