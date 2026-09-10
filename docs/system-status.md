# PocketCHIP system integration and validation

Historical Step 2 record. The [Step 3 hardware compatibility report](compatibility-step3.md) supersedes the hardware-access restrictions and results below.

Implemented on 2026-09-10 against the static Marshmallow reference at [dccbd38dc233f3f45ebd6ea130d6a787268e23f5](https://github.com/o-marshmallow/PocketCHIP-pocket-home/tree/dccbd38dc233f3f45ebd6ea130d6a787268e23f5). The reference checkout is outside this repository. No hardware was accessed, packages installed, permissions changed, session files modified, or commits created. This extends the deliberately limited Step 1 audit.

## Audited mechanisms

| Capability | Reference evidence | Vitrallis implementation / limits |
| --- | --- | --- |
| Battery | `Source/BatteryMonitor.cpp`: `/dev/i2c-0`, forced slave `0x34`, register `0xb9`, ten-sample mean, 2-second interval | Optional `i2cget -y -f 0 0x34 0xb9 b`; same bus/address/register, bounded byte read. Valid values 0..100 only; suspended/uninitialized gauge and missing battery are unavailable. Instantaneous sample, not the reference mean. |
| Charging / external power | Same file reads register `0x00`, calling any nonzero byte charging | Read `0x00` and `0x01` separately. Usable ACIN/VBUS is `0x00 & 0x50`; connected battery and charging are `0x01 & 0x60 == 0x60`. This deliberately corrects the ambiguous reference label. |
| Brightness | `Source/SettingsPageComponent.cpp`: reads `/sys/class/backlight/backlight/brightness`, writes integer native levels 1..10 | Direct bounded file I/O, no shell. Require `max_brightness == 10` and a readable current level 1..10 before mutation. Generic 0..100 maps to native 1..10, rounded to nearest; zero means minimum visible brightness. Readback reports the actual quantized level. Unsupported driver/range disables this control. |
| Audio | Same file: `amixer sget/sset 'Power Amplifier'`, percentage 0..100 | Same control and arguments; parse bracketed percentages and optional on/off switch. Reject missing, out-of-range or unequal channel values. Bounded volume steps, actual readback, and MUTE display when reported. No separate mute toggle. |
| Wi-Fi | `Source/WifiStatusNM.cpp`: NetworkManager client, interface `wlan0`, wireless enabled and device state | Optional NetworkManager CLI adapter: `nmcli radio wifi` then `nmcli -t -f GENERAL.STATE device show wlan0`. Locale fixed to C. Reports off, disconnected, connecting, connected, or unavailable. CLI availability/version on the image still needs verification; no scan or connection mutation. |
| Bluetooth | `Source/Main.cpp` loads `bluetooth.json`; `Source/SettingsPageBluetoothComponent.cpp` toggles a fixture object's connected flag | No live BlueZ integration exists in this reference. Show unavailable, no fabricated adapter state or pairing controls. |
| Clock | `Source/ClockMonitor.cpp`: local time, 12/24-hour mode, one-second refresh | Optional `/bin/date +%H:%M`, validated 24-hour output, refreshed with system snapshot. Timezone inherited; no clock/timezone mutation. |
| Power | `Source/PowerPageComponent.cpp`: `systemctl reboot` / `systemctl poweroff`, spinner, no extra confirmation | Same actions plus `--no-ask-password`. Separate confirmation, Cancel initially selected, 15-second expiry; Escape/Home/F1/footer and focus loss cancel. No sudo, permission installation, or interactive privilege prompt. Failures return to the usable panel. |
| Buttons | Audited launcher arrows/Return and page controls; no complete physical Home/WM binding in this repository | Preserve existing navigation. F1/footer opens controls; SDL Power also opens the panel. Physical GPIO/evdev/global WM interception is not installed or claimed. |

Register interpretation is grounded in the [X-Powers AXP209 datasheet, sections 10.1, 10.2 and 10.52](https://aw-som.com/docs/public/products/AXP209_Datasheet_v1.0en.pdf). The fuel-gauge high bit suspends calculation, and `0x7f` is not a valid percentage. Unlike Marshmallow's broad reads, Vitrallis does not read unrelated registers or mutate PMIC configuration. The optional i2c-tools adapter uses the reference's forced address access; device permissions, bus ownership/concurrency and tool availability require review on the target before deployment. No assumption that the development host proves this adapter works on the installed image is made.

## Architecture and behavior

`platform::System` owns refresh and typed controls; `Platform` retains window/session policy. Validated `Percent`, `Control`, `Power`, `Wifi` and optional status fields keep hardware conventions out of the renderer and input code. `PocketChip` contains all device paths and command argument lists. Generic mode supplies local time and unavailable hardware; test mocks exercise successful/failed writes, slow reads and missing data.

The worker uses bounded channels (one queued request and one update), nonblocking UI submission, one outstanding control, and cached snapshots. Full refresh occurs ten seconds after the previous full refresh completes; control actions only reread their affected field and do not trigger I2C or Wi-Fi probes. Missing fields replace previous values instead of retaining misleading successes. After 30 seconds without an update, displayed hardware data is cleared. The UI wakes at most every 250 ms while idle to consume updates and expire confirmation; it redraws only when dirty.

Commands run without a shell, with null stdin/stderr, C locale, fixed standard executable directories, 8 KiB output limit and two-second timeout; timed-out children are killed/reaped. Reader threads keep full pipes from stalling the worker. Sysfs reads are bounded to 128 bytes, missing nodes are never created, and permissions are not escalated. There are no new Cargo dependencies. Ordinary filesystem/kernel stalls can still hold the worker, but not the UI; stale status then degrades to unavailable. This is not evidence of device frame-time performance.

The scalable system panel uses the existing six large tile hitboxes: brightness −/+, reboot; volume −/+, shutdown. The status line uses B=battery percentage, C=charging, P=usable external power, W=Wi-Fi connection, BT=Bluetooth, and local time. Unknown values use `?`/`--`. Mouse/touch retain matching press/release and synthesized-event filtering. UI controls are available separately from app launching; session and Marshmallow configuration remain untouched.

## Automated validation

| Command/check | Result |
| --- | --- |
| `cargo fmt --all --check` | Passed after formatting touched Rust files with rustfmt |
| `cargo clippy --workspace --all-targets --all-features -- -D warnings -D clippy::all -D clippy::pedantic -D clippy::nursery -D clippy::cargo` | Passed; no new lint suppressions |
| `cargo test --workspace --all-features` | Passed: 34 unit tests and 3 integration tests, zero failures/ignored tests |
| `cargo build --locked --release --workspace --all-features` | Passed, development-host release binary |
| `SDL_VIDEODRIVER=dummy cargo run --locked -- --smoke-test` | Passed, launch/reap/render recovery |
| System / confirmation / unavailable panel rendering | Passed at 480×272, 800×480, 1280×720 using SDL dummy; 480×272 and 800×480 frames visually inspected |

Tests cover percentage endpoints and all 101 brightness inputs, malformed/oversized/missing command output, failed commands and timeout, unsupported backlight range with no mutation, battery absence/full/charging/invalid gauge, Wi-Fi states, unequal audio channels, fixed power arguments and denial, mock control readback, queue guarding, stale status, a channel-blocked backend with a responsive client, no full re-probe on control, and confirmation cancellation/expiry/default selection. Existing app/catalog/lifecycle tests still pass. Test power operations use mocks only. Rendered device data is synthetic.

## Exact real-hardware results

**Not run: no device requested or accessed.**

| Required real-device check | Result |
| --- | --- |
| Battery percentage compared with Marshmallow | NOT RUN |
| Charging and external-power transitions / full battery | NOT RUN |
| Wi-Fi off/disconnected/connecting/connected | NOT RUN |
| Bluetooth behavior comparison | NOT RUN; static reference has no live backend |
| Brightness read/write and physical min/max | NOT RUN |
| Volume/readback/mute and audible min/max | NOT RUN |
| Status refresh with visible-stutter/frame-time observation | NOT RUN |
| Missing/malformed device data without crash | NOT RUN on device; automated fixtures passed |
| Reboot followed by normal boot | NOT RUN; no real power action issued |
| Shutdown, power-on and normal boot | NOT RUN; no real power action issued |
| Marshmallow still starts and remains selectable | NOT RUN on device; repository/session/configuration paths were not modified |
| Physical Home/Power/touch delivery and session focus | NOT RUN |

For later validation, launch Vitrallis manually within the existing session after closing apps. First confirm Marshmallow remains launchable, inspect installed utility versions and permissions, then compare status in both launchers on battery/external power. Exercise the entire brightness/volume range and unavailable-data cases without changing driver configuration. Measure visible responsiveness during status refresh. Finally exercise Cancel and one deliberate reboot/shutdown at a time, recording normal boot and Marshmallow availability after each. Do not change the default session as part of these checks.

## Remaining Marshmallow system behaviors not matched

- Battery ten-sample smoothing and two-second cadence; raw-reference “charging” labeling intentionally corrected.
- Wi-Fi scans, SSID/signal details, password prompts, connect/disconnect and radio enable/disable controls.
- Bluetooth fixture lists/pairing-like UI; no genuine reference hardware integration to port.
- Twelve-hour clock preference, date/timezone settings.
- Sleep/DPMS, lockscreen/password, FEL flashing-mode actions, advanced/login settings.
- The startup ALSA handle held open to mitigate touchscreen buzzing; hardware need and impact unverified.
- Continuous settings sliders (Vitrallis uses bounded steps) and any independent audio routing/mute features.
- Physical Home/Power/global shortcuts, WM focus integration, hardware permissions and tool-version verification.

Default-session replacement and Vitrallis Store are expressly excluded. ARM linking/runtime compatibility and all real-device results remain unverified.

## Files changed

- Added: `src/platform/system.rs`, `src/platform/command.rs`, `src/settings.rs`, `docs/system-status.md`.
- Updated: `src/platform/mod.rs`, `src/platform/generic.rs`, `src/platform/pocketchip.rs`, `src/input.rs`, `src/launcher.rs`, `src/lib.rs`, `src/renderer.rs`, `src/ui.rs`, `README.md`.
- `Cargo.toml`, `Cargo.lock`, startup scripts and existing configuration files are unchanged. Generated QA images are under ignored `target/system-qa*`.

`git diff --check` passed. New source and documentation files were additionally checked for trailing whitespace. Hardware command strings/sysfs paths remain confined to `src/platform/`; no production `unwrap()`, `expect()`, unsafe blocks or new lint allowances were added.
