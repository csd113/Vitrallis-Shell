# Phase 3 physical graphics validation

Status: physical acceptance completed on 2026-09-14, within the scope and
measurement limitations recorded below.

The only physical GPU target in this phase is PocketCHIP at `10.0.0.137`, accessed
by SSH as the normal `chip` desktop account. The desktop is X11/Awesome at 480×272.
No boot configuration, graphics stack, installed generation or system package was
changed. Test builds and test data use isolated user-owned directories.

- PocketCHIP Mali-400/Lima: TESTED / PASS.
- Raspberry Pi 1 / VideoCore IV / vc4: PHYSICAL GPU VALIDATION PENDING.
- Raspberry Pi 4 / VideoCore VI / V3D: PHYSICAL GPU VALIDATION PENDING.

The [environment transcript](evidence/graphics-phase3/environment.txt) records
Debian 13.7, kernel `6.12.107+deb13-chip`, SDL `2.32.4+dfsg-1`, Mesa
`25.0.7-2+deb13u1`, EGL 1.5 and GLES 2.0. SDL uses `opengles2`; GL reports
`Mesa`, `Mali400` and `OpenGL ES 2.0 Mesa 25.0.7-2+deb13u1`. The display card uses
`sun4i-drm`; the GPU card and render node use `lima`.

See [hardware acceleration](../../hardware-acceleration.md) for architecture,
troubleshooting and the future physical test procedure. Software CI and ARM
compilation never substitute for these physical results.

## Completed idle observation

The staged Auto renderer remained running for **960.43 seconds (16 minutes)**.
The [samples](evidence/graphics-phase3/idle.json) contain 17 observations:

- CPU: 450 ticks at 100 Hz, approximately **0.47% of one CPU** averaged over the run.
- RSS: 43,732 → 43,784 KiB, a 52 KiB increase; anonymous RSS increased by the same amount.
- File descriptors: normally 13, briefly 14, ending at 13.
- CPU thermal sensor: 47.2–50.2 °C. This is not a separate GPU temperature measurement.
- GPU runtime PM: about 786 seconds suspended and 175 seconds active across the interval.
- GP/PP interrupts increased; they are system-wide and include the existing desktop,
  installed shell and clock/status changes. They cannot establish this process's
  utilization percentage. Runtime suspension shows the GPU was not continuously busy.

The [process observations](evidence/graphics-phase3/process-evidence.txt) link the
staged executable to DRM descriptors and Mesa mappings. The device tree identifies
`arm,mali-400`; the bound kernel driver is `lima`. The existing installed generation
was left running behind the temporary test window; its unrelated terminal/htop
was not stopped. Therefore these are coexistence measurements, not isolated whole-
system power benchmarks. The host idle instrumentation separately verifies no
redraws after initial exposure when visible state stays unchanged.

The [endurance binary hashes](evidence/graphics-phase3/endurance-binaries.sha256)
identify that build. Later diagnostic-only safety/formatting refinements are
checked with a separate final build and final self-tests; do not conflate hashes.

## Test harness corrections

The first app-cycle attempt correctly encountered Terminal's safe default Cancel
confirmation; the test needed to select Close. The second attempt encountered an
X11 enumeration race while a test window closed. The temporary harness was fixed
to handle disappearing windows and explicitly confirm only its owned terminal.
Neither event was a renderer failure. The ordinary shell/native tests continue to
preserve safe confirmation defaults.

## Software and build gates

Both complete `sh scripts/validate.sh` gates passed on macOS and Linux:
`cargo fmt --all --check`, locked workspace check, strict workspace Clippy with
all/pedantic/nursery/cargo denied, 231 Rust tests, 84 Python tests (two gated
skips), release builds, native screenshot parity, shell/native lifecycle smokes,
script syntax, local documentation links and diff whitespace. ARMv7 release
binaries and the physical scene-test executable were cross-compiled in Debian
Bookworm with target SDL libraries; no host SDL library was substituted.

The deterministic Linux simulator passed App Center lifecycle, desktop shortcut
keyboard/mouse/touch, and Awesome native-session scenarios. The Mesa/Xvfb test
correctly rejects software Mesa in Hardware and falls back in Auto. These are
software/API tests, not physical GPU acceptance. macOS Metal desktop and native
screenshot comparisons passed. Its in-process atlas unit test could not initialize
a video device on Rust's test thread; the actual physical Linux atlas test is
reported separately. This host limitation was not hidden by weakening CI.

The opt-in software workload benchmark and two-second idle instrumentation passed.
The latter rendered two startup/exposure frames and no frames after the first
500 ms. The full 16-minute device record above includes real system-status updates.
No version, dependency, commit, release or installed device generation was changed.

## Final physical acceptance results

