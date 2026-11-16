# Contributing

Thank you for helping. Primitive Browser is small and careful about the device, so changes are
checked on the desktop first and then on hardware.

## Before you start

- Read the [README](README.md) and the [versioning](docs/VERSIONING.md) notes.
- For a new Kindle model, read [device support](docs/DEVICES.md) first. A model is only listed as
  supported after a hardware test, and its results go in [device notes](docs/device-notes.md).

## Make a change

1. Create a branch from `main`: `git switch -c <short-name>`.
2. Make the change. Keep it focused, and follow the style of the code around it.
3. Add or update tests:
   - Unit tests live beside the code (`cargo test --workspace`).
   - Browser behaviour is checked with a desktop regression scenario in
     `scripts/desktop-regression.sh`.
   - Launcher changes are checked with `scripts/launcher-stub-test.sh`.
4. Run the checks:

   ```sh
   cargo test --workspace
   cargo build --release -p kb-desktop && sh scripts/desktop-regression.sh
   sh scripts/launcher-stub-test.sh
   cargo zigbuild --release -p kb-kindle --target armv7-unknown-linux-musleabihf
   ```

5. Update `docs/CHANGELOG.md` under `## [Unreleased]`, in the Keep a Changelog sections.
6. Open a pull request. CI must pass. Say which checks you ran, and whether the change was tried on a
   Kindle, with the model and firmware.

## Device changes

A change that touches the device (input, display, the launcher, power or the framework) needs a
hardware test on the Oasis 3 before it is merged, and a note of the result in the pull request. If
you can only test on another model, say so; that test does not count as supported.

## Style

- Rust: `cargo fmt` for new code. Keep crates separate as the [architecture](docs/02-architecture.md)
  describes: the engine knows nothing about the device.
- Shell: POSIX `sh`, which runs on the Kindle's BusyBox.
- Documentation: short sentences, and say what is verified and what is not.

## Reporting problems

Open an issue with the Kindle model, the firmware version, the Primitive Browser version (shown in the
Menu title), what you did, what happened, and the last lines of `browser/log-browser.txt`. Do not
include cookies or the contents of `browser/data/`.

## Security

Report security problems privately, as described in the [README](README.md#security).
