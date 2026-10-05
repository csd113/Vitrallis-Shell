# Vitrallis Shell

**Small screens. Big possibilities.**

A compact Rust + SDL2 launcher for small Linux screens. Open a terminal, jot down a note, browse your files, and make a small screen feel useful again. Vitrallis runs inside your existing desktop session.

[![Validate](https://github.com/csd113/Vitrallis-Shell/actions/workflows/validate.yml/badge.svg)](https://github.com/csd113/Vitrallis-Shell/actions/workflows/validate.yml) [![Releases](https://img.shields.io/github/v/release/csd113/Vitrallis-Shell?include_prereleases&label=release)](https://github.com/csd113/Vitrallis-Shell/releases)

![Vitrallis at 480×272: Terminal, Notepad, Files and App Center, with Terminal selected](docs/images/shell-480x272.png)

*Current desktop build rendered at 480×272; hardware status is unavailable.*

- **Three native essentials:** Terminal with a real PTY, a text-editing Notepad, and Files for browsing and everyday file operations. All ship with the shell and work offline.
- **Room to explore:** App Center checks GitHub catalogs and installs selected manifest packages, with source trust, checksums, and local-edit protection.
- **Keys or touch:** visible selection and shared activation across native controls, with confirmations for destructive actions.
- **Optional system integration:** brightness, volume, status, Wi-Fi utility access, and a supervised session that restores the original Home binding on exit.
- **Whole-build updates:** System Settings updates the shell and all three native utilities together when a compatible newer release is available.
- **Compact Settings:** display and sound, clock format and time zone, wireless and Tor, background-app policy, storage, device controls, software updates and About. A verified retained build can be restored from Software Updates.

## Installation

**[Vitrallis Shell 1.0.3 certification prerelease](https://github.com/csd113/Vitrallis-Shell/releases/tag/v1.0.3)
is published and verified on PocketCHIP.** Clean public installation, repeat
installation, software and physical cold startup, native utilities, App Center,
keyboard, touch and audible-audio checks pass. The candidate is release ready
with the qualifications in the report below; full stable publication remains
separate from this certification prerelease. The release restores keyboard focus
after a background app exits and uses Rust 1.99 with refreshed dependencies and
Arti 2.7.0. See the
[readiness report](docs/release-readiness-2026-10-02.md) and
[certification record](docs/release-certification-2026-10-02.md#final-public-candidate).
Install the complete bundle containing the shell, Terminal, Notepad,
Files and the shared Arti executable.
See [device installation and recovery](docs/devices/pocketchip.md) for supported
OS/runtime requirements, the single copy-and-paste setup command and hardware
validation limits. Setup prepares missing Debian packages and installs a verified
complete bundle with all five executables. Run it as your normal desktop user;
sudo is used for package and platform preparation.

Run this as your normal user on a supported PocketCHIP (requires `curl` and
working HTTPS certificates):

<!-- pocketchip-install-command -->
```sh
(set -eu; PATH=/usr/sbin:/usr/bin:/sbin:/bin; export PATH; umask 077; vitrallis_setup=$(mktemp -d /tmp/vitrallis-entry.XXXXXXXX); trap 'rm -rf "$vitrallis_setup"' 0; trap 'exit 130' 1 2 15; curl -q -fSL --proto '=https' --proto-redir '=https' --max-redirs 5 --connect-timeout 10 --max-time 30 --max-filesize 262144 https://raw.githubusercontent.com/csd113/Vitrallis-Shell/main/integrations/pocketchip/bootstrap.sh -o "$vitrallis_setup/bootstrap.sh"; sh "$vitrallis_setup/bootstrap.sh")
```

Setup replaces PocketHome's launch command with Vitrallis at login. PocketHome
does not run behind the Shell. Uninstall restores its original launch command;
SSH and serial login remain available for recovery.

## First launch and controls

Reboot after setup to start Vitrallis in place of PocketHome. Vitrallis starts
automatically at subsequent desktop logins. For later launches within that
session, use `~/.local/share/vitrallis/launch` or the Home key.

| Action | Control |
| --- | --- |
| Select an app | Arrow keys |
| Open or resume it | Enter, click, or tap |
| Change page | Page Up / Page Down or header arrows |
| Return from an app | Home in a supervised session |
| Leave Vitrallis for Awesome | Select **Exit Vitrallis** in a supervised session |
| Update the native build | System Settings → Software Updates → Check for Updates |

Returning Home backgrounds an app. Apps stay open by default; Settings → Applications can set an automatic background timeout or exempt individual apps. Save and close them before stopping or removing the session. Native utility menus and dialogs have visible keyboard focus; see [Terminal, Notepad and Files controls](docs/native-apps.md). App Center has [its own navigation and package guide](docs/app-center.md).

Use **Manage [F10] → Add shortcut** (F2) to launch ordinary Linux programs or scripts without an
App Center package. See [desktop shortcuts](docs/desktop-shortcuts.md) for command
quoting, terminal mode, icons, editing, and removal rules.

## Compatibility and beta limits

Linux release packaging targets x86-64 and ARMv7, with glibc 2.36+ and SDL2
2.26.5+. macOS is a development host; Linux artifacts cannot install there.
The shell uses SDL hardware acceleration when available, with automatic software
fallback and a GLES2 compatibility floor. See [renderer selection and diagnostics](docs/shell.md#sdl-renderer-selection)
for overrides and GPU validation limits. Hardware support requires a matching adapter and validation. A matching screen
size alone is not support. See the [device guide](docs/devices/pocketchip.md) for
recorded evidence and checks still requiring hardware.

Apps run with your user's permissions: **Vitrallis is not an app sandbox**. Catalog availability and runtime dependencies belong to each publisher. Publisher-disabled packages appear as **Unavailable** with their compatibility note. Fonts do not provide full Unicode shaping; Bluetooth hardware qualification, a public Python SDK, and signed publisher packages are future work. See the [trust model](docs/security.md) and [design roadmap](docs/design.md).

## Documentation and development

Start with the [documentation index](docs/README.md), [device guide](docs/devices/pocketchip.md), or [app developer guide](docs/app-development.md).

Host development requires the pinned Rust 1.99.0 toolchain, SDL2 development libraries, and pkg-config. CI also validates latest stable; release pins advance after the complete validation gates pass, including ARMv7 cross-builds. See [compiler policy](docs/dependencies.md).

```sh
cargo build --workspace --locked
cargo run --locked
sh scripts/validate.sh
cargo build --workspace --release --locked
```

The workspace build includes all native utilities. The validation script runs formatting, strict Clippy, Rust/Python tests, release builds and SDL smoke checks; [validation guidance](docs/validation.md) covers visual checks and Linux simulators. [Contributor guidance](CONTRIBUTING.md) covers setup, focused changes, validation, and pull requests. Please use the [bug and feature forms](https://github.com/csd113/Vitrallis-Shell/issues/new/choose) for feedback and the [security reporting policy](SECURITY.md) for security concerns.

**License:** project-owned code and documentation use [MIT](LICENSE). Third-party terms and artwork provenance are listed in [third-party notices](THIRD_PARTY_NOTICES.md); MIT does not relicense those items.

## Uninstall

**Save your work first.** Removal stops only a verified Vitrallis-owned session, closes its apps, and restores temporary Home bindings. Run as the same normal user; no network connection is needed:

```sh
python3 "$HOME/.local/share/vitrallis/uninstall.py"
```

Add `--dry-run` to inspect first. Add `--purge` to also remove the default Vitrallis preferences and session logs; deletion requires typing `PURGE`. Purge still keeps `~/Documents/Vitrallis/Apps` and `AppData`, saves, App Center transaction backups, installation backups, user documents, system packages, and the original desktop. Custom XDG locations and edited or unrecognized files are preserved. Matching shortcuts are removed and the exact managed startup block restores its original PocketHome command, preserving surrounding configuration edits.

A tiny update lock remains for safe concurrency. After successful removal the uninstaller itself is gone; running the line again reports a missing script and changes nothing. For interrupted or partial installs, retained paths, and the local recovery command, see [offline removal and recovery](docs/devices/pocketchip.md#offline-removal-and-recovery).
