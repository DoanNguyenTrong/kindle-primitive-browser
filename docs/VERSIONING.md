# Versioning and releases

Primitive Browser uses [Semantic Versioning](https://semver.org/) with a `MAJOR.MINOR.PATCH`
number. Until 1.0.0, the browser is pre-release: a minor bump may change behaviour on the
device.

## One version, in one place

The version is the workspace version in `Cargo.toml`. Every crate inherits it, the menu title shows
it (`Menu v0.1.17`), and the release package is named after it. Nothing else holds a version number.
The changelog and the README quote it; update those when you release.

## Two kinds of build

| Build | Version | How it is made | Where it goes |
|---|---|---|---|
| **Development device build** | Patch bump on every run, for example `0.1.17` → `0.1.18` | `scripts/build-device.sh` | Your own Kindle, through `deploy/install.sh` |
| **Release** | The version in `Cargo.toml`, unchanged | `scripts/package-release.sh`, then a `v<version>` tag | The GitHub release package |

A development build is a snapshot of work in progress. The patch number tells you which device
build is installed; the menu shows it. The bump is undone if the build or the install fails, so a
failed run does not use up a number.

## Releasing

1. **Finish the changes.** Move the `Unreleased` entries in `docs/CHANGELOG.md` under a new
   `## [X.Y.Z] - YYYY-MM-DD` heading. Keep the Keep a Changelog sections (Added, Changed, Fixed,
   Removed).
2. **Set the version.** Edit the `version` line in `Cargo.toml` to `X.Y.Z`. Use the next minor number
   for new features, the next patch number for fixes, and the next major number for breaking changes.
   Commit the change.
3. **Check it passes.** The CI workflow must be green on that commit. Then run the device build once
   and test it on the hardware listed in `docs/03-test-and-validation.md`.
4. **Create an annotated tag** on the commit, named exactly `vX.Y.Z`:

   ```sh
   git tag -a vX.Y.Z -m "Primitive Browser X.Y.Z"
   git push origin vX.Y.Z
   ```

5. **The release workflow** (`.github/workflows/release.yml`) starts on the tag. It checks that the
   tag matches `Cargo.toml`, builds the ARMv7 binary, packages `primitive-browser-X.Y.Z.zip` with its
   `.sha256` file, and attaches both to a GitHub release. Edit the release notes on GitHub, and paste
   in the changelog section.

Tags are never moved or reused. If a release is wrong, publish a new patch version.

## Current state

- Workspace version: **0.1.18**, released as tag `v0.1.18`. Earlier release tags: `v0.1.17`.
- Earlier versions (0.1.0 to 0.1.16) were development device builds, so they have no tags.

## Naming

- Tags: `vX.Y.Z`, with the `v` prefix.
- Release package: `primitive-browser-X.Y.Z.zip`, with `primitive-browser-X.Y.Z.zip.sha256`.
- Branches: `main` is the release branch. Work happens on feature branches merged with a pull request.
