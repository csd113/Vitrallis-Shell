# PocketCHIP hardware evidence

These are historical observations from September 2026, before the current native
utilities and complete-bundle lifecycle. They are retained to explain the tested
Marshmallow integration. Old Store/package behavior in the matrix is historical,
not a supported package contract. F1 and title-based app resume mentioned in old
evidence have since been replaced; see [current controls](../../../../README.md#first-launch-and-controls).
No new physical-device test is claimed by this document.

The inspected image used Debian 13, ARMv7, Linux 6.12.94, SDL2 2.32.4, Awesome 4.x
and a 480×272 panel. Physical arrows/Return/Escape, touch, Home/resume and return
to Marshmallow were user-confirmed. Separate injected X input covered repeated
starts, app ownership, crashes and recovery. Original Marshmallow binaries and
Awesome startup configuration hashes were preserved across reboot and cold start.

| Feature | Status | Evidence and deliberate differences |
| --- | --- | --- |
| Normal session startup | PASS | Separate transient user service on the physical panel; original Awesome/greetd startup preserved. |
| Repeated restart | PASS | Five real-device start/stop cycles; original home regained each time. |
| App grid/order/pages | PASS | Existing five app entries plus configured Wi-Fi and fallback tile; host boundary tests and user-confirmed second page/fallback. |
| Keyboard/touch/layout | PASS | User confirmed physical arrows, Return, Escape, tapping Store, readable panel and one-tap resume. Injected X tests are separately identified. |
| Selection/focus | PASS | Selection retained by identity; physical Home/resume confirmed. Session-scoped Awesome binding restored on exit. |
| Application launch/return | PASS | Fresh Bitcoin, Store, Terminal, Write, Browse Files and nmtui windows observed on hardware. |
| Multiple running applications | PASS | Physical device with injected input: Bitcoin → Home → Store → Home → resume Bitcoin → close → resume Store → close; both original PIDs reaped, no duplicate starts. Test first tapped an incorrect Store Home coordinate, then the supported Home key closed it; preboot log confirms both exits. |
| Child crash/missing executable | PASS | Real `/bin/false` reaped; missing command showed dismissible error; subsequent Store launch/return worked. |
| Launcher/supervisor crash | PASS | Separate SIGKILL tests restored original home/key bindings and removed owned descendants. |
| Malformed/missing entries/config | PASS | Parser fixtures reject malformed fields, preserve visible unavailable entries; startup remains usable, refresh keeps last valid catalogue. Host tests. |
| Icons/wallpaper | PARTIAL | PNG/BMP bounded decode, real icons visible, host wallpaper/color pixel test. SVG/JPEG and Unicode/font parity unsupported. |
| Personalization | PARTIAL | Reads existing background, clock visibility/12-hour mode and cursor settings; editing uses preserved Marshmallow settings. No duplicate preferences writer. |
| Battery/external power/Wi-Fi/clock | PARTIAL | Kernel sysfs and nmcli available on hardware; no forced I²C when kernel driver exists. Full charge/radio transitions were not measured; the final current-state readings and Marshmallow battery-label difference are recorded below. |
| Brightness/volume | PASS | Real UI taps changed backlight 7→6→7 and amplifier raw 47→41→47; original values verified restored. Audible output and physical full-range perception not measured. |
| Power cancel/denial | PASS | Real reboot confirmation cancelled with Escape; boot ID unchanged. Host tests cover default cancel, expiry, focus-loss cancellation and denied fixed command. |
| Reboot/cold start | PASS | Confirmed UI reboot returned to Marshmallow; user separately confirmed full shutdown and physical power-on. New boot ID and actual post-cold-start menu/Bitcoin/return workflow verified. |
| Settings entry points | PASS | F1/touch system controls plus configured nmtui entry; real terminal and nmtui launch/return. |
| Sleep/lock/login/advanced settings | PARTIAL | Preserved Marshmallow remains reachable. Reference sleep couples DPMS with its own lock UI; copying just screen-off would omit authentication semantics. No replacement password storage or fabricated security boundary. |
| FEL flashing mode | NOT SAFELY TESTABLE | Reference invokes recovery/flashing behavior; excluded by device-preservation requirement. Preserved Marshmallow retains its original features. |
| Bluetooth | NOT APPLICABLE | Reference is fixture UI, not a live pairing backend; Vitrallis reports unavailable. |
| Date/timezone administration | PARTIAL | Reference invokes sudo dpkg-reconfigure in a terminal; no additional privileged editor installed. Existing terminal/Marshmallow route retained; display format read. |
| Store catalogue/metadata | PASS | Existing explicit APPS catalogue and pinned GitHub commit/blob workflow; real Check displayed Bitcoin version and installation state. |
| Bitcoin install/entry/launch | PASS | Fresh installation through Store UI on physical device; generated launch/icon/menu, exact upstream source hash, live Bitcoin UI and return. |
| Installed/reinstall/update state | PASS | Real installed state/Check; upstream regression suite and repair marker tests on ARM. |
| Store network failure | PASS | Child-only unreachable proxy on device; visible refusal, install disabled, clean Home/child exit. Device network configuration unchanged. |
| Malformed/incomplete packages | PASS | 44 upstream tests on physical ARM include checksum/metadata errors, missing files, rollback and abrupt installer exit/repair. Launcher refuses pending install. |
| Store self-update | PARTIAL | Manual under reviewed safety patch to avoid remote replacement silently removing it; Bitcoin update supported. |
| Uninstall | NOT APPLICABLE | Existing Store has no safe removal workflow; no parallel package manager introduced. |
| Read-only storage | PASS | Real non-writable session log failed closed and restored Marshmallow; original permissions restored. Installer rollback fixtures also pass. |
| Full filesystem | NOT SAFELY TESTABLE | Filling shared device storage would risk existing data/services. File-operation failures and rollback tested in isolated fixtures. |
| Bounded logs/relaunch/orphans | PASS | Two bounded 128 KiB logs; no restart loop/PID file; cgroup cleanup on real Rust/supervisor failure. Focus fallback never grants ownership of external processes. |
| Marshmallow/recovery | PASS | Original binary/wrapper/Awesome hashes unchanged, physical fallback tile confirmed, crash recovery focused original home. Reboot returned to focused Marshmallow; Vitrallis/Bitcoin also passed after reboot. |


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


These short samples are not endurance, thermal or battery-life benchmarks, and
must not be presented as measurements of the current native utilities.

## Retained evidence

The [evidence directory](../evidence/step3) retains the original command outputs
and screenshots, including failed intermediate checks with later corrections:

- [Physical input confirmation](../evidence/step3/device-physical-input.txt)
- [Session and controls](../evidence/step3/device-controls.txt)
- [Supervisor crash](../evidence/step3/device-supervisor-crash.txt)
- [Power and preservation](../evidence/step3/device-power-and-preservation.txt)
- [Postboot launch](../evidence/step3/device-postboot-launch.txt)
- [Cold-start final state](../evidence/step3/device-cold-final.txt)
- [Final cleanup](../evidence/step3/device-cold-final-cleanup.txt)
- [Original home after cold start](../evidence/step3/final-marshmallow.png)
- [Access revocation](../evidence/step3/device-key-revocation.txt)

A two-second cleanup check once observed `deactivating`; the configured stop
limit was five seconds. The subsequent check found an inactive unit, no owned
Vitrallis process, focused Marshmallow and restored keys. Boot readiness was
also checked separately from kernel reboot: Awesome registered after the system
returned. Full storage and FEL flashing were not exercised. Battery/radio
transitions, audible range, endurance and original Jessie compatibility were not
established.

Current [native-app measurements](native-validation.md),
[App Center contract evidence](../../../app-center-validation.md), and
[validation commands](../../../validation.md) have separate scopes.

## Settings qualification — September 10, 2026

The ARMv7 binary tested was `0.1.0-beta.1`, SHA-256
`b928204fa85c81f9556ff9e1c25c9700fd6bde926f456850595d72503a7ae6b2`.
The pinned Rust 1.91.1 artifact matched that device binary.

Host validation passed on both pinned Rust 1.91.1 and MSRV 1.91.0:

```sh
sh scripts/validate.sh
RUSTUP_TOOLCHAIN=1.91.0 sh scripts/validate.sh
```

Each pass includes `cargo fmt --all --check`, strict workspace/all-target/all-feature
Clippy (`-D warnings -D clippy::all -D clippy::pedantic -D clippy::nursery -D clippy::cargo`),
`cargo test --workspace --all-features` (64 unit + 5 integration tests),
`python3 -m unittest discover -s tests -p 'test_*.py'` (21 tests), release build,
SDL dummy-driver launch/child-exit smoke test, shell syntax and `git diff --check`.
Cargo invocations also use `--locked`.

PocketCHIP release cross-builds passed with both toolchains:

```sh
PATH="$PWD/target/beta/tools/bin:$PATH" \
PKG_CONFIG_LIBDIR="$PWD/target/arm-libs/pkgconfig" \
sh scripts/build-pocketchip.sh
```

The MSRV pass adds `RUSTUP_TOOLCHAIN=1.91.0`; the pinned artifact was rebuilt last.
The renderer test also exported settings, additional settings, zone picker,
confirmation, unavailable controls and loading screens at 480×272, 800×480 and
1280×720. Device-size and scaled previews were inspected.

Device evidence:

- Header IP, running version, embedded icons and App Center label rendered on the actual 480×272 screen.
- The final binary passed Home/resume and immediate close/reopen for Bitcoin, App Center, Terminal, Write and Files, without duplicate app processes or launch errors. Loading feedback was captured before Bitcoin's window appeared.
- F1 remained unbound. Brightness passed all 10% keypad levels, live touch movement, held-touch jitter filtering and rapid changes settling on the final requested value. Across 64 changes spanning status polling, the maximum observed hardware-write latency was 0.298 seconds. Volume touch/keypad control and Wi-Fi open/return passed. Native brightness 7 and mixer volume 75% were restored.
- Timeout choices worked through touch/keypad and persisted. Thirty seconds caused actual DPMS off; Never disabled the timers. Explicit DPMS on restored the display. The original 600-second timeout was restored. The user subsequently confirmed normal wake from physical input.
- Time-zone selection requested authorization before changing the system. The normal password prompt changed America/Vancouver to America/Whitehorse; timedatectl readback matched. America/Vancouver was restored through the same UI. On the final binary, closing the authorization prompt without authenticating preserved Vancouver, reaped the helper and returned to additional settings with immediate readback.
- Five additional Wi-Fi close/return cycles matched the rendered System Settings header. Terminal Home/resume reused the same process. Marshmallow return stopped the session unit, removed the old launcher and restored the original Home binding; relaunch succeeded.
- The user confirmed completing the physical calibration targets. That saved result was preserved. A subsequent automated open/cancel preserved both the saved JSON bytes and live libinput matrix. The calibrator's override-redirect window now bypasses ordinary WM focus lookup while retaining supervised process ownership.

Logs, raw screen captures, rendered previews and temporary build material remain
under ignored `target/beta/`; the shipped artwork remains under `assets/system/`.

Final restoration verified the saved preference and active X timers at 600 seconds,
America/Vancouver, native brightness 7 and mixer volume 75%. The user's completed
calibration remained intact. The serial console returned to agetty. Temporary sudo
authentication was invalidated; exactly the temporary SSH key was removed while
preserving other authorized keys. After closing the shared SSH connection, a fresh
connection using only that key failed with public-key authentication denied (exit
255). The local private/public key files were deleted.
