# Native application validation — 2026-09-12

The Shell workspace now ships Terminal, Notepad and Files as native Rust/SDL2
binaries, with shared UI/filesystem components and normal launcher supervision.
The workspace version remains **0.1.0-beta2.5**. Validation used the current branch
without a release, deployment or physical PocketCHIP connection.

## Implementation and limits

See [native applications](native-apps.md) for controls, architecture, bounded
buffers, file safety, and the Files → Notepad path. The native registry is
`vitrallis_native::APPLICATIONS`, integrated by `src/native.rs`; `AppSource`
keeps native utilities distinct from App Center and device applications.
Original SVG/PNG icon sources are in `assets/native/`. `vitrallis-native` supplies
SDL setup, compact text/buttons/dialogs, keyboard/touch targeting, geometry,
shared browser/picker, document I/O, filesystem operations and private IPC.

Terminal uses a real PTY, vt100 ANSI/VT state, bounded scrollback and one blocking
I/O worker. Notepad preserves UTF-8 and LF/CRLF bytes with a 1 MiB document guard,
selection/clipboard/find and safe saves. Files explicitly lists/sorts one bounded
directory and supports open, folder creation, rename, delete, copy, move and
properties. Move is exclusive and same-filesystem; delete protects nonempty
folders. Copies stage complete destinations and stream through 64 KiB buffers.
Undo/redo, recursive deletion and cross-filesystem move are not implemented.

The only newly locked registry crates are vt100 0.16.2, vte 0.15.0,
unicode-width 0.2.2 and arrayvec 0.7.8. Existing libc is direct for the documented
POSIX boundary; existing SHA-256 is reused for same-size save-conflict detection.
No new GUI/runtime framework or installed Python app is involved. The
[dependency review](dependencies.md) explains portable-pty evaluation and the
4 KiB guard around vte's escape-string accumulator.

## Measured resources

Local **Linux AArch64 / Debian 12 container on an Apple Silicon macOS host**, Rust
1.91.1 release/LTO/stripped binaries, SDL2 2.26.5, local Xvfb X11 server, 480×272.
These are development measurements, not PocketCHIP measurements. Five warm runs
measure SDL first-frame rendering plus process exit, including process/loader
cost. Terminal's preview does not create a PTY, so its startup column is not a
shell-ready latency measurement. The idle sample runs the actual applications,
including Terminal's `/bin/sh -i` PTY and each app's blocking session inbox.
RSS is the app process, excluding the child shell and X server. CPU is process
user+system tick delta over 20 seconds; 0.00% means below sampled resolution.

| App | Preview/exit median | Idle CPU | RSS | Linux release | ARMv7 release |
| --- | ---: | ---: | ---: | ---: | ---: |
| Terminal | 23.20 ms | 0.05% | 9.04 MiB | 516.7 KiB | 523.1 KiB |
| Notepad | 13.53 ms | 0.00% | 8.69 MiB | 516.7 KiB | 513.1 KiB |
| Files | 15.63 ms | 0.00% | 8.69 MiB | 580.7 KiB | 542.6 KiB |

The complete ARM bundle is **3,373,396 bytes** (about 3.22 MiB), including the
1,756,604-byte shell. The AArch64 bundle is 3,427,360 bytes. No size/time threshold
is placed in CI. Reproduce on a local Linux host as a normal user:

```sh
DISPLAY=:99 python3 scripts/measure-native.py --bin-dir target/release --driver x11
# A local X server must already be available; the harness starts no display server.
python3 scripts/measure-native.py --bin-dir target/release --driver dummy
```

Measurement caught SDL's optional accelerated window surface loading a large
Mesa stack even with a software Canvas: about 95 MiB RSS in this Xvfb environment.
Native UI initialization now sets `SDL_FRAMEBUFFER_ACCELERATION=0`, avoiding
that stack. X11 then blocks on native events. The dummy backend's own event wait
showed 0.75–1.15% CPU, despite no application repaint timer; its roughly 7.7–7.8 MiB
RSS and ~6 ms preview times are useful for smoke comparison, not device forecasts.

Bounded regression cases cover 16 MiB malformed terminal strings, 10,000 terminal
lines, a 512 KiB PTY flood through EOF, a maximum-size document/long line, 3,000
directory entries, and a 4 MiB streamed file copy. The listing caps 20,000 inspected
entries and 8 MiB of retained name/path buffers. No recursive scan or file watcher
runs to render a directory. Copy progress is coalesced to at most 10 Hz; the
copy worker exists only during the operation. Idle UI waits without an app timer.

## Validation

