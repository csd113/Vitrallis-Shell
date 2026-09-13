# Vitrallis Shell

**Small screens. Big possibilities.**

A compact Rust + SDL2 launcher for PocketCHIP and desktop Linux. Open a terminal, jot down a note, browse your files, and make a small screen feel useful again. Vitrallis runs alongside Marshmallow, with a way home when you need it.

[![Validate](https://github.com/csd113/Vitrallis-Shell/actions/workflows/validate.yml/badge.svg)](https://github.com/csd113/Vitrallis-Shell/actions/workflows/validate.yml) [![Releases](https://img.shields.io/github/v/release/csd113/Vitrallis-Shell?include_prereleases&label=release)](https://github.com/csd113/Vitrallis-Shell/releases)

![Vitrallis at 480×272: Terminal, Notepad, Files and App Center, with Terminal selected](docs/images/shell-480x272.png)

*Current desktop build rendered at 480×272; hardware status is unavailable.*

- **Three native essentials:** Terminal with a real PTY, a text-editing Notepad, and Files for browsing and everyday file operations. All ship with the shell and work offline.
- **Room to explore:** App Center checks GitHub catalogs and installs selected manifest packages, with source trust, checksums, and local-edit protection.
- **Keys or touch:** visible selection and shared activation across native controls, with confirmations for destructive actions.
- **PocketCHIP integration:** brightness, volume, status, Wi-Fi utility access, and a supervised session that restores Marshmallow's Home binding on exit.
- **Whole-build updates:** System Settings updates the shell and all three native utilities together when a compatible newer release is available.

## Install on PocketCHIP

**Beta.** Release `v0.1.0-beta2.6` uses a complete bundle containing the shell, Terminal, Notepad and Files, plus matching installation/removal helpers. The command below selects a published compatible release. A fresh complete-bundle installation, shell startup, update check and initial App Center load passed on a Debian 13 PocketCHIP; see [hardware results and limits](docs/history/beta2.6-device-validation.md).

On a compatible device, open a terminal as your **normal desktop user**, save any Vitrallis work, close its session, and copy this entire line:

```sh
(set -eu; t=$(mktemp); trap 'rm -f "$t"' 0; trap 'exit 130' 1 2 15; curl -q -fSL --proto '=https' --proto-redir '=https' --connect-timeout 10 --max-time 30 --max-filesize 262144 https://raw.githubusercontent.com/csd113/Vitrallis-Shell/main/devices/pocketchip/bootstrap.py -o "$t"; python3 "$t")
```

Requires Debian 12+ **armhf**, glibc 2.36+, SDL2 2.26.5+, Python 3.8+, HTTPS curl with CA certificates, PocketHome/Marshmallow, Awesome 4.x and a working systemd user session. The recorded device tests used Debian 13; the original Jessie image is unsupported. No Git, Rust compiler, build tools, or sudo are needed on the device. Missing runtime dependencies produce an error; the installer does not install system packages.

The bootstrap includes published beta prereleases when choosing the newest compatible bundle. It verifies the bundle, checksums, and helpers from **one release**, then installs all four binaries under `~/.local/share/vitrallis/`. It adds a Marshmallow menu item and desktop shortcut, saves installation backups, and installs an offline uninstaller. Marshmallow stays the normal boot default. [Installation details, stable-only selection, and recovery](docs/devices/pocketchip.md).

## First launch and controls

Reload Marshmallow's Apps menu and choose **Vitrallis**, or run `~/.local/share/vitrallis/launch` from a terminal in the existing graphical session.

| Action | Control |
| --- | --- |
| Select an app | Arrow keys |
| Open or resume it | Enter, click, or tap |
| Change page | Page Up / Page Down or header arrows |
| Return from an app | Physical Home in the supervised PocketCHIP session |
| Return to the original home | Select the Marshmallow tile |
| Update the native build | System Settings → More → Check for Updates |

Running apps remain open when you return Home. Save and close them before stopping or removing the session. Native utility menus and dialogs have visible keyboard focus; see [Terminal, Notepad and Files controls](docs/native-apps.md). App Center has [its own navigation and package guide](docs/app-center.md).

## Compatibility and beta limits

The shell's PocketCHIP integration has [recorded Debian 13 hardware evidence](docs/history/device-validation.md), plus [beta2.6 installation and startup checks](docs/history/beta2.6-device-validation.md). Removal, installing a later native update, physical input and extended native-app behavior still need fresh PocketCHIP validation. Emulation and desktop previews do not establish it.

Linux release packaging targets x86-64 and ARMv7 with the ABI requirements above. macOS is a development host; Linux artifacts cannot install there. Other devices need a platform adapter and validation. A matching screen size alone is not support.

Apps run with your user's permissions: **Vitrallis is not an app sandbox**. Catalog availability and runtime dependencies belong to each publisher. Some packages are disabled by their publisher. Fonts do not provide full Unicode shaping; Bluetooth controls, a public Python SDK, and signed publisher packages are future work. See the [trust model](docs/security.md) and [design roadmap](docs/design.md).

## Documentation and development

Start with the [documentation index](docs/README.md), [PocketCHIP guide](docs/devices/pocketchip.md), or [app developer guide](docs/app-development.md).

Host development uses the pinned Rust 1.91.1 toolchain (minimum 1.91), SDL2 development libraries, and pkg-config:

```sh
cargo build --workspace --locked
cargo run --locked
sh scripts/validate.sh
```

The workspace build includes all native utilities. [Contributor guidance](CONTRIBUTING.md) covers setup, focused changes, validation, and pull requests. Please use the [bug and feature forms](https://github.com/csd113/Vitrallis-Shell/issues/new/choose) for feedback and the [security reporting policy](SECURITY.md) for security concerns.

**License:** the workspace is marked `LicenseRef-Proprietary` in [Cargo metadata](Cargo.toml). No open-source license grant is included in this repository. Ask the maintainer about reuse; dependency licenses remain their own.

## Uninstall

**Save your work first.** Removal stops only a verified Vitrallis-owned session, closes its apps, and restores temporary Home bindings. Run as the same normal user; no network connection is needed:

```sh
python3 "$HOME/.local/share/vitrallis/uninstall.py"
```

Add `--dry-run` to inspect first. Add `--purge` to also remove the default Vitrallis preferences and session logs; deletion requires typing `PURGE`. Purge still keeps third-party apps, saves, App Center transaction backups, installation backups, user documents, system packages, and Marshmallow. Custom XDG locations and edited or unrecognized files are preserved. Matching shortcuts and the exact optional startup block are removed without restoring entire configuration files.

A tiny update lock remains for safe concurrency. After successful removal the uninstaller itself is gone; running the line again reports a missing script and changes nothing. For interrupted or partial installs, retained paths, and the local recovery command, see [offline removal and recovery](docs/devices/pocketchip.md#offline-removal-and-recovery).
