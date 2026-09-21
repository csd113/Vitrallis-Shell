# Visual redesign validation

The crystal imagery is confined to startup. Normal screens retain their original
icons and layout and use the shared color theme. No versions, dependencies,
commits, releases or physical-device installations were changed by this work.

## Checks

| Command or check | Result |
| --- | --- |
| `cargo fmt --all --check` | Passed |
| `cargo clippy --workspace --all-targets --all-features -- -D warnings -D clippy::all -D clippy::pedantic -D clippy::nursery -D clippy::cargo` | Passed |
| `cargo test --workspace --all-features` | Passed; opt-in hardware/performance checks remain explicitly ignored by the normal suite |
| `python3 -m unittest discover -s tests -p 'test_*.py'` | Passed, 102 tests; optional renderer cases separately exercised after building binaries |
| `sh scripts/validate.sh` | Passed: formatting, locked checks, Clippy, Rust/Python tests, release build, native renderer checks, smoke tests, shell syntax, Python compilation, documentation links and diff whitespace |
| Docker `tests/simulator/run.sh` | Passed: Linux Rust/Python suites, real Awesome session integration, App Manager lifecycle and keyboard/mouse/touch shortcut flows |
| `cargo test --test desktop accelerated_readback_and_presentation -- --ignored --nocapture` | Passed: process-based host software/hardware/auto readback and presentation |
| `VITRALLIS_RENDERER_BIN_DIR="$PWD/target/release" VITRALLIS_TEST_ACCELERATED=1 python3 -m unittest discover -s tests -p 'test_native_renderer.py'` | Passed: native app GPU/software equivalence at 480×272, 800×480 and 1280×720 |
| `cargo test --lib rendering_workloads -- --ignored --nocapture` | Passed: all steady rendering workloads recorded zero image decodes and zero texture uploads |
| Boot lifecycle and artwork checks | Passed: keyframe dimensions, malformed/wrong-size rejection, reset/reload pixel identity, Escape handoff, close and discovery error handling |
| Contrast and visual references | Passed: body/secondary/disabled/focus text contrast; 227 reviewed frame hashes on both macOS and Linux |

Native 480×272 images were visually inspected for text, clipping, selected and
unavailable controls, storage errors, installation/update controls, App Manager
lists/details, the home running badge and the startup stages. The render fixture
also covers 320×200, 800×480 and 1280×720. Existing Settings and App Manager
keyboard regression tests ran unchanged. The interactive simulator verified
install/update/remove, failure preservation, safe confirmation defaults,
selection/filter preservation, native launch/Home/resume and keyboard/touch flows.

## Performance and limits

A separate Linux aarch64 Docker measurement used the real 480×272 Shell with SDL's
dummy software driver and isolated homes. A baseline was built from the unchanged
HEAD source, outside the working source files. Five seconds of idle measurement:

| Metric | Baseline | Redesign |
| --- | ---: | ---: |
| Ready after process start | 51 ms | 3,090 ms, including the intentional three-second sequence |
| Idle CPU, one core | 0.6% | 0.8% |
| Idle resident memory | 13.04 MiB | 13.57 MiB |
| Idle resident-memory growth | 0 bytes | 0 bytes |

These short host samples are diagnostic, not PocketCHIP performance estimates.
The new boot assets total 351,209 compressed bytes and five RGBA32 textures total
2,611,200 bytes. No boot texture or animation loop survives handoff. The code keeps
the existing synchronized presentation boundary. The USB follow-up below covers
ARM timing and hardware rendering; physical scanout still requires visual
observation. The installed device generation was not replaced.

The following opt-in history remains explicitly recorded:

- `idle_loop_stops_after_startup` fails its fixed `frames <= 2` assertion with
  three startup frames on both unchanged baseline and redesigned builds. The
  event loop and that test are unchanged. The independent process measurement
  showed no idle memory growth. This unrelated assertion was not weakened.
- The macOS in-process boot test could not open a video device. The USB device
  provides the required graphical environment; see the follow-up below.

Logs, full-size captures and the comparison preview are in the ignored
`target/redesign/` directory. Simulator artifacts are in
`target/app-center-audit/docker/`.

## USB PocketCHIP follow-up — 2026-09-19

The owner made the device available over USB for validation. Current ARMv7
executables ran from a private temporary directory using the existing X11/Picom
session and Mesa Mali400/GLES2, with an isolated test home. No installed binary,
version, service, package or device setting was replaced. Original configuration,
receipts and the installed generation link matched their before-test snapshots.
The original launcher retained its process and regained focus after each preview.
The temporary device files were removed after collecting local evidence.

Owner feedback during the preview resulted in two focused corrections:

- The full crystal is a transparent layer reused at the identical position and
  scale throughout the cave reveal. The cave background has no second crystal;
  it fades in underneath the existing sprite. Every frame still needs at most
  two cached texture copies. The supplied lettering remains in the background.
- The home Actions button now shares the rightmost tile's horizontal bounds.
  The added separator strokes above and below the grid were removed. Existing
  keyboard and pointer activation continue to use the same layout bounds.

The hardware graphics self-test passed with SDL backbuffer, hardware GLES2 and
verified GL swap interval 1. All three native applications passed hardware smoke
tests. All 37 boot frames passed the bounded hardware/software comparison. The hardware
scene comparison and font-atlas refresh check passed.
Keyboard input exercised home, App Center and Settings focus on the real display;
the temporary generic-platform preview intentionally did not change hardware
controls or installed applications. X11-injected input does not validate physical
keypad switches or touch calibration.

