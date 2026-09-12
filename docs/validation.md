# Validation

Run the complete host gate from the repository root:

```sh
sh scripts/validate.sh
```

It runs these required Rust checks, with the lockfile enforced where applicable:

```sh
cargo fmt --all --check
cargo clippy --workspace --all-targets --all-features -- -D warnings -D clippy::all -D clippy::pedantic -D clippy::nursery -D clippy::cargo
cargo test --workspace --all-features
```

It also checks the workspace, runs Python unittest discovery, builds all four
release binaries, verifies their versions, exercises SDL dummy-driver smoke tests
at 480×272 and 800×480, checks shell/Python syntax and local Markdown links, and
runs `git diff --check`. Rust tests intentionally exclude the separately invoked
online application-contract test. SDL2 development libraries and pkg-config are
host prerequisites; native application/runtime tests use normal-user fixtures.

## Installation and removal fixtures

Python tests isolate filesystem writes in temporary HOME directories, including
paths with spaces. They mock release downloads, runtime preflight and session
commands where host execution cannot represent the ARM device. They exercise:

- Complete bundle install, repeat install, removal and reinstall; all four binaries.
- Failed/truncated/corrupt downloads, absent or duplicate helpers/checksums,
  wrong ABI and version disagreement before publication.
- Edited menu fields, shortcuts, helpers, generation content and retained data.
- Symlinks, hardlinks, malformed receipts, escaping pointers and unsafe paths.
- Shared locks, failed writes, interruption recovery and later-edit conflicts.
- Dry runs, default data preservation and literal purge confirmation.
- The literal README shell commands with mocked transport/session boundaries.
- Release inventory and complete source-archive rebuild from another directory.

A staged command test proves command sequencing, cleanup and local installation
behavior. It does **not** prove that a compatible release exists at a live URL.
[Release validation](releases.md) records the separate release asset checks.

## Visual and device scope

The README image is a genuine current desktop-build SDL render at 480×272,
exported through `--screenshot`. Preview its Markdown at desktop and narrow/mobile
widths; the image must scale and command blocks must remain copyable.

No current physical-device validation is implied. Before certifying PocketCHIP,
check real input and readable controls, ABI/startup, PTY behavior, Home/resume,
update/relaunch, offline removal/purge, rollback on actual storage, and endurance.
[Native validation](native-validation.md) retains host/container measurements.
[Historical hardware evidence](history/device-validation.md) retains earlier
Marshmallow integration observations. The scopes must remain distinct.

## Repository lifecycle validation before beta2.6 — 2026-09-12

| Check | Result and scope |
| --- | --- |
| `sh scripts/validate.sh` | Passed on macOS: formatting, workspace check, strict Clippy, 183 Rust tests passed (one explicit online test ignored), 77 Python tests, four-binary release build/version checks, SDL smokes, shell/Python syntax, local links and diff whitespace. |
| Python 3.8 / Linux | 64 installer, uninstaller, bootstrap and session tests passed in an isolated, network-disabled `python:3.8-slim` container as an unprivileged user. Runtime, network and session boundaries use fixtures. All four device scripts also passed Python 3.8 syntax parsing. |
| Literal README commands | Passed install, reinstall, dry run and removal with temporary HOME and mocked transport/session/runtime probes. Failed initial downloads were not executed; temporary scripts were cleaned. Corrupt release assets prevented installation. |
| Interruption and edit preservation | Passed rollback, later-edit conflict, committed cleanup retry, and actual local CLI recovery after the installer helper had already been removed. Dry runs leave no import caches in the installation. |
| Release packaging | Nine fixture tests passed, including the four-binary inventory, exact helper bytes/checksums, ARM hard-float rejection and tag/version agreement. No new real ARM artifacts or remote CI result is claimed. |
| Markdown and repository forms | All 26 maintained Markdown files passed local path/anchor checks. Both workflows and all three issue-template YAML files parsed. README HTML from GitHub's Markdown API was reviewed locally at 390px and 1100px widths; commands stayed copyable, the page did not overflow, and Uninstall was the final section. |
| Live URLs | The bootstrap URL returned HTTP 404. Official release metadata confirmed published beta2.5 has standalone binaries only; public one-line installation remains blocked. No live installer was executed. |

No physical device was connected during that repository audit. Version numbers and
the dependency lockfile were unchanged in that pass. The later App Center audit is
recorded in [App Center validation](app-center-validation.md), and the separately
authorized beta2.6 release is documented in [release validation](releases.md).
