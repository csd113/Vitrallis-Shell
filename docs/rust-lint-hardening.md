# Rust 1.99 lint hardening

The requested strict policy is implemented in all five packages. Required
validation passes on Rust 1.99.0 on macOS ARM64 and Debian 13 Linux ARM64.
The initial 3,128 unique host findings are resolved; final uncapped strict
Clippy reports **zero diagnostics**. The validation record below also identifies
optional checks that could not run on these platforms.

## Baseline

The starting tree was clean. Rust 1.99.0 and SDL2 2.32.74 were available on the
macOS ARM64 host. The root package forbade unsafe code; other packages had no
Cargo lint policy. `scripts/validate.sh` enforced warnings and Clippy's `all`,
`pedantic`, `nursery`, and `cargo` groups. CI called that script on Rust 1.99.0
and stable. The hardening pass changed no versions or dependencies. The subsequent
owner-authorized 1.0.4 release changes only workspace package versions and their
lockfile entries; dependency versions remain unchanged.

The first strict run stopped in the native dependency with 785 emitted errors
across the library and test targets. A second run used the exact requested
strict flags with `--cap-lints warn` **for inventory only**, allowing every
host target to be checked. Deduplicating diagnostics by lint, primary file,
byte range, and message yielded **3,128 findings**:

| Category | Initial unique findings | Final findings |
| --- | ---: | ---: |
| Arithmetic | 782 | 0 |
| Numeric types and conversions | 575 | 0 |
| Unused/ignored results | 555 | 0 |
| Indexing and string slicing | 617 | 0 |
| Shadowing | 324 | 0 |
| Enum/struct matching | 180 | 0 |
| Lost error context | 89 | 0 |
| Expect/panic paths | 5 | 0 |
| Assertion diagnostics | 1 | 0 |
| Total | 3,128 | 0 |

These counts cover the host's active conditional compilation only. Linux FFI
and platform-specific paths need separate validation.

## Permanent policy

The policy is centralized in the root `Cargo.toml` workspace lint tables and
inherited by all five packages. CI and local development use the same tables.
The shell's existing unsafe prohibition is retained on its crate roots.

Rust: `warnings`, `future_incompatible`, `unused_results`, and
`unsafe_op_in_unsafe_fn` are denied.

Clippy: `all`, `pedantic`, `nursery`, `cargo`, `unwrap_used`, `expect_used`,
`panic`, `todo`, `unimplemented`, `indexing_slicing`, `string_slice`,
`default_numeric_fallback`, `lossy_float_literal`, `as_conversions`,
`arithmetic_side_effects`, `undocumented_unsafe_blocks`,
`multiple_unsafe_ops_per_block`, `allow_attributes_without_reason`,
`let_underscore_must_use`, `map_err_ignore`, `rest_pattern_accessible_field`,
`wildcard_enum_match_arm`, `shadow_unrelated`, `shadow_reuse`, `shadow_same`,
`missing_assert_message`, `large_stack_arrays`, `large_stack_frames`, and
`large_enum_variant` are denied. No complete restriction group or excluded
UI arithmetic lint is enabled.

## Validation record

The initial host inventory's 3,128 findings are resolved. The exact requested
strict Clippy command passes on macOS, and the inherited identical policy passes
on Linux. Warning caps were used only during diagnostic collection, never for
these gates. Formatting, all-target checking, workspace tests, Python tests,
release builds, native rendering/readback and normal smoke checks pass on macOS.
The Linux workspace suite passes, including native package install/launch,
process identity/termination, update and uninstall. Real simulator checks pass
for App Center lifecycle, shortcuts, Settings and the stock Awesome session.

macOS workspace tests: shell 321, desktop integration 7, native 44, Files 2,
Notepad 3 and Terminal 12; nine opt-in tests are ignored by the ordinary run.
Linux: shell 324, desktop integration 7, native 44, Files 2, Notepad 3 and
Terminal 12; twelve opt-in/platform tests are ignored by the ordinary run.
Python: 181 tests are discovered on each host; macOS has 172 passes and nine
skips, and Linux has 173 passes and eight skips. Opt-in pixel references,
accelerated CLI readback and idle/workload checks are exercised separately.
Physical/device-mutating fixtures are excluded from this host audit.

