# Vitrallis 0.1.0-beta2.1

This prerelease adds the native Rust App Center for PocketCHIP OTA testing.
Catalog manifests are fetched from GitHub at runtime. No applications, catalog
manifests, or application artwork are bundled; app files and assets are fetched
only when installing the associated app. Existing installed apps are preserved.

The App Center includes repository management, verified sequential installations,
local-change and publisher protections, recovery, and keyboard/touch navigation.
See [App Center behavior](app-center.md) and [changed files and validation](app-center-validation.md).

Version `0.1.0-beta2.1` and tag `v0.1.0-beta2.1` are explicitly authorized.
This version sorts above `0.1.0-beta.2` under semantic version precedence.
Release CI builds Linux x86-64 and ARMv7 binaries against glibc 2.36 / SDL2 2.26.5,
checks ARM startup and a 480×272 frame under Cortex-A8 QEMU, and supplies SHA-256
sidecars. Physical PocketCHIP OTA validation remains pending.

Use **More → Check for Updates** on the corrected beta.2 shell. The initially
published beta.2 updater excluded prereleases and needs the one-time replacement
described in [shell updates](shell-updates.md#one-time-transition-from-the-original-beta-updater).

---

# Vitrallis 0.1.0-beta.2

This release fixes keyboard navigation in the native system-service screens.
The user explicitly authorized this version, commit, push, and GitHub release.
`AGENTS.md` records that every future version change needs explicit permission.

- General Settings: Down still reaches More; Left selects Back, Right returns to
  More, and Enter activates the selected footer button. Up returns to the controls.
- More: Down after Check for Updates selects Back; Enter returns to General.
- Time zones: Down from the last visible entry selects the footer. Left/Right
  selects Previous, Back, or Next; Enter activates it. Left/Right on a zone and
  Page Up/Down still change pages, including partial final pages.
- Updates: the footer Back is selectable and cancels an install confirmation
  before leaving the page. Power and install confirmations still default to Cancel.
- Footer rendering, pointer hit testing, and key activation share the same visible
  targets. Blank footer areas no longer act as hidden navigation buttons.
- Update cleanup explicitly unlocks after removing staged files. A shared file
  descriptor retained across a helper fork can no longer keep a completed update
  locked until that helper executes. A regression reproduces the former failure
  and verifies that closing the old descriptor cannot unlock a new installation.
- The release container trusts only its exact mounted checkout path so Git's
  final diff validation can run across the runner/container ownership boundary.
- SDL key-event regression tests cover brightness, volume, Wi-Fi launch, restart,
  shutdown, timeout, time zones, calibration launch, update checks/installation,
  cancellation, and navigation while a hardware operation is pending. Native
  finger-event tests verify matching releases and footer parity at four sizes.

Validation on macOS ARM64 with Rust 1.91.1:

| Command | Result |
| --- | --- |
| `cargo fmt --all --check` | Passed after formatting touched Rust files |
| `cargo clippy --workspace --all-targets --all-features -- -D warnings -D clippy::all -D clippy::pedantic -D clippy::nursery -D clippy::cargo` | Passed; no lint suppressions |
| `cargo test --workspace --all-features settings::` | 15 Settings tests passed |
| `sh scripts/validate.sh` | Passed: strict Clippy, 93 unit + 6 integration tests, 37 Python tests, release build, SDL smoke, shell/Python syntax and diff checks |
| `PKG_CONFIG_LIBDIR="$PWD/target/arm-libs/pkgconfig" sh scripts/build-pocketchip.sh` | ARMv7 hard-float release cross-build passed with the existing image-matched SDL2 2.32.4 library |
| `VITRALLIS_QA_DIR="$PWD/target/beta2-qa" cargo test --workspace --all-features system_panels_render_at_device_and_scaled_sizes` | Rendered all pages and selected footer targets at 320×200, 480×272, 800×480, and 1280×720; focus screenshots inspected |

No new dependencies or hardware backend changes. This pass does not claim fresh
physical-device validation or a device installation. Calibration measures touch
coordinates, so its measurement still requires touch; keys launch or cancel it.
External utilities such as the Wi-Fi manager and App Center retain their own
input controls. The initial beta.2 publication excluded prereleases and used a
manual-only ARM asset name; the beta updater correction below supersedes those
limitations.

Changed files: `AGENTS.md`, `Cargo.toml`, `Cargo.lock`, `README.md`, this report,
`src/settings.rs`, `src/settings/{device,footer,geometry,keyboard_tests,pointer,update}.rs`,
`src/renderer.rs`, `src/renderer/system.rs`, and `src/platform/update/{unix,tests}.rs`.
Release workflow: `.github/workflows/shell-release.yml`.

## Beta updater correction

The version remains `0.1.0-beta.2`, as requested. Prerelease builds now select newer
published prereleases and stable releases using semantic precedence. Stable
builds retain stable-only selection; drafts, equal versions, downgrades, malformed
metadata, unverifiable downloads, and foreign artifacts remain excluded.

The release workflow now builds both x86-64 and ARMv7 against Debian 12's supported
ABI baseline. ARM uses the standard updater filename and receives an emulated
Cortex-A8 startup/version check and SDL frame render. The packager accepts an
explicit local emulator, invoked without shell interpretation.

Older installed beta.1/beta.2 updater binaries need a one-time replacement. The
refreshed updater can then receive future newer betas from the built-in Settings
page. No new Rust dependencies or version changes were introduced.

Correction files: `src/updater/{mod,release,tests}.rs`,
`scripts/package-shell-release.py`, `tests/test_shell_release.py`,
`.github/workflows/shell-release.yml`, `README.md`, `docs/shell-updates.md`, and
this report. `sh scripts/validate.sh` passed on macOS ARM64 / Rust 1.91.1:
formatting, strict workspace Clippy, 95 unit and 6 integration tests, 38 Python
tests, release build, SDL smoke, syntax checks, and `git diff --check`. Targeted
updater and release-packaging tests also passed. GitHub and device results are
recorded with the corrected release.

The following beta.1 records are historical and retain their original dates,
binary hashes, device results, and handoff status.

## Vitrallis 0.1.0-beta.1 candidate

The user has confirmed physical navigation/activation, cold startup into Marshmallow followed by Vitrallis launch, and return to Marshmallow. A second settings/loading follow-up addresses the remaining intermittent missing-window notice, slider jitter, and removal of F1. Its current validation is recorded at the end of this document.
This is a beta candidate, not production-ready. The previous manifest said 0.1.0,
but there are no release tags; the prerelease version identifies the first
explicit beta qualification rather than claiming a published stable release.

## Delivered scope

PocketHome/Marshmallow metadata discovery, proportional SDL2 rendering,
keyboard/touch controls, background/resume, Store catalogue refresh, typed
PocketCHIP status/settings, and a reversible user session with Marshmallow
fallback. Rust 1.91.1 is now pinned and actually tested, with shared workspace
metadata/dependencies, edition 2024 and resolver 3. The candidate includes build
and host-validation scripts plus a pinned read-only CI workflow.

See [dependency review](dependencies.md) for the attempted PNG API migration,
five exact upstream/policy exceptions, updated cross linker and security review.
See [security](security.md), [app development](app-development.md),
[Store](devices/pocketchip/store.md) and [installation/recovery](devices/pocketchip.md) for current interfaces.
No public Python SDK, `app.toml` loader, signed package format or generic
archive installer is claimed.

## Correctness and security changes

- Window resume requests run outside the UI thread, report errors and retain
  existing process ownership without duplicate launches. Activation checks for
  an exit before resuming, and reaping an unchanged background/returned app no
  longer resets an in-progress touch. Other children are still reaped if one
  child reports a wait error. Readiness is logged after the first rendered frame.
- Helper commands own process groups and clean descendants holding output pipes
  when deadlines expire.
- A dangling incomplete-install marker blocks launch/marks Store repair.
- Installer locking prevents two simultaneous launcher installs. Invalid JSON
  document/marker types fail with diagnostics. Symlink/hardlink mutation targets
  are rejected; atomic installer writes also sync the containing directory.
- Installation rechecks and rollback preserve detected concurrent user edits.
  Store deployment preserves private PocketHome file permissions and validates
  bundle names/labels before application writes.
- Log files use private, no-follow descriptor creation and regular/link checks.
- Nonregular catalogue files are rejected before opening; catalogue ID checks
  avoid quadratic scanning. ARM installer checks include ELF/program-header
  bounds, EABI5 and hard-float flags; file reads are bounded. Retained icon textures
  are limited to 16 MiB with placeholder fallback.

## Automated validation

Host: macOS ARM64, Python 3.13.5, Rust 1.91.1. Separate Rust 1.91.0 checks verify
the declared MSRV. No Rust lint suppressions were introduced.

| Command | Result |
| --- | --- |
| `cargo fmt --all --check` | Passed |
| `cargo clippy --workspace --all-targets --all-features -- -D warnings -D clippy::all -D clippy::pedantic -D clippy::nursery -D clippy::cargo` | Passed |
| `cargo test --workspace --all-features` | 52 unit + 5 integration tests passed; 0 failed/ignored; 0 doctests |
| `python3 -m unittest discover -s tests -p 'test_*.py'` | 21 passed on host and device |
| `python3 -m unittest discover -s target/reference/store -p 'test_*.py'` | 50 patched Store tests passed on host and device |
| `cargo build --locked --release --workspace --all-features` | Host passed |
| `sh scripts/build-pocketchip.sh` | ARMv7 hard-float release passed, Rust 1.91.1, Zig 0.16.0/cargo-zigbuild 0.23.4, glibc floor 2.36 |
| `RUSTUP_TOOLCHAIN=1.91.0 sh scripts/build-pocketchip.sh` | ARM MSRV build passed |
| `cargo +1.91.0 test --locked --workspace --all-features` | Same 52 unit + 5 integration tests passed |
| `SDL_VIDEODRIVER=dummy cargo run --locked -- --smoke-test` | Launch/reap/render recovery passed |
| `sh -n devices/pocketchip/run-pocketchip.sh scripts/build-pocketchip.sh` | Passed |
| `cargo audit` | No RustSec advisories/unmaintained warnings; 1,243 advisories loaded |
| `cargo tree --duplicates` | No duplicate crate versions |
| `cargo outdated --workspace` plus complete registry/lock comparison | Five documented exceptions; see dependencies.md |
| `git diff --check` and reverse application of the complete Store patch | Passed |

There was no existing Python lint/type-check configuration or dependency to
upgrade. Native tests cover multiple layouts, mock status/control failures,
malformed assets, FIFO catalogue refusal, bounded helpers, input filtering,
actual subprocess lifecycle, and 40 launch/exit cycles. New regressions cover
asynchronous focus, reactivation before exit polling, wait-error isolation,
bounded installer inputs, lock contention, hardlinks, malformed recovery state,
concurrent-edit preservation and failure after atomic rename. Remote GitHub
Actions execution is not claimed; the workflow will run after an authorized push.

## Hardware qualification

The user has now confirmed physical navigation and cold startup (see the latest follow-up below). Temporary access from the earlier pass was removed. The historical
[Step 3 evidence](compatibility-step3.md) is not counted as a fresh candidate test.
Five fresh injected launch/Home/resume/Escape cycles now preserve the app PID on
every resume. Bitcoin launch measured 6.882–7.368 s, resume 0.814–1.222 s, and
return 0.909–1.451 s including the test's 0.6 s settling delay. Initial runs exposed
and led to fixes for stale-child resume and touch cancellation on reaping.
The first harness also launched through Marshmallow before the initial frame;
those invalid timing/duplicate results are excluded from the final matrix.


Reboot via the serial console completed after a slow OS startup. A changed boot
ID confirmed the reboot; Marshmallow was the default. Starting Vitrallis after
boot, Return activation, Home, touch resume with the same Bitcoin PID, Escape,
and the fallback tile all passed. The fallback test waited for complete unit
cleanup and confirmed the saved Home binding was restored. Both Marshmallow
binaries and `~/.config/awesome/rc.lua` retained their original SHA-256 hashes.
The rendered status line showed 97% battery, charging/external power and Wi-Fi
connected, consistent with sysfs and NetworkManager readings.

## PocketCHIP performance samples

Debian 13, ARMv7, Linux 6.12.94, SDL2 2.32.4, Awesome 4.x, 480×272,
463,092 KiB kernel-reported RAM. Three supervised starts of the candidate were
observed through the active X window and a pixel in the fully rendered initial
selection. Idle CPU is the launcher's `/proc/PID/stat` tick delta over each
20-second interval; it includes the status worker. RSS is the process resident
page count. No Store tests or other scripted device workload ran during these
final intervals. OS services and Marshmallow continued normally.

| Measurement | Samples/result |
| --- | --- |
| Startup through first visible selected tile | 8.158, 7.698, 7.716 s |
| Idle CPU, 20 s each | 0.60%, 0.60%, 0.45% of one CPU |
| Idle CPU including waited status helpers, separate 40 s interval | 2.899% total: 0.400% launcher (0.125% status thread included), 2.499% waited helpers |
| Resident memory | 37,416–37,424 KiB (about 36.5 MiB) |
| Highlight change, 18 alternating directions | 83–89 ms including the deliberate 80 ms key-down interval |
| Store catalogue check, 3 read-only requests | 4.455, 1.119, 1.085 s; Bitcoin v1.1.0 current, Store manual update retained |
| Bitcoin launch, 5 cycles | 6.882–7.368 s |
| Home/resume, 5 cycles | 0.814–1.222 s; PID unchanged for every resume |
| Escape/return, 5 cycles | 0.909–1.451 s including 0.6 s test settling time |

The pre-change build's three focus-based supervised starts took 8.870, 7.663 and
7.728 s, with 0.45% idle CPU and 37,296–37,368 KiB RSS. The final startup check is
stricter (it also requires the rendered tile), so this is an approximate
comparison. Memory increased by at most 128 KiB in these samples; CPU differs by
0–0.15 percentage points. No large startup/idle regression is evident, but these
short samples are not an endurance, thermal or battery-life benchmark. Early
contaminated/harness-debug runs are not used in this table.

Fresh Bitcoin installation through the actual Store UI completed. The launcher
refreshed the catalogue and launched the newly installed copy. The previous
application remains preserved at
`~/.local/share/pocket-bitcoin.before-beta1-validation`; the older Step 3 backup
also remains intact. No OS Python/Tk or Debian packages were installed/upgraded.


Launcher and Python supervisor SIGKILL tests both removed the owned launcher
and Bitcoin descendants, returned focus to Marshmallow and restored the saved
Home binding. The installed launcher/session files were upgraded through the
reviewed installer; backups remain available. Marshmallow and its Awesome
startup configuration have not been replaced.


A refused local proxy produced a visible Store connection error; closing Store
returned cleanly to Vitrallis and reaped its child. Missing-command and nonzero
child-exit tests also preserved the launcher with useful diagnostics. Stopping
the session through the authenticated USB serial console restored Marshmallow
focus, an inactive user unit, and the original Home binding.

## Current limits and next milestone

This qualification covers the inspected Debian 13 image, not original Jessie
or another SBC. Real-world endurance, battery drain and thermal behavior remain
unmeasured. The 40-second status measurement includes waited helper processes;
the shorter idle samples above measure only the launcher and its threads.
Catalogue/icon filesystem work still occurs on the UI thread. A Store exit while
another app remains active delays catalogue refresh until a later app exit at
the ready grid. Arbitrary Tk apps without window PID metadata lack the reviewed
Bitcoin/Store identity fallback. SVG/JPEG, broader font coverage and Bluetooth
controls remain unsupported.

The next beta should establish a versioned application/package format, public
hardware and window-identity APIs, signed distribution, stronger transaction
recovery and endurance coverage before expanding to a second backend. These
are future work; this release does not ship an `app.toml` loader or Python SDK.
Production qualification also needs independent security review and broader
hardware/runtime coverage.

## Running and recovery

On this device, select **Vitrallis** from Marshmallow's Apps menu or run
`~/.local/share/vitrallis/launch` in the graphical user's terminal. Use this
supervised session for everyday use. Select **Marshmallow** on the next grid
page to exit, or run `systemctl --user stop vitrallis-session.service`.
Over USB serial, use
`XDG_RUNTIME_DIR="/run/user/$(id -u)" systemctl --user stop vitrallis-session.service`.
The exact optional five-second startup block and removal instructions are in
[PocketCHIP installation and recovery](devices/pocketchip.md#optional-reversible-default). It has not been installed;
Marshmallow remains the boot default.

## Changed files and source hygiene

- Toolchain/build: `Cargo.toml`, `Cargo.lock`, `rust-toolchain.toml`,
  `.github/workflows/validate.yml`, `.gitignore`, `scripts/build-pocketchip.sh`,
  `scripts/validate.sh`, `devices/pocketchip/run-pocketchip.sh`.
- Rust: `src/lib.rs`, `src/launcher.rs`, `src/process.rs`, `src/ui.rs`,
  `src/renderer.rs`, `src/preferences.rs`, `src/discovery/mod.rs`,
  `src/discovery/marshmallow.rs`, `src/platform/command.rs`,
  `src/platform/pocketchip.rs`, `src/platform/system.rs`.
- Deployment/Store: `devices/pocketchip/install.py`,
  `devices/pocketchip/apply-store-patch.py`, `devices/pocketchip/vitrallis-session.py`,
  `devices/pocketchip/integration/pocketchip-store.patch`, `devices/pocketchip/integration/store-patch-manifest.json`.
- Tests: `tests/desktop.rs`, `tests/test_installer.py`, `tests/test_session.py`,
  `tests/test_store_patch.py`; additional Store tests are included in its patch.
- Documentation: `README.md`, `docs/app-development.md`, `docs/dependencies.md`,
  `docs/security.md`, `docs/release-candidate.md`, `docs/devices/pocketchip.md`,
  `docs/devices/pocketchip/store.md`, and historical pointers in `docs/compatibility-step3.md`,
  `docs/devices/pocketchip/validation.md`, `docs/engineering-report.md`, `docs/validation.md`.

All build outputs, copied libraries, device logs, screenshots, local tools and
raw test harnesses are under ignored `target/`. No commit or push was made.
The worktree intentionally contains the candidate source changes above.

Temporary validation SSH authorization was removed by matching exactly the
newly added key, preserving all other authorized-key bytes and permissions.
A fresh public-key-only connection with multiplexing disabled was rejected.
The local temporary private/public key files were deleted; the USB serial
console was back at the unauthenticated login prompt. No persistent validation
access remains. Installed launcher/session hashes match the final local source
and ARM build, and the private SDL2 library hash matches the target library.


## Settings and immediate-reopen follow-up

This section records the first settings build. The later physical-feedback follow-up supersedes its F1 shortcut, slider behavior, and final binary hash.

User testing exposed a remaining process/window close race across applications.
A resume now remains pending while window lookup retries; if the former child
exits, the launcher reaps it before starting exactly one replacement. A missing
window alone never authorizes a second process. The wait is asynchronous and
bounded to ten seconds. Regression tests cover both a closing child and a window
that appears late without restarting its process. A second on-device failure exposed the old 400 ms return cooldown swallowing a fresh tap; focus return and active-child exit now end that cooldown, while unmatched releases remain rejected.

The generated Wi-Fi Settings tile is now System Settings, with its stable ID
preserved. It and F1 open the same native screen: brightness/volume sliders,
Wi-Fi access, Restart and Power Off. Sliders support drag/tap and arrow keys,
preview locally and submit once on release, then display hardware readback.
The configured Wi-Fi connection manager remains available from that screen.
Status text abbreviations were replaced by native battery, charging and Wi-Fi
icons; unsupported Bluetooth status is omitted. The PocketCHIP backend also adds the installed LXTerminal
`--no-remote` option so Terminal and Wi-Fi windows have separate process owners;
other terminal commands and generic backends are unchanged. No new dependency
or OS package was introduced. New source modules: `src/settings/geometry.rs`,
`src/settings/pointer.rs`, `src/renderer/system.rs`; `src/settings.rs`,
`src/platform/mod.rs`, `src/app.rs` and `docs/devices/pocketchip/system-status.md` are also changed.

The updated host gates pass: 52 unit + 5 integration Rust tests and 21 owned
Python tests; the ARM release build passes. Render QA covers 480×272, 800×480
and 1280×720, and slider input also covers 320×200. Earlier performance and
hardware figures above describe the preceding candidate. Follow-up device
qualification and temporary validation-access cleanup are complete below.


Follow-up test-harness correction: checking only the active window title could
send the initial tap through Marshmallow before Vitrallis had drawn its frame.
The final cycle harness waits for both focus and a rendered selection pixel,
then checks the launcher's owned child PID before/after resume and replacement.
The test-created Marshmallow Terminal was closed separately. Earlier title-only
cycle runs are diagnostic evidence, not the final ownership qualification.


Final follow-up lifecycle qualification: three Home/resume/close/immediate-reopen
cycles each for Bitcoin, Store, Terminal, Write and Browse Files passed (15 total).
Each resume retained the same launcher-owned PID; each replacement had a new
PID only after the previous process was absent. Terminal and the Wi-Fi manager
were then opened together: they had distinct launcher-owned processes, network
resume retained its PID, closing it returned to System Settings, and the original
Terminal still resumed with its own PID. These checks used the installed final
binary, SHA-256 `7f0346951ab868a209842be23d691bba100ad704e90c5fe0ede5b15ea6fe01bb`.

Brightness dragging changed the backlight from native level 7 to 4; keyboard
adjustment advanced it to 5. The original level 7 was restored. Device screenshots
confirmed the new settings tile, F1 screen, battery/Wi-Fi icons, power confirmation
with Cancel selected, and return from the existing network manager. At this
earlier stage, physical finger/key confirmation and a full cold start were still
pending; subsequent user results are recorded below.


Final volume readback changed from 75% to 30%, then restored to 75%. The final
Marshmallow fallback test waited for an inactive unit and confirmed the original
Home binding. The Marshmallow binaries and Awesome configuration still match
their original hashes. The installed launcher matches the final ARM artifact.

A separate 40.010 s idle sample of the updated build measured 0.425% launcher CPU
(0.100% status thread included), 2.449% waited helper CPU, 2.874% combined and
38,124 KiB RSS (about 37.2 MiB). One warm supervised start reached window focus in
6.703 s. This focus-only sample is not a speedup comparison against the earlier
three rendered-frame startup measurements. No other scripted device workload
ran during the idle interval. It is still a short sample, not endurance testing.

The follow-up temporary SSH key was removed, a fresh non-multiplexed public-key
connection was rejected, and the local private/public key files were deleted.
The serial validation session was logged out. Vitrallis was left running with
System Settings open for the user's physical check. No commits or pushes were
made; the current worktree intentionally contains the candidate changes.

At that handoff, physical qualification was deferred because the user was away.
The subsequent hands-on results below supersede that deferral; automated input
and a warm reboot were not counted as substitutes for those results.


## Physical feedback and loading/slider follow-up

The user confirmed physical directional controls, activation/touch, Wi-Fi access,
a full shutdown/power-on followed by launch from Marshmallow, and normal
Marshmallow fallback. They reported an occasional missing-window error during
rapid reopen and glitchy brightness dragging, and requested removal of F1.
The earlier absence-related physical-test deferral is therefore superseded by
this feedback; the reported defects required another implementation pass.

- A persistent Opening panel stays visible during startup/resume until focus
  transfers. Additional taps during that transition cannot create a duplicate.
- Device logs showed repeated missing-window results followed by successful
  focus for Bitcoin. Initial window creation is now tracked as well as resume;
  the asynchronous wait is 30 seconds. If the owned process is still alive but
  has no window, the UI returns to the grid with a retry hint instead of a modal
  launch error. It preserves ownership. A closed child is reaped before a
  replacement starts; an initial startup crash is not automatically restarted.
- F1 is unbound and its footer prompt removed. The System Settings tile,
  directional keypad activation, and footer remain available.
- Brightness native levels 1–10 map directly to 10–100%, preserving the lowest
  lit level. Volume uses 0–100%. Both controls use 10% detents; mixer readback is
  rounded to the nearest detent because ALSA can report a nearby quantized value.
- Dragging sends live changes while the contact is held. Pending updates retain
  only the newest requested value per slider. A small dead band filters touch
  jitter, drag redraws are limited to roughly 60 per second, and opening the settings tile no
  longer starts the app-launch input cooldown.
- A measured two-second drag delay exposed serialization behind a status probe.
  Status sampling and control writes now use independent bounded workers.
  Snapshot timestamps prevent an older read from overwriting a newer control
  result; control activity cannot conceal stale battery/network status.

Files changed for this follow-up: `src/input.rs`, `src/launcher.rs`,
`src/process.rs`, `src/ui.rs`, `src/settings.rs`, `src/settings/pointer.rs`,
`src/platform/system.rs`, `src/platform/pocketchip.rs`, `src/renderer.rs`,
`README.md`, `docs/devices/pocketchip.md`, `docs/devices/pocketchip/system-status.md`, and this report.

Final source validation passed with Rust 1.91.1 and the declared 1.91.0 MSRV:
`sh scripts/validate.sh` (formatting, strict workspace/all-target/all-feature
Clippy, **59 unit + 5 integration tests**, **21 Python tests**, release build,
SDL smoke, shell syntax and diff checks), the same Rust tests/strict Clippy on
1.91.0, and both ARM builds through `scripts/build-pocketchip.sh`. No new
libraries or dependency changes were needed; `cargo tree --duplicates` is empty.

The installed ARM binary is SHA-256
`ea0c35caff3a9d5d63592ddecd9fb33b4f4eb845a663c7c4714eb6ae0c3e571b`.
The follow-up passed 25 owned-process close/reopen/Home/resume cycles (five each
Bitcoin, Store, Terminal, Write and Files), followed by five more cycles after
clearing recovered transport errors. The subsequent worker-only change leaves
that app/process code unchanged and receives its own device checks.
The first focus-only close harness had one ambiguous target failure; the
corrected harness closes the specific owned test window and waits for its
former PID to be reaped. Its log reader verifies the log inode across rotation
and refuses an empty evidence slice.

Final slider testing verified all brightness keypad steps from 10 to 100 and
back, minimum clamping, live writes while the pointer remains pressed, touch
jitter filtering, and coalescing a rapid drag to its final 50% target. An initial
bursty XTest keypad sequence missed a step; the repeat used distinct 150 ms
release intervals and passed. A longer run on the earlier shared worker exposed
a **2.006 s** hardware delay. After splitting status and control workers, **64
live steps spanning a status polling interval had a maximum latency of 0.291 s**;
the initial 17-step sweep ranged from 0.035 to 0.102 s. These are small injected
input/readback samples, not a claim about subjective physical feel or endurance.
Volume changed while the pointer was held (30%, then 60%), and keypad adjustment
reached 70%. The original native brightness 7 and ALSA volume 75% were restored.
F1 left the home screen unchanged. A screenshot comparison exposed that a late
focus-loss event could close the restored settings panel after Wi-Fi exited.
Focus loss now clears gestures and power confirmation while retaining the
settings screen. The regression covers the exit/focus event ordering.
The current controls show ten brightness levels as 10–100%; audio detents round
nearby hardware readback, so the restored 75% mixer level displays as 80%.

The final build passed five Wi-Fi close/return cycles with actual rendered
System Settings header comparisons, plus Terminal launch/Home/resume/close.
Marshmallow fallback reached an inactive unit, removed the old launcher process,
and restored the original Home binding. Relaunch succeeded; the device was left
on System Settings with native brightness 7 and mixer volume 75% restored.
The serial console was at `agetty`, with no authenticated shell. The temporary
SSH key was removed, a fresh connection using only that key was rejected, and
the local private/public key and login helper were deleted. No commits or pushes
were made; validation logs and screenshots remain under ignored `target/beta/`.


## Settings expansion — September 10, 2026

The current installed `0.1.0-beta.1` includes six embedded GPT Image settings
icons, the **App Center** name for free downloads and updates, a header IPv4
address, running-version display, persistent screen timeout, an installed-zone
picker with interactive device authorization, and the existing touchscreen
calibration utility. Back/More remain visible while controls report results.
Calibration uses its own fullscreen input surface without waiting for an Awesome
client window. Time-zone authorization returns to settings with immediate readback.

The current binary SHA-256 is
`b928204fa85c81f9556ff9e1c25c9700fd6bde926f456850595d72503a7ae6b2`.
Rust 1.91.1 and MSRV 1.91.0 each passed formatting, strict Clippy, 64 unit plus
5 integration tests, 21 Python tests, release and smoke checks. Both toolchains
also cross-built the ARM release. No dependencies were added.

Final-device checks passed five-app immediate close/reopen and Home/resume,
64 live slider changes across status polling (maximum observed write latency
0.298 seconds), touch jitter filtering, all 10% brightness keypad steps, volume,
five Wi-Fi close/return cycles, and Marshmallow return/relaunch. A real authorized
time-zone change and restoration passed. The user completed calibration; a later
cancel test preserved the new saved and live matrix. See the
[settings expansion report](devices/pocketchip/settings.md) for the complete changed-file
list, commands, persistence and screenshots/log locations. The user also confirmed
that physical input wakes the display normally after the 30-second timeout.


Final restoration verified the saved preference and active X timers at 600 seconds,
America/Vancouver, native brightness 7 and mixer volume 75%. The user's completed
calibration remained intact. The serial console returned to agetty. Temporary sudo
authentication was invalidated; exactly the temporary SSH key was removed while
preserving other authorized keys. After closing the shared SSH connection, a fresh
connection using only that key failed with public-key authentication denied (exit
255). The local private/public key files were deleted. No commits or pushes were made.