| Check | Actual result |
| --- | --- |
| Auto | PASS: X11 `opengles2`, Mali400, GLES 2.0 Mesa 25.0.7, SDL flags `0xe`, vsync flag set, no fallback. |
| Hardware | PASS: same genuine Mali/Lima path; self-test and lifecycle smoke succeed. |
| Software | PASS: explicit SDL software; self-test, native screenshots and lifecycle smoke succeed. |
| Forced software Mesa | PASS: `LIBGL_ALWAYS_SOFTWARE=1` yields recognized software Mesa; Hardware exits 1, Auto falls back and passes. |
| Explicit EGL | PASS with process-local `SDL_VIDEO_X11_FORCE_EGL=1`; EGL 1.5, GLES 2.0, Mali400; EGL display driver correctly labeled `sun4i-drm`. |
| Cold launch/home/icons | PASS, real 480×272 framebuffer capture inspected. |
| 16-minute idle | PASS, measurements above. |
| Native launch/exit | PASS: ten cycles each of Terminal, Notepad and Files; 30 clean exits and shell fullscreen restoration checks. |
| Ten-minute navigation | PASS: 9,672 injected X11 arrow events; no repeated renderer errors. |
| Settings/App Center/shortcuts | PASS: live panels captured and inspected; test-only detailed scenes additionally cover text, dialogs and pickers. No package installation or power action was performed. |
| Native fullscreen | PASS: final binaries with the normal `VITRALLIS_SESSION=1` flag render at 480×272 and close cleanly. |
| Screenshots/text | PASS: Auto/Hardware/Software home and all three native readbacks; native screenshots match software exactly, including a 120-line Notepad document. |
| Wallpaper and dense icons | PASS: actual production scene rendering, with embedded fixture artwork materialized on the device and successful upload asserted. Wallpaper has no current user-facing configuration control. |
| Atlas refresh | PASS on the physical Mali renderer after five unchanged-artwork refreshes. |
| Clean shutdown | PASS: all temporary test apps and the test shell exited; the previously installed generation and its unrelated Terminal/htop remained intact. This is application shutdown, not powering off the device. |

The [final command results](evidence/graphics-phase3/results.json) record exit codes
and wall times. [Readback comparisons](evidence/graphics-phase3/readback-comparisons.json)
record every mode. [Explicit EGL output](evidence/graphics-phase3/egl-hardware.txt)
and [Hardware diagnostics](evidence/graphics-phase3/graphics-info-hardware.txt)
retain the exact identities. The [stress samples](evidence/graphics-phase3/stress.json)
record 30 cycles plus all ten navigation minutes. [UI results](evidence/graphics-phase3/ui-results.json)
record final native exits.

Across app cycles and navigation, RSS grew from 43,812 to 43,848 KiB (36 KiB).
Descriptor counts stayed at 13–14. The last nine navigation minutes averaged
21.48% of one CPU (100 Hz process ticks); the first minute includes transition
work. CPU temperature during navigation was 49.8–50.8 °C. GP/PP interrupts rose
strongly under navigation, corroborating GPU work alongside the active GL identity,
Lima-bound render node, actual process DRM descriptors and Mesa library mappings.
No reliable GPU utilization percentage, separate GPU thermal sensor, battery power
measurement or exact input-to-photon latency was available. These bounded runs
show no sustained renderer-resource growth, not proof against every possible leak.

The [20-scene physical comparison](evidence/graphics-phase3/scenes-final.log) covers home, settings, modal, nine App Center
states, four shortcut states, 1,000 icon references, wallpaper and navigation.
Text-heavy panels differ by at most one channel level. Dense linearly filtered
icons differ by at most 6/255, mean 0.161/255, between SDL software and Lima.
Both images were inspected. The new test bounds every dense-icon channel to six
and total error to 0.2/channel; other scenes retain the original <1% differing-
channel bound. This is a measured backend-filtering allowance, not a production
rendering change or a relaxation of existing CI tests. Existing native parity and
font-atlas pixel checks remain exact. A failed initial comparison is preserved in
the working artifacts, not misreported as an initial pass.

## Changes, scope and remaining work

Implemented reusable active-context GL identity, software-Mesa rejection, optional
Linux DRM/library diagnostics and EGL display-driver querying in the existing
shared renderer. Added `--graphics-info` and the non-mutating `--graphics-test`.
Auto retains the original SDL/Mesa errors and falls back; required Hardware fails
with recovery advice. Installer EGL/GLES/DRM checks are advisory; the installed
Debian packages were inspected, not changed. No missing-DRM install failure was
introduced. The architecture remains GLES2 compatible without board-specific GPU
selection or proprietary dependencies.

The reviewed local Debug app source exposes CPU utilization, not a Mali GPU
counter. On the target, devfreq is empty and DRM fdinfo lacks engine-time fields;
there is no defensible conversion of those absent readings into 0% GPU activity.
The independent evidence above is the basis for acceleration acceptance.

No blocker remains for this phase. Manual physical key/touch actuation, longer
soak/power-loss testing and precise power/latency measurements were not performed.
Input in this record is injected X11 input on the real physical GPU/display.
No device was reflashed, rebooted or given experimental graphics packages. No
Raspberry Pi was contacted or physically tested. Future physical execution of the
[vc4/V3D procedure](../../hardware-acceleration.md#future-physical-procedure-vc4-and-v3d-still-pending)
is still required; compilation, mocks and software Mesa do not satisfy it.

After collecting and verifying a local evidence archive, both temporary device
work directories and their temporary archive were removed. No test process remained;
the original installed session and unrelated applications were preserved. The
complete working archive is retained locally under `target/graphics-phase3/`;
the concise reviewed evidence beside this document is checked into the worktree.
