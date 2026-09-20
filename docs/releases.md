# Release validation and assets

## 0.1.0-beta4

Adds shared, Shell-managed Arti as a separate supervised process, on-demand Tor
application requirements, fail-closed required-app networking and keyboard/touch
Tor Settings. Arti 2.6.0 is the fifth executable in the complete v2 bundle.
See [Tor architecture](tor.md) and [Tor validation](tor-validation.md).

Existing users update through Settings. beta3.9 installs the full beta4 bundle
directly. Users skipping beta3.9 receive a compatible four-executable beta4 entry
point, then repeat Check/Install/Relaunch once to complete the same release with
Arti. Complete installations do not repeat the update. See the
[upgrade paths and prerequisites](beta4-upgrade.md) for the tested contracts.

## 0.1.0-beta3.9

A four-executable update bridge that prepares existing Shell installations for
beta4's full five-executable bundle and Arti's independent version. It preserves
the existing atomic generation-switch transaction and ordinary app behavior.
It does not include the Tor service. No manual reinstallation is needed.

## 0.1.0-beta3.1

This release improves App Manager installation synchronization, keeps the Apps
footer focused on Actions, adds static running-app badges and persistent local
folders, and supports precompiled Rust application packages alongside Python.
Native packages select a compatible Linux target, validate ELF architecture and
permissions, and use the existing install, update and uninstall transactions.

Python dependency provisioning waits for process completion and output EOF,
preserves useful failure diagnostics, and reports dependency work separately from
the application commit. Folder deletion returns apps to the normal view without
uninstalling them. Folder actions, navigation and confirmations remain usable with
keyboard and touch.

The [PocketCHIP validation record](devices/pocketchip/app-manager-validation.md)
documents the development build's Python and Rust lifecycles, folder and keyboard
checks, resource samples and the user's clean physical display observation. The
release artifacts are rebuilt from the versioned tag and follow the CI, checksum
and bundle review gates below.

## 0.1.0-beta3

This release merges the GPU hardware acceleration work into main. The shell,
Terminal, Notepad and Files share SDL renderer selection with automatic software
fallback. Renderer artwork caching and a shared font atlas reduce repeated work;
graphics diagnostics identify software Mesa renderers and reject them when
hardware acceleration is explicitly required.

See the [hardware acceleration guide](hardware-acceleration.md),
[rendering performance report](rendering-performance.md) and
[PocketCHIP graphics validation](devices/pocketchip/graphics-phase3.md) for
configuration, measurements and the scope of recorded device checks. Those device
checks cover the development build; release artifacts follow the gates below.

## 0.1.0-beta2.7.2

App Center now installs missing declared Python dependencies automatically in an
isolated app-local environment, including Carousel's Pillow dependency. Failed
provisioning cleans up its staged environment, existing environments are retained,
and app code is validated before installation. System Python/Tk and venv support
remain prerequisites; dependency downloads require network access.

This release also includes the removal of obsolete shell implementations already
on main. Validation uses the host and release CI gates below; this patch has no
recorded device validation.

## 0.1.0-beta2.7.1

This patch fixes the built-in PocketCHIP Fn layer in the launcher and native
apps. Fn+1–0, minus and equals produce F1–F12; Terminal receives Fn punctuation
without an unintended Alt/Meta Escape prefix. Desktop keyboards retain their
existing behavior. See the [device keyboard notes](devices/pocketchip.md#built-in-fn-keyboard)
for device verification and its limits.

Both architecture bundles and matching helpers follow the release gates below.

## 0.1.0-beta2.7

Release `v0.1.0-beta2.7` adds native desktop shortcuts for ordinary Linux
programs and scripts, including executable/icon browsing, editing, explicit
shell mode and native terminal launching. Keyboard, mouse and touch actions use
provenance-specific removal: custom records are removed without deleting their
targets, managed apps use App Center's existing uninstall transaction, and
unmanaged entries are hidden only. See the [shortcut guide](desktop-shortcuts.md)
and [validation audit](desktop-shortcuts-validation.md).

This release also includes the stock PocketHome/session integration and matching
`install-session.py` helper added since beta2.6. Release CI builds and checks both
architecture bundles; the draft's uploaded assets must pass the review below
before publication.

## 0.1.0-beta2.6

Release `v0.1.0-beta2.6` packages the shell, Terminal, Notepad and Files as one
native generation. It includes the App Center stabilization and UI pass, cached
release notes, targeted local state updates, corrected launcher/menu refresh,
verified update finalization, and a reproducible Docker lifecycle simulator.
See the [App Center audit](app-center-validation.md) for reproduced causes,
regressions, scenario results and software/device boundaries.

The [GitHub release](https://github.com/csd113/Vitrallis-Shell/releases/tag/v0.1.0-beta2.6)
is a beta prerelease.

## Current source asset inventory

The complete beta4 bundle contains Shell, Terminal, Notepad, Files and Arti:

- `vitrallis-x86_64-unknown-linux-gnu-glibc2.36-v2.vtrbundle`
- `vitrallis-x86_64-unknown-linux-gnu-glibc2.36-v2.vtrbundle.sha256`
- `vitrallis-armv7-unknown-linux-gnueabihf-glibc2.36-v2.vtrbundle`
- `vitrallis-armv7-unknown-linux-gnueabihf-glibc2.36-v2.vtrbundle.sha256`

beta4 additionally publishes both architecture bundles under the original names
(without `-v2`) as four-executable OTA entry points, each with its own sidecar.
The ARM job packages `bootstrap.py`, `install-session.py`, `uninstall.py`,
`vitrallis-session.py`, `platform-setup.py` and `media-setup.py`, each with its own
sidecar. beta4 therefore has 20 assets. beta3.9 has the original 16-asset inventory.
The matching beta4 bootstrap uses the complete v2 bundle directly.

Native OTA updates switch the binary generation and preserve installed session
helpers and user configuration. The narrowly scoped four-file transition exists
for the explicitly supported beta3.9/beta4 upgrade; older standalone layouts
remain outside the managed updater contract. See [upgrade details](beta4-upgrade.md).

## Release gates

The tag workflow builds on Debian 12 with Rust 1.91.1 and runs
`sh scripts/validate.sh`: formatting, locked workspace check, strict Clippy,
complete Rust/Python tests, native release build, SDL smoke checks and
repository/documentation validation. Separate host CI also validates Rust 1.91.0.

Packaging checks each executable's target and version: the four Vitrallis binaries
match the workspace version; Arti independently reports 2.6.0. ARMv7
cross-builds are checked under QEMU with the Cortex-A8 CPU model, including
version probes, a 480×272 shell frame and native-app smokes. Both architecture
bundles require glibc 2.36+ and SDL2 2.26.5+.

The workflow prepares a draft and refuses to alter an already published release.
Before publication, review the complete asset inventory, verify uploaded bytes
against checksums and bundle contents, and confirm successful release CI.
The release description records those checks for the published artifacts.

Published validation and its limits are recorded in the [hardware follow-up](devices/pocketchip/history/beta2.6-device-validation.md).
Those beta2.6 artifacts predate the stock-session integration changes. Beta2.7
packages matching bundles and helpers; the previous hardware checks do not
constitute physical-device validation of beta2.7.