| Command / check | Result |
| --- | --- |
| `cargo fmt --all -- --check` | Passed |
| `cargo check --workspace --locked` | Passed |
| `cargo test --workspace --locked` | Passed |
| `cargo test --workspace --all-features --locked` | Passed: 183 tests; one pre-existing explicit online GitHub contract test ignored |
| `cargo clippy --workspace --all-targets --all-features --locked -- -D warnings -D clippy::all -D clippy::pedantic -D clippy::nursery -D clippy::cargo` | Passed on macOS and Linux |
| `cargo build --workspace --release --locked` | Passed; all four executables |
| `cargo +1.91.0 check --workspace --all-features --locked` | Passed declared MSRV |
| `sh scripts/validate.sh` | Passed formatting, workspace check, strict Clippy, Rust/Python tests, release inventory, SDL smokes, script syntax and diff checks |
| Python unittest discovery | Passed: 38 tests |
| Complete source archive | All members/assets included; extracted into a path with spaces and all four binaries rebuilt offline and version-probed |
| Debian 12 Linux AArch64 | Workspace check, strict Clippy, tests, release build and complete bundle packaging passed |
| `PKG_CONFIG_LIBDIR=<cached ARM sysroot>/pkgconfig sh scripts/build-pocketchip.sh` | All four ARMv7 release executables built; cached libraries only |
| Cortex-A8 QEMU / Debian SDL2 2.26.5 | All four ARM binaries version-probed; native apps rendered at 480×272; shell launcher frame with native icons rendered; complete release bundle packaged |
| Native SDL dummy smoke | Terminal/Notepad/Files passed at 480×272 and 800×480 |
| Visual review | Native 480×272 screenshots and launcher icons inspected; shared input/dialog tests cover 480×272, 800×480 and 1280×720 |

Shell tests retain App Center and device-discovery coverage, and add native IDs,
icons, missing binaries, safe open requests, actual child ownership/reaping,
private native focus, and exact Notepad argument delivery. Notepad exercises
Cancel-default New/Close, Save-before-close, Discard, editing and footer focus.
Filesystem tests cover UTF-8/CRLF, inode replacement/modes, same-size external
changes, failed saves, limits, sorting/hidden/parent navigation, collisions,
streamed copies, growing sources, rollback, permission errors, links and dispatch.

Installer/update tests verify the four-file inventory, corrupt/truncated/extra
payload rejection, versions, architecture, locks, unsafe companion files, pointer
escape, atomic current/previous switches, failure after pointer publication,
retained old generations, independent app sentinels and relaunch PID/arguments.
The device installer now probes all four matching versions with bounded output
and timeout before publication. Releases use one fixed-name `.vtrbundle` plus
whole-bundle SHA-256; each binary also has its own size/hash in the bundle.
No remote CI workflow run or GitHub release publication is claimed.

## Remaining physical PocketCHIP validation

- Execute the complete ARM bundle on the Debian 13 image and verify loader/SDL ABI.
- Check actual 480×272 readability, brightness and touch target accuracy.
- Exercise physical Ctrl/Alt/Fn shortcuts, text entry, navigation and Cancel defaults.
- Run interactive PTY commands, signals, resize, alternate-screen CLI applications,
  rapid output, EOF and shell/foreground-job shutdown.
- Measure real device startup, idle CPU/RSS and sustained output/copy load.
- Check large directories, storage permissions, interrupted saves/copies and
  complete-bundle update/recovery on the actual storage.
- Verify Home, native window focus/resume, Files → Notepad with unsaved edits,
  close/return and supervisor reaping in the Awesome/systemd session.

No physical PocketCHIP was contacted for this work. Host emulation and historical
shell hardware reports do not establish these new utilities' hardware behavior.

## Changed files

```text
.github/workflows/shell-release.yml
Cargo.lock
Cargo.toml
README.md
apps/files/Cargo.toml
apps/files/src/lib.rs
apps/files/src/main.rs
apps/notepad/Cargo.toml
apps/notepad/src/lib.rs
apps/notepad/src/main.rs
apps/terminal/Cargo.toml
apps/terminal/src/lib.rs
apps/terminal/src/main.rs
apps/terminal/src/model.rs
apps/terminal/src/pty.rs
assets/native/README.md
assets/native/files.png
assets/native/files.svg
assets/native/notepad.png
assets/native/notepad.svg
assets/native/terminal.png
assets/native/terminal.svg
crates/vitrallis-native/Cargo.toml
crates/vitrallis-native/src/browser.rs
crates/vitrallis-native/src/document.rs
crates/vitrallis-native/src/files.rs
crates/vitrallis-native/src/ipc.rs
crates/vitrallis-native/src/lib.rs
crates/vitrallis-native/src/tests.rs
crates/vitrallis-native/src/ui.rs
devices/pocketchip/install.py
devices/pocketchip/vitrallis-session.py
docs/app-center.md
docs/app-development.md
docs/dependencies.md
docs/devices/pocketchip.md
docs/devices/pocketchip/store.md
docs/native-apps.md
docs/native-validation.md
docs/release-candidate.md
docs/repository-layout.md
docs/shell-updates.md
scripts/measure-native.py
scripts/package-shell-release.py
scripts/package-source.py
scripts/validate.sh
src/app.rs
src/app_center/discovery.rs
src/discovery/mod.rs
src/discovery/pockethome.rs
src/lib.rs
src/native.rs
src/platform/generic.rs
src/platform/pocketchip.rs
src/platform/pocketchip/recovery.rs
src/platform/update.rs
src/platform/update/tests.rs
src/platform/update/unix.rs
src/process.rs
src/renderer.rs
src/ui.rs
src/updater/bundle.rs
src/updater/mod.rs
src/updater/release.rs
src/updater/tests.rs
tests/desktop.rs
tests/test_installer.py
tests/test_repository_layout.py
tests/test_session.py
tests/test_shell_release.py
```