Mali/software pixel comparisons explicitly bound rounding differences, rather
than require identical implementations of alpha blending and linear filtering.
Transparent boot layers differ by at most 5/255 per channel; sparse blends allow
at most 0.2 average channel error, and full-screen cave fades at most 1.25.
The dense icon fixture retained its existing average-error limit; the navy
background produces two channels at 7/255 instead of the prior 6/255 maximum.
The unchanged HEAD ARM baseline passed with its original palette and tolerance.

A quiet twenty-second final preview measured 0.2% process CPU, 37.26 MiB resident
memory and zero resident-memory growth. Process launch to ready was 4,835 ms,
including library/GPU setup and the intentional three-second sequence. These
are bounded measurements, not battery-life or endurance guarantees. The existing
compositor and synchronization settings were retained; new physical scanout or
no-tearing certification is not claimed.

The full host validation script and complete Linux interactive simulator passed
again after the layout and artwork corrections. An interim host run encountered
failures in concurrently edited bootstrap tests; the final full run passed all
123 Python tests (three conditional skips). Device scripts, logs, pixel captures
and final screenshots are retained locally under `target/redesign/device/`.

## Files changed for this task

The pre-existing documentation edits were preserved and are excluded from this
list.

> This section records the earlier redesign pass. The later usability and
> lifecycle pass is recorded below.

- `crates/vitrallis-native/src/theme.rs`: shared colors, metrics, cards, progress
  treatment and contrast regression check.
- `crates/vitrallis-native/src/lib.rs`, `crates/vitrallis-native/src/ui.rs`,
  `crates/vitrallis-native/src/browser.rs`: theme integration.
- `apps/files/src/lib.rs`, `apps/notepad/src/lib.rs`,
  `apps/terminal/src/lib.rs`: current consumers use the shared palette.
- `src/preferences.rs`, `src/layout.rs`: default background and named text metrics.
- `src/renderer/performance.rs`: measured Mali/software icon-filtering tolerance
  for the changed background, retaining the existing total-error bound.
- `src/renderer.rs`, `src/renderer/app_center.rs`, `src/renderer/shortcuts.rs`,
  `src/renderer/system.rs`, `src/renderer/system_storage.rs`: themed interface
  drawing and expanded render fixtures, preserving control geometry.
- `src/boot.rs`, `src/ui.rs`, `src/lib.rs`: bounded startup playback, concurrent
  discovery, asset fallback/reset, current window geometry at handoff and tests.
- `assets/boot/{clean,subtle,cyan,full,scene}.png`, `assets/boot/README.md`:
  native startup frames and usage notes.
- `assets/branding/source/{clean,subtle,full,small}.png`,
  `assets/branding/crystal-{8,16,24,32,64,128}.png`,
  `assets/branding/README.md`: reusable artwork hierarchy and preparation notes;
  none replaces normal application icons.
- `scripts/prepare-branding.py`: optional deterministic asset packing tool.
- `tests/desktop.rs`, `tests/fixtures/renderer/phase1-sha256.json`: updated color
  assertions and reviewed reference pixels, including boot/home states.
- `tests/simulator/lifecycle.py`, `tests/simulator/session.py`: wait for actual
  Shell readiness and assert current palette colors.
- `docs/visual-design.md`, `docs/visual-design-validation.md`: design, operation,
  reproduction, results and limitations.

# Usability and lifecycle pass

A later pass fixed the shared drawing primitives, reorganised Settings, overhauled
the App Center, polished Terminal and Notepad, and made application launching and
background lifetime explicit. It is recorded separately because it changes
behaviour, not only pixels.

## Checks

| Command or check | Result |
| --- | --- |
| `cargo fmt --all --check` | Passed |
| `cargo clippy --workspace --all-targets --all-features -- -D warnings -D clippy::all -D clippy::pedantic -D clippy::nursery -D clippy::cargo` | Passed |
| `cargo test --workspace --all-features` | Passed, including the desktop integration suite |
| `VITRALLIS_QA_DIR=… cargo test --lib system_panels_render_at_device_and_scaled_sizes` | Passed against the regenerated `macos` reference block (262 frames per size sweep) |
| Settings keyboard walk | Passed: every home option is reachable by arrows/Enter, every category has a visible Back control, and the home menu is the only exit |
| Lifecycle state machine | Passed: launch → launching → running → background → foreground → explicit close, plus launch → failure → retry, and duplicate-launch refusal |

## Findings and fixes

- The focused-panel decoration drew a violet underline and a blue inset edge
  inside every highlighted box. That is the reported bottom-right stray line. The
  shared `card` primitive now draws exactly one border, and the running badge no
  longer draws a separate rail along the bottom of a tile.
- Progress bars and sliders used three fixed colour blocks. Both now use one
  continuous cyan/blue/violet ramp that belongs to the track; sliders dim it and
  add a solid thumb for the inactive/active/handle/focus distinction.
- Launching ran on the UI thread, so the menu looked frozen. Process creation now
  runs on a worker and reports back through `poll_launch`, with a status-line
  message instead of a modal screen.
- Returning to the main menu cleared the foreground owner only. The policy is now
  explicit: returning home backgrounds the app, the configured timeout is the only
  automatic stop, advisory close is followed by a bounded stop, and explicit
  termination is unchanged.

## Pending validation

- The `linux` pixel-reference block was removed with this pass because every frame
  changed. Run the documented QA command on the Linux simulator, review the new
  screenshots and re-add the block before the next release validation.
- Physical PocketCHIP checks remaining: Mali-400/Lima presentation of the new
  primitives, real launch latency on device storage, background-lifetime policy
  against an App Center-installed app, and Terminal/Notepad keyboard/touch feel.
