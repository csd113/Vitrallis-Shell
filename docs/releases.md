# Release validation and assets

## 0.1.0-beta2.6

Release `v0.1.0-beta2.6` packages the shell, Terminal, Notepad and Files as one
native generation. It includes the App Center stabilization and UI pass, cached
release notes, targeted local state updates, corrected launcher/menu refresh,
verified update finalization, and a reproducible Docker lifecycle simulator.
See the [App Center audit](app-center-validation.md) for reproduced causes,
regressions, scenario results and software/device boundaries.

The maintainer explicitly authorized version `0.1.0-beta2.6`, commit/push and
GitHub release publication. This authorization does not cover future versions.
The [GitHub release](https://github.com/csd113/Vitrallis-Shell/releases/tag/v0.1.0-beta2.6)
is a beta prerelease.

## Complete asset inventory

Each architecture has a complete four-binary bundle and SHA-256 sidecar:

- `vitrallis-x86_64-unknown-linux-gnu-glibc2.36.vtrbundle`
- `vitrallis-x86_64-unknown-linux-gnu-glibc2.36.vtrbundle.sha256`
- `vitrallis-armv7-unknown-linux-gnueabihf-glibc2.36.vtrbundle`
- `vitrallis-armv7-unknown-linux-gnueabihf-glibc2.36.vtrbundle.sha256`

The ARM job also packages `bootstrap.py`, `install.py`, `uninstall.py` and
`vitrallis-session.py`, each with its own `.sha256` sidecar. The resulting 12
assets come from the tagged checkout. The bootstrap obtains matching helpers
and bundle from one release; native OTA updates replace only the four-binary
generation. Installer/session helper updates require rerunning the reviewed
bootstrap with the Vitrallis session closed.

The earlier published beta2.5 standalone binaries cannot satisfy this contract.
Obsolete pre-release installations/layouts are not migrated. Use the current
complete-bundle installation flow when replacing an obsolete layout.

## Release gates

The tag workflow builds on Debian 12 with Rust 1.91.1 and runs
`sh scripts/validate.sh`: formatting, locked workspace check, strict Clippy,
complete Rust/Python tests, four-binary release build, SDL smoke checks and
repository/documentation validation. Separate host CI also validates Rust 1.91.0.

Packaging checks each executable's target and exact workspace version. ARMv7
cross-builds are checked under QEMU with the Cortex-A8 CPU model, including
version probes, a 480×272 shell frame and native-app smokes. Both architecture
bundles require glibc 2.36+ and SDL2 2.26.5+.

The workflow prepares a draft and refuses to alter an already published release.
Before publication, review the complete asset inventory, verify uploaded bytes
against checksums and bundle contents, and confirm successful release CI.
The release description records those checks for the published artifacts.

Docker App Center validation operates the real Linux SDL shell and installer,
including published Carousel and Debug processes. After publication, a Debian 13
PocketCHIP passed a fresh installation of the published ARM bundle, all four
version probes, supervised shell startup, the live update check and an initial
App Center catalog load with zero installed apps. See the
[hardware follow-up](history/beta2.6-device-validation.md), including the bootstrap
permissions correction on `main`. Published assets and the release tag are unchanged.

These checks do not establish physical touch, storage durability under power
loss, Home/resume, media behavior, removal or installation of a later native
update. The literal README command was fixture-tested; the hardware installation
used verified, locally staged published helpers after manually archiving beta2.5.

## Repository About

Suggested description: **A compact Rust + SDL2 launcher for small-screen Linux,
with native Terminal, Notepad and Files, App Center, and reversible PocketCHIP
integration. Currently in beta.**

Suggested topics: `pocketchip`, `embedded-linux`, `rust`, `sdl2`, `launcher`,
`touchscreen`. Repository settings are unchanged by the release.