Corrections implemented include:

- Normalize invalid UTF-8 document cursors before editing; validate replacement
  lengths before mutation and retry interrupted digest reads.
- Create copy staging directories privately from the outset; preserve the
  original copy or transaction failure when cleanup or recovery also fails.
- Harden bounded state reads against symlinks, special files, hard links, and
  pathname/file-identity changes; preserve the original failure on cleanup.
- Check bundle, download, recovery, removal, byte-count, and buffer-size
  arithmetic before writes. Reject duplicate download entries and incomplete or
  oversized payloads. Preserve App Center data and receipt ownership rules.
- Retain child ownership on helper setup failures; log kill, wait, channel,
  socket, permission, and cleanup failures rather than silently dropping them.
- Reserve owned child PIDs with a non-reaping wait before process-group cleanup,
  then reap. This avoids signalling a reused PID after `try_wait` consumed the
  original child. Terminal group cleanup uses the same ownership discipline.
- Acquire a Linux process handle before validating an external App Center
  process's script/executable and start identity, then signal that handle.
  `/proc` reads are bounded and process identity is checked before and after
  inspection. Permission failures remain visible. Linux process handles avoid
  the numeric-PID race, as described by
  [pidfd_send_signal](https://man7.org/linux/man-pages/man2/pidfd_send_signal.2.html).
  [pidfd_open](https://man7.org/linux/man-pages/man2/pidfd_open.2.html) requires
  Linux 5.3 or newer. If the capability is unavailable, or on a host without an
  equivalent process handle, external apps must be closed manually; no unsafe
  numeric-PID fallback is used. Shell-owned application closing is unchanged.
- Recognize the `--check-hash-based-pycs always` arguments emitted by installed
  Python launchers. Previously, that option hid a live app from update detection,
  allowing replacement without the running-app confirmation. Validate the option's
  [three supported values](https://docs.python.org/3/using/cmdline.html#cmdoption-check-hash-based-pycs)
  and fail closed on unrecognized options referencing the exact target script.
  Actual-process tests cover launcher arguments, stale identity, exact script
  matching and ambiguous options on both hosts. The real update simulation verifies
  Cancel preserves the process and Close and Update stops it before replacement.
- Preserve final Tor status transitions when the bounded queue is full, avoid
  joining a reader blocked on that queue, and retain a child if bounded cleanup
  cannot confirm it was reaped. Failed cleanup no longer claims Tor stopped.
- Clamp stale launcher selections before activation and validate App Center
  row targets before changing selection. Keep keyboard/touch action routing and
  safe confirmation defaults.
- Reject an unavailable repository edit target instead of silently adding a
  replacement source. Make missing required JSON fields explicit errors.
- Validate decoded PNG lengths/channels and BMP header ranges. Publish shell
  and native screenshots through one bounded temporary-file writer and an
  exclusive atomic rename, preserving existing destinations on failure.
- Split Linux EGL unsafe operations into minimal documented blocks and retain
  graphics-library lifetimes through every FFI call.
- Keep worker completion/cancellation explicit and report unexpected panics or
  poisoned state. Bounded rendering/input arithmetic clips invalid geometry;
  it does not wrap into buffer offsets or valid selections.
- Share owned-child group termination across launchers, system helpers, App
  Center runtime probes and Terminal. Reserve the leader with `waitid(WNOWAIT)`,
  signal before reaping, and remove the external `/bin/kill` helper.
- Retain runtime-helper ownership until output draining completes, preventing
  reused-PID cleanup after a premature `try_wait`. Bound output and deadlines;
  do not block the UI thread on helper/network work.
- Check artwork allocation sizes and retained-texture budgets; open icon files
  nonblocking and validate the opened descriptor, preventing a FIFO replacement
  from blocking rendering. Keep icon symlink behavior and existing caches.
- Replace index-based footer navigation with iterator lookup, remove its temporary
  vectors, clamp stale selections and validate navigation arithmetic.
- Keep fixture access fallible and assert input, persistence and channel outcomes.
  Distinguish initial, validated, upgraded and refreshed fixture values by name.
  Accept an already-selected SDL dummy driver when a redundant hint is rejected.
- Give the simulator HTTPS fixture an explicit Content-Length. On Debian 13's
  curl 8.14/OpenSSL 3.5, an HTTP/1.0 body delimited only by an unclean TLS close
  fails with curl 56. Production TLS checks and error handling remain strict.
- Compare Settings page-title pixels without the live clock/status row in the
  navigation fixture. A minute rollover changed only pixels `(460,24)..(466,29)`
  and falsely failed the previous full-header comparison. Separate clock-format
  persistence and restart assertions remain intact.

## Unsafe and suppression audit

The shell crate roots and build script retain `forbid(unsafe_code)`. Minimal
FFI stays in the native crate and Terminal PTY. Every unsafe operation has a
local ownership, pointer, ABI or lifetime explanation. EGL symbol lookup,
transmutation, context queries and copied strings now have separate scopes;
the loaded library remains alive throughout. New process-handle operations use
existing libc and are justified by PID-reuse correctness, not performance.

The obsolete shell glyph truncation allowance was removed by eliminating its
cast. The native touch truncation allowance remains local and now explicitly
covers the requested `as_conversions` lint. No expect attributes, panic/unwrap
calls, blanket test allowances, group-level exemptions or ignored-error
allowances remain. All current exceptions are enumerated below; their reasons
are the source's exact range/API proof. Apart from the retained native touch
attribute, every listed attribute is a new local exception; the touch attribute
adds only `as_conversions` to its existing truncation rule.

| Location | Lints | Justification |
| --- | --- | --- |
| [crates/vitrallis-native/src/font.rs:41](../crates/vitrallis-native/src/font.rs) | `clippy::arithmetic_side_effects` | The immutable 352-glyph atlas is 128x176 RGBA; slots, rows, and columns are bounded by these constants, and offsets fit usize on every supported host |
| [crates/vitrallis-native/src/keyboard.rs:36](../crates/vitrallis-native/src/keyboard.rs) | `clippy::rest_pattern_accessible_field`, `clippy::wildcard_enum_match_arm` | The keyboard translator only modifies key events and focus loss; other SDL events and their unrelated metadata pass through unchanged |
| [crates/vitrallis-native/src/renderer.rs:52](../crates/vitrallis-native/src/renderer.rs) | `clippy::as_conversions` | SDL's repr(u32) flag discriminant is the renderer's u32 bit mask |
| [crates/vitrallis-native/src/renderer.rs:57](../crates/vitrallis-native/src/renderer.rs) | `clippy::as_conversions` | SDL's repr(u32) flag discriminant is the renderer's u32 bit mask |
| [crates/vitrallis-native/src/renderer.rs:62](../crates/vitrallis-native/src/renderer.rs) | `clippy::as_conversions` | SDL's repr(u32) flag discriminant is the renderer's u32 bit mask |
| [crates/vitrallis-native/src/theme.rs:64](../crates/vitrallis-native/src/theme.rs) | `clippy::arithmetic_side_effects` | Every channel and amount is u8: the signed interpolation intermediate is bounded by 65025 and the result remains in 0..=255 |
| [crates/vitrallis-native/src/ui.rs:431](../crates/vitrallis-native/src/ui.rs) | `clippy::rest_pattern_accessible_field`, `clippy::wildcard_enum_match_arm` | SDL dispatch ignores timestamps and device fields not used by this UI, and events outside its supported input/window set |
| [crates/vitrallis-native/src/ui.rs:516](../crates/vitrallis-native/src/ui.rs) | `clippy::cast_possible_truncation`, `clippy::as_conversions` | Normalized finite touch positions are clamped to a display bounded by SDL |
| [crates/vitrallis-native/src/ui.rs:803](../crates/vitrallis-native/src/ui.rs) | `clippy::as_conversions` | SDL's repr(u32) event discriminant is the API's required user-event type code |
| [src/app_center/screen.rs:1307](../src/app_center/screen.rs) | `clippy::wildcard_enum_match_arm` | This SDL handler consumes selected keyboard, pointer or window events; unrelated controller, audio, drop and platform events intentionally have no action |
| [src/boot.rs:141](../src/boot.rs) | `clippy::wildcard_enum_match_arm` | This SDL handler consumes selected keyboard, pointer or window events; unrelated controller, audio, drop and platform events intentionally have no action |
| [src/input.rs:16](../src/input.rs) | `clippy::wildcard_enum_match_arm`, `clippy::rest_pattern_accessible_field` | SDL metadata is intentionally ignored. This SDL handler consumes selected keyboard, pointer or window events; unrelated controller, audio, drop and platform events intentionally have no action |
| [src/input.rs:152](../src/input.rs) | `clippy::wildcard_enum_match_arm`, `clippy::rest_pattern_accessible_field` | SDL metadata is intentionally ignored. This SDL handler consumes selected keyboard, pointer or window events; unrelated controller, audio, drop and platform events intentionally have no action |
| [src/input.rs:327](../src/input.rs) | `clippy::wildcard_enum_match_arm`, `clippy::rest_pattern_accessible_field` | SDL metadata is intentionally ignored. This SDL handler consumes selected keyboard, pointer or window events; unrelated controller, audio, drop and platform events intentionally have no action |
| [src/layout.rs:42](../src/layout.rs) | `clippy::arithmetic_side_effects` | Dimensions are validated as 320..=4096 by 200..=4096 with at most 6 columns and 4 rows before arithmetic; all i32 coordinates and products remain below 32768 |
| [src/renderer/system.rs:36](../src/renderer/system.rs) | `clippy::arithmetic_side_effects` | Fallback icon dimensions are checked as 1..=4096 before multiplication and division; normalized coordinates stay in -16..=15 and translated positions saturate |
| [src/renderer/system.rs:77](../src/renderer/system.rs) | `clippy::arithmetic_side_effects` | The input guard bounds x and y to -16_i32..=16_i32; squared distances, absolute values and fixed offsets stay below 1024 |
| [src/renderer/system.rs:112](../src/renderer/system.rs) | `clippy::arithmetic_side_effects` | The guard bounds the sample to -16_i32..=16_i32 and the six fixed bolt vertices share that range; cross products stay below 2048 and winding within -6..=6 |
| [src/renderer/system.rs:146](../src/renderer/system.rs) | `clippy::arithmetic_side_effects` | Radius is checked as 0..=128, so squared sums stay at most 32768 and negation is safe; translated coordinates use saturation |
| [src/settings/pointer.rs:16](../src/settings/pointer.rs) | `clippy::wildcard_enum_match_arm` | This SDL handler consumes selected keyboard, pointer or window events; unrelated controller, audio, drop and platform events intentionally have no action |
| [src/shortcuts/screen.rs:854](../src/shortcuts/screen.rs) | `clippy::wildcard_enum_match_arm`, `clippy::rest_pattern_accessible_field` | SDL metadata is intentionally ignored. This SDL handler consumes selected keyboard, pointer or window events; unrelated controller, audio, drop and platform events intentionally have no action |
| [src/storage/disk.rs:13](../src/storage/disk.rs) | `clippy::arithmetic_side_effects` | A u64 byte count times 100 fits in u128, and total.max(1) makes the divisor nonzero |
| [src/storage/disk.rs:85](../src/storage/disk.rs) | `clippy::arithmetic_side_effects` | Widening u64 to u128 leaves room for multiplication by ten and rounding; the selected divisor is one of 1000, 1000000 or 1000000000 |
| [src/storage/disk.rs:117](../src/storage/disk.rs) | `clippy::arithmetic_side_effects` | Both u64 byte counts are widened to u128 before multiplying by at most 255, so neither product can overflow |
| [src/ui.rs:1020](../src/ui.rs) | `clippy::wildcard_enum_match_arm` | This SDL handler consumes selected keyboard, pointer or window events; unrelated controller, audio, drop and platform events intentionally have no action |

## Commands and functional coverage

Completed command groups (uncapped):

- `cargo fmt --all --check`
- `cargo check --locked --workspace --all-targets --all-features`
- `cargo clippy --locked --workspace --all-targets --all-features` and the exact
  objective's explicit `-D` list on macOS; identical inherited policy on Linux.
- `cargo test --locked --workspace --all-features` on macOS and Linux.
- `python3 -m unittest discover -s tests -p 'test_*.py'` on macOS and Linux.
- `cargo build --locked --release --workspace --all-features` on both hosts.
- `VITRALLIS_RENDERER_BIN_DIR=target/release python3 -m unittest discover -s tests
  -p 'test_native_renderer.py'` on both hosts; one dummy-policy test passes and
  one accelerated test is skipped by default. With `VITRALLIS_TEST_ACCELERATED=1`
  on macOS, both tests pass, including exact native software/hardware/auto pixels
  for Terminal, Notepad and Files at 480x272, 800x480 and 1280x720.
- `SDL_VIDEODRIVER=dummy cargo run --locked -- --smoke-test`.
- Release `--version` probes and Terminal/Notepad/Files `--smoke-test` at
  480x272 and 800x480.
- `sh -n` for all shell scripts; `python3 -m compileall -q scripts integrations`;
  `python3 scripts/check-doc-links.py`; `git diff --check`.
- `sh scripts/validate.sh`: the complete macOS repository gate passed.
- `cargo test --locked -p vitrallis-shell --lib idle_loop_stops_after_startup
  -- --ignored --nocapture`: passes on macOS.
- `cargo test --locked -p vitrallis-shell --lib rendering_workloads
  -- --ignored --nocapture`: passes on macOS.
- `cargo test --locked --test desktop accelerated_readback_and_presentation
  -- --ignored --nocapture`: passes on macOS with CLI hardware readback.
- `cargo test --locked --test desktop mesa_software_is_not_hardware
  -- --ignored --nocapture`: passes under Linux Xvfb/Mesa; software Mesa is
  correctly rejected by hardware mode.
- Linux simulator: `dbus-run-session -- python3 tests/simulator/session.py`,
  `python3 tests/simulator/lifecycle.py`, `python3 tests/simulator/shortcuts.py`
  and `python3 tests/simulator/settings.py`: all pass.
- `python3 scripts/measure-native.py --bin-dir /target/release --driver dummy
  --seconds 5 --samples 2`: passes as an unprivileged Linux container user.
- `python3 -m py_compile tests/simulator/settings.py tests/simulator/repositories.py`:
  passes after the fixture corrections.

Tests exercise malformed/missing metadata, unavailable icons, UTF-8 edits,
copy failure cleanup, IPC, bounded downloads, invalid inventories/packages,
source trust changes, cancelled/interrupted transactions, recovery, restore,
process output timeouts, running-state identity, relaunch, keyboard/touch
selection and safe destructive confirmation defaults. System-panel screenshots
match the checked-in exact macOS reference hashes at 320x200, 480x272, 800x480
and 1280x720.

The real 480x272 simulator passes 20 App Center scenarios, ten shortcut scenarios,
three Settings scenarios and the stock session check. App Center exercises sequential
installs, actual old/new code execution, changed entry points, removal of obsolete
files, corrupt updates, running-app confirmation, safe removal, invalid metadata,
unavailable sources, cached restart and device-menu refresh failure. Shortcuts exercise
keyboard and native SDL touch input, structured command arguments, icon ownership,
save/restart, safe cancellation, removal and receipt identity revalidation. Settings
exercises saved clock formats, all eight categories and keyboard Back from Tor.
The Awesome session covers native crash recovery, launch/Home/resume/close, focus and
key restoration, preservation of concurrent keybindings and missing-utility repair.
Recovery, folder persistence, low-space rejection, permission errors, unavailable
networking and invalid package boundaries also have isolated Rust/Python coverage.

The idle test renders three startup frames in each two-second observation and then
stops redrawing, both with and without the Tor page open. The 200-frame rendering
workloads have zero fresh decodes/uploads after preparation; repeated cached App Center
icon draws also upload nothing. The Linux dummy-driver probe measures native release
binaries at 594,760 bytes each, warm first-frame-and-exit medians of 6.22–9.50 ms,
7.51–7.78 MiB RSS and 0.6% idle CPU over five seconds. These two-sample VM measurements
are a host sanity check, not a PocketCHIP benchmark or a before/after speed comparison.
No new per-frame filesystem/network probes or background polling were introduced.

Gate logs, simulator screenshots and scenario JSON are saved under the ignored
`target/rust-lint-audit/` directory, including `simulator-final/`.

## Remaining platform limits

No physical PocketCHIP, Mali/Lima GPU, battery/backlight/audio/radio controls,
physical suspend/resume or systemd user-manager operation was exercised.
Software Mesa in Xvfb is a fallback/API-path check, not genuine GPU validation.
Physical QA fixtures intentionally mutate an explicitly configured isolated
home and were not run. No timing or memory number from this host is a PocketCHIP
performance claim. ARMv7 cross-device benchmarks remain hardware work.

The optional online `published_catalog_packages_match_the_current_contract` test
was attempted against actual GitHub on both hosts. It cannot validate the whole
published catalog because app 5 has no compatible Rust binary for macOS or
`aarch64-unknown-linux-gnu`. This external catalog/platform limitation is recorded;
native compatibility checks were not weakened. Deterministic Python and native
package lifecycle tests pass. The separate published Python dependency/device QA
fixtures were not run.

Two optional in-process macOS Rust GPU tests (`hardware_boot_matches_software` and
`hardware_scenes_match_software`) returned `No available video device`; the atlas
test sharing that in-process backend was not attempted afterward. Process-isolated
shell and native accelerated CLI tests pass on this host. Physical Mali/Lima boot,
atlas and scene equivalence remain unverified.

Intentional behavior changes are fail-closed/error-path corrections: reject
invalid/stale UI targets and unsafe files, retain real cleanup/transaction errors,
and refuse unsafe process termination. External App Center process closing needs
Linux 5.3+ process handles; an unavailable handle requires manual close. Normal
shell-owned app lifecycle, screen geometry/style, app APIs and Documents paths
are preserved. The hardening pass made no version, lockfile, dependency or commit
changes. Its subsequent authorized 1.0.4 release bumps the workspace version and
lockfile package entries and commits the reviewed changes; dependencies stay fixed.

## Files changed

- [Cargo.lock](../Cargo.lock)
- [Cargo.toml](../Cargo.toml)
- [README.md](../README.md)
- [apps/files/Cargo.toml](../apps/files/Cargo.toml)
- [apps/files/src/lib.rs](../apps/files/src/lib.rs)
- [apps/notepad/Cargo.toml](../apps/notepad/Cargo.toml)
- [apps/notepad/src/lib.rs](../apps/notepad/src/lib.rs)
- [apps/terminal/Cargo.toml](../apps/terminal/Cargo.toml)
- [apps/terminal/src/command.rs](../apps/terminal/src/command.rs)
- [apps/terminal/src/lib.rs](../apps/terminal/src/lib.rs)
- [apps/terminal/src/model.rs](../apps/terminal/src/model.rs)
- [apps/terminal/src/pty.rs](../apps/terminal/src/pty.rs)
- [build.rs](../build.rs)
- [crates/vitrallis-native/Cargo.toml](../crates/vitrallis-native/Cargo.toml)
- [crates/vitrallis-native/src/browser.rs](../crates/vitrallis-native/src/browser.rs)
- [crates/vitrallis-native/src/document.rs](../crates/vitrallis-native/src/document.rs)
- [crates/vitrallis-native/src/files.rs](../crates/vitrallis-native/src/files.rs)
- [crates/vitrallis-native/src/font.rs](../crates/vitrallis-native/src/font.rs)
- [crates/vitrallis-native/src/ipc.rs](../crates/vitrallis-native/src/ipc.rs)
- [crates/vitrallis-native/src/keyboard.rs](../crates/vitrallis-native/src/keyboard.rs)
- [crates/vitrallis-native/src/keyboard/tests.rs](../crates/vitrallis-native/src/keyboard/tests.rs)
- [crates/vitrallis-native/src/lib.rs](../crates/vitrallis-native/src/lib.rs)
- [crates/vitrallis-native/src/paths.rs](../crates/vitrallis-native/src/paths.rs)
- [crates/vitrallis-native/src/process.rs](../crates/vitrallis-native/src/process.rs)
- [crates/vitrallis-native/src/renderer.rs](../crates/vitrallis-native/src/renderer.rs)
- [crates/vitrallis-native/src/renderer/egl.rs](../crates/vitrallis-native/src/renderer/egl.rs)
- [crates/vitrallis-native/src/renderer/graphics.rs](../crates/vitrallis-native/src/renderer/graphics.rs)
- [crates/vitrallis-native/src/renderer/tests.rs](../crates/vitrallis-native/src/renderer/tests.rs)
- [crates/vitrallis-native/src/tests.rs](../crates/vitrallis-native/src/tests.rs)
- [crates/vitrallis-native/src/theme.rs](../crates/vitrallis-native/src/theme.rs)
- [crates/vitrallis-native/src/ui.rs](../crates/vitrallis-native/src/ui.rs)
- [docs/releases.md](../docs/releases.md)
- [docs/rust-lint-hardening.md](../docs/rust-lint-hardening.md)
- [scripts/validate.sh](../scripts/validate.sh)
- [src/app.rs](../src/app.rs)
- [src/app_center/accounting.rs](../src/app_center/accounting.rs)
- [src/app_center/cache.rs](../src/app_center/cache.rs)
- [src/app_center/discovery.rs](../src/app_center/discovery.rs)
- [src/app_center/install.rs](../src/app_center/install.rs)
- [src/app_center/lifecycle_tests.rs](../src/app_center/lifecycle_tests.rs)
- [src/app_center/metadata.rs](../src/app_center/metadata.rs)
- [src/app_center/mod.rs](../src/app_center/mod.rs)
- [src/app_center/native.rs](../src/app_center/native.rs)
- [src/app_center/native_tests.rs](../src/app_center/native_tests.rs)
- [src/app_center/network.rs](../src/app_center/network.rs)
- [src/app_center/running.rs](../src/app_center/running.rs)
- [src/app_center/runtime.rs](../src/app_center/runtime.rs)
- [src/app_center/runtime_cleanup.rs](../src/app_center/runtime_cleanup.rs)
- [src/app_center/screen.rs](../src/app_center/screen.rs)
- [src/app_center/sources.rs](../src/app_center/sources.rs)
- [src/app_center/storage.rs](../src/app_center/storage.rs)
- [src/app_center/tests.rs](../src/app_center/tests.rs)
- [src/app_center/transaction.rs](../src/app_center/transaction.rs)
- [src/app_center/uninstall.rs](../src/app_center/uninstall.rs)
- [src/boot.rs](../src/boot.rs)
- [src/config.rs](../src/config.rs)
- [src/discovery/catalog.rs](../src/discovery/catalog.rs)
- [src/discovery/mod.rs](../src/discovery/mod.rs)
- [src/discovery/pockethome.rs](../src/discovery/pockethome.rs)
- [src/folders.rs](../src/folders.rs)
- [src/input.rs](../src/input.rs)
- [src/launcher.rs](../src/launcher.rs)
- [src/layout.rs](../src/layout.rs)
- [src/lib.rs](../src/lib.rs)
- [src/main.rs](../src/main.rs)
- [src/native.rs](../src/native.rs)
- [src/navigation.rs](../src/navigation.rs)
- [src/platform/command.rs](../src/platform/command.rs)
- [src/platform/linux_handheld.rs](../src/platform/linux_handheld.rs)
- [src/platform/linux_handheld/display.rs](../src/platform/linux_handheld/display.rs)
- [src/platform/linux_handheld/gpu.rs](../src/platform/linux_handheld/gpu.rs)
- [src/platform/linux_handheld/radio.rs](../src/platform/linux_handheld/radio.rs)
- [src/platform/system.rs](../src/platform/system.rs)
- [src/platform/update.rs](../src/platform/update.rs)
- [src/platform/update/tests.rs](../src/platform/update/tests.rs)
- [src/platform/update/unix.rs](../src/platform/update/unix.rs)
- [src/preferences.rs](../src/preferences.rs)
- [src/process.rs](../src/process.rs)
- [src/renderer.rs](../src/renderer.rs)
- [src/renderer/app_center.rs](../src/renderer/app_center.rs)
- [src/renderer/artwork.rs](../src/renderer/artwork.rs)
- [src/renderer/cache_tests.rs](../src/renderer/cache_tests.rs)
- [src/renderer/performance.rs](../src/renderer/performance.rs)
- [src/renderer/shortcuts.rs](../src/renderer/shortcuts.rs)
- [src/renderer/system.rs](../src/renderer/system.rs)
- [src/renderer/system_storage.rs](../src/renderer/system_storage.rs)
- [src/renderer/system_tor.rs](../src/renderer/system_tor.rs)
- [src/renderer/system_wireless.rs](../src/renderer/system_wireless.rs)
- [src/settings.rs](../src/settings.rs)
- [src/settings/device.rs](../src/settings/device.rs)
- [src/settings/footer.rs](../src/settings/footer.rs)
- [src/settings/geometry.rs](../src/settings/geometry.rs)
- [src/settings/keyboard_tests.rs](../src/settings/keyboard_tests.rs)
- [src/settings/pointer.rs](../src/settings/pointer.rs)
- [src/settings/preferences.rs](../src/settings/preferences.rs)
- [src/settings/storage.rs](../src/settings/storage.rs)
- [src/settings/tor.rs](../src/settings/tor.rs)
- [src/settings/update.rs](../src/settings/update.rs)
- [src/settings/wireless.rs](../src/settings/wireless.rs)
- [src/shortcuts/command.rs](../src/shortcuts/command.rs)
- [src/shortcuts/mod.rs](../src/shortcuts/mod.rs)
- [src/shortcuts/screen.rs](../src/shortcuts/screen.rs)
- [src/shortcuts/screen_tests.rs](../src/shortcuts/screen_tests.rs)
- [src/shortcuts/tests.rs](../src/shortcuts/tests.rs)
- [src/storage/disk.rs](../src/storage/disk.rs)
- [src/storage/mod.rs](../src/storage/mod.rs)
- [src/storage/scan.rs](../src/storage/scan.rs)
- [src/storage/tests.rs](../src/storage/tests.rs)
- [src/test_support.rs](../src/test_support.rs)
- [src/tor/mod.rs](../src/tor/mod.rs)
- [src/tor/tests.rs](../src/tor/tests.rs)
- [src/tor/worker.rs](../src/tor/worker.rs)
- [src/ui.rs](../src/ui.rs)
- [src/ui/relaunch.rs](../src/ui/relaunch.rs)
- [src/updater/bundle.rs](../src/updater/bundle.rs)
- [src/updater/mod.rs](../src/updater/mod.rs)
- [src/updater/release.rs](../src/updater/release.rs)
- [src/updater/tests.rs](../src/updater/tests.rs)
- [src/updater/transport.rs](../src/updater/transport.rs)
- [tests/desktop.rs](../tests/desktop.rs)
- [tests/simulator/repositories.py](../tests/simulator/repositories.py)
- [tests/simulator/settings.py](../tests/simulator/settings.py)
