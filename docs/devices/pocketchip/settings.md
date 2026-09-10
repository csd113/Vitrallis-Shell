# System Settings expansion — 0.1.0-beta.1

This is the current settings interface; earlier backend audits and release notes describe previous screens.

## Controls

- The launcher and settings header show the device's Wi-Fi IPv4 address, falling back to USB IPv4 when Wi-Fi has no address. Missing addresses display `IP --`. IPv6-only networks are not displayed by this backend.
- The System Settings header displays the version compiled into the running binary, rather than a potentially different installed receipt.
- Gear, Wi-Fi, brightness, volume, restart and power icons are transparent GPT Image assets embedded in the binary. They load once, use linear filtering at small sizes, and require no runtime files or network access. [Assets and complete prompts](../../../assets/system/README.md) document the built-in generation workflow.
- **App Center** is the launcher's name for the existing free app download/update utility. Its original installation paths, IDs, catalogue and upstream `Update Apps` window title are preserved. There is no purchasing or new package ecosystem. The selected tile's footer explains that it provides free apps and updates.
- Brightness and volume retain their existing live 10% sliders. Choose **More** at the bottom, move down to More and press Enter, or press Page Down to reach the additional settings.
- **Screen timeout:** Never, 30 seconds, 1, 2, 5, 10 or 30 minutes. Left/right keypad changes the selected timeout; tap the left portion to decrease or the remainder to increase. Wait for Applying to finish before another change. Touching a timeout row does not drag the brightness slider.
- **Time zone:** choose from the installed `zone.tab` database plus UTC. Up/down moves one zone; left/right or page keys moves five. Touch selects a visible row; Previous/Next moves a page. `*` marks the current zone. Choosing a zone applies through timedatectl; when authorization is needed, a supervised terminal requests the normal device password through sudo. Ctrl+C cancels; after completion Enter returns to settings. No password is stored or read by Vitrallis. The clock and zone display refresh from system readback, including an immediate refresh when the authorization terminal closes.
- **Calibrate touchscreen:** launches the image's existing `/usr/local/bin/pocketchip-calibration` utility. Tap its crosshairs accurately; any key cancels. The existing utility saves completed calibration for future logins and restores the previous matrix when cancelled. A missing utility displays unavailable; the installer does not install or replace calibration tools.

Back and More remain visible while a setting reports Saved or an error. Escape returns from the zone list to additional settings, from additional settings to the first settings page, and from there to the launcher. F1 remains unbound.

## Persistence, permissions and recovery

Timeout uses the X server's screen saver and DPMS timers, covering the current graphical session. The user confirmed that physical input wakes the display normally after a 30-second timeout. Synthetic XTest input did not wake DPMS on this X server; that automation limitation does not reproduce with physical input. This turns the display off; it does not lock the session or suspend the device. Applications that explicitly inhibit screen saving can still override X's idle behavior.

A selected timeout is saved atomically with mode 0600 in `~/.config/vitrallis/screen-timeout` and restored when Vitrallis next starts. Missing preferences leave the existing timer alone. Unsupported values and symlinked paths are rejected. A failed application/readback/save attempts to restore the previous X timers. The setting stays effective when returning to Marshmallow in the same X session; after a fresh login, Vitrallis restores it when launched. Choose Never to disable idle blanking, or choose the previous value to undo a change.

Time-zone changes use the existing device authorization policy and system zone database; no sudoers or polkit rule is installed. The interactive helper validates its single zone argument against that database and invokes the fixed sudo/timedatectl executables with explicit arguments. Only the password-taking subprocess receives the authentication input. All interactive settings utilities remain owned by the Vitrallis session and return to the additional settings page on exit.

The normal worker continues polling status independently of controls. Timestamped readback prevents an older snapshot from reverting newly applied brightness, volume, timeout or time-zone fields. Bounded, noninteractive helper commands retain the two-second deadline. The deliberate interactive password/calibration applications use the existing supervised app lifecycle instead.

No Marshmallow binary, Awesome configuration, X startup file, calibration implementation, default boot setting, system package, or authorization policy is changed by the Vitrallis installer.

## Files changed for this expansion

- `assets/system/{gear,wifi,sun,speaker,power,restart}.png`, `assets/system/README.md`
- `src/discovery/store.rs`, `src/process.rs`, `src/lib.rs`
- `src/platform/mod.rs`, `src/platform/system.rs`, `src/platform/pocketchip.rs`, `src/platform/pocketchip/display.rs`
- `src/settings.rs`, `src/settings/device.rs`, `src/settings/geometry.rs`, `src/settings/pointer.rs`
- `src/renderer.rs`, `src/renderer/system.rs`, `src/ui.rs`
- `README.md`, `docs/devices/pocketchip/store.md`, `docs/devices/pocketchip/system-status.md`, this report and the current release record

Existing changes from the earlier beta hardening work remain intact. No new Cargo or Python dependencies are required.

## Validation — September 10, 2026

The installed ARMv7 binary is `0.1.0-beta.1`, SHA-256
`b928204fa85c81f9556ff9e1c25c9700fd6bde926f456850595d72503a7ae6b2`.
The pinned Rust 1.91.1 artifact matches the device binary. No new dependencies,
commits or pushes are part of this expansion.

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
under ignored `target/beta/`; only the six curated PNG assets and their prompts
are repository content.

Final restoration verified the saved preference and active X timers at 600 seconds,
America/Vancouver, native brightness 7 and mixer volume 75%. The user's completed
calibration remained intact. The serial console returned to agetty. Temporary sudo
authentication was invalidated; exactly the temporary SSH key was removed while
preserving other authorized keys. After closing the shared SSH connection, a fresh
connection using only that key failed with public-key authentication denied (exit
255). The local private/public key files were deleted. No commits or pushes were made.
