# Vitrallis Shell

**0.1.0-beta.1 release candidate — not production-ready.** Portable SDL2 launcher
for embedded Linux, tested on a Debian 13 PocketCHIP with Marshmallow recovery.
See [current release validation](docs/release-candidate.md),
[toolchain/dependency policy](docs/dependencies.md),
[app developer quickstart](docs/app-development.md) and
[beta security model](docs/security.md). The original project reference is a
future design document; this beta does not yet ship an `app.toml` loader or Python SDK.

Vitrallis reads existing PocketHome/Marshmallow application metadata without converting apps or modifying Marshmallow. It runs as a separate SDL2 launcher, with an optional supervised PocketCHIP launch target that preserves Marshmallow as the normal boot default.

Build with Rust 1.91.1 (minimum 1.91), SDL2 development libraries, and pkg-config:

```sh
cargo build --locked
cargo run --locked
# Preview an exported PocketHome configuration and its assets on a desktop:
cargo run --locked -- --app-config /absolute/config.json --assets /absolute/assets --size 480x272
# Print normalized entries and discovery diagnostics without opening a window:
cargo run --locked -- --app-config /absolute/config.json --assets /absolute/assets --list-apps
# Explicit, self-contained test fixtures:
cargo run --locked -- --demo
SDL_VIDEODRIVER=dummy cargo run --locked -- --smoke-test
```

`--pocketchip` selects the PocketCHIP system backend, fullscreen and a 480×272 default. Default desktop size is 800×480. `--size WIDTHxHEIGHT` changes the proportional layout. `--screenshot NEW.bmp` writes the initial frame and exits, refusing to overwrite an existing file.

Discovery first reads `~/.pocket-home/config.json`. Only when it is absent does it read the default asset configuration. Asset lookup checks `/usr/share/pocket-home/`, then `../../assets/` relative to the launcher's working directory, then that working directory. Explicit `--app-config` and `--assets` override these locations. A broken user config is reported rather than silently replaced or merged with defaults. All `Apps` pages contribute their `items` in configured order. A configured `wifiCommand` provides the System Settings tile; its Wi-Fi control opens that existing connection manager. Runtime discovery never writes source metadata; the explicit installer backs up and adds only a Vitrallis shortcut.

Arrows traverse the ordered 3×2 grid across page boundaries. Page Up/Down and the header arrows change pages, retaining the local slot when possible and clamping on a partial last page. Enter opens the selection. Mouse and touch require a matching press/release target. Repeated keydown and synthesized touch mouse events are ignored. Selection and page survive app exit. Escape/Home clear idle feedback. The supervised PocketCHIP session temporarily routes the physical Home key through Awesome; Enter/tapping the selected running app resumes its window.

Home returns to the grid while owned apps keep running, marked with an asterisk. Selecting a running app resumes it; other tiles can launch another app. A reopen requested during process shutdown waits asynchronously for the old child to be reaped, then starts exactly one replacement. Window discovery retries for up to ten seconds; a missing window alone never triggers a duplicate process. Launching state blocks duplicate activation; a 400 ms input cooldown covers launch attempts. Returning focus or reaping the active app ends that cooldown so the first fresh tap is accepted. An exit discovered after the grid already regained focus does not discard a new touch unless the catalogue changed. Spawn failures show a dismissible error panel; Enter/tap dismisses before a retry. App output is inherited, and stderr logs discovery source, app identity, command, arguments, cwd, environment **keys**, PID, exit and cleanup diagnostics. Avoid placing secrets in command arguments. Unavailable apps retain their labels and icons with a warning mark; the catalogue refreshes on app exit when the grid is ready, so returning from Store reveals its installs. If Store exits in the background while another app remains active, refresh waits for a later app exit at the grid.

The launcher polls and reaps each owned child, retains selection, and requests foreground focus when the active app exits. Background app exits do not steal focus. Unix children receive their own process group. The PocketCHIP backend starts LXTerminal with `--no-remote` so Terminal and Wi-Fi windows do not share a server outside their own launch lifecycle. `/bin/kill` is an optional cleanup helper for remaining members of that group on direct-child exit or launcher shutdown; shutdown also kills/waits for the direct child. Close apps before closing Vitrallis to avoid losing their work. Apps reusing already-running external processes are outside direct-child ownership; the supervisor never claims ownership merely because a window has a matching title. The optional user systemd session supervises forced launcher termination and cleans the entire session cgroup, then restores Marshmallow. Manual direct invocation still lacks this outer supervisor.

`AppEntry` contains a stable ID, display name, icon and availability diagnostic. `AppManifest` contains optional runtime, absolute entry, arguments, cwd and per-child environment. An absent runtime executes the entry directly; a runtime receives the entry as its first argument. Existing PocketHome `name`/`icon`/`shell` entries need no changes. Optional `args`, `cwd`, and `env` fields are Vitrallis extensions. Discovery preserves the pinned JUCE command tokenizer's double-quote grouping and literal argument quotes; it does not silently interpret shell syntax. Use an explicit shell command only when intended by metadata.

`src/discovery/` is separate from platform window policy. `src/config.rs` centralizes filesystem conventions, including the reserved future `$XDG_DATA_HOME/vitrallis/apps` (fallback `~/.local/share/vitrallis/apps`) directory. App Center reuses the existing PocketCHIP updater and its PocketHome entries. See [App Center setup and safety](docs/store.md) and [installation, selection and recovery](docs/session.md).

PNG and bounded uncompressed BMP icons are decoded once per changed catalogue, retaining aspect ratio. The existing `background` color/PNG/BMP wallpaper, `showclock`, `timeformat` and `cursor` preferences are read without mutation. Missing icons use Marshmallow's default asset when available; broken/unsupported images use a built-in placeholder and log a warning. SVG/JPEG parity and full Unicode/font parity remain future work. No reference artwork is bundled. `serde_json` handles metadata and `png` handles the actual shipped icons. Retained icon textures are capped at 16 MiB; excess artwork uses placeholders. [Dependency exceptions](docs/dependencies.md) explain why the required strict Clippy policy currently prevents the newest PNG/compression versions.

See [current candidate validation](docs/release-candidate.md) for fresh evidence and remaining checks; the [Step 3 hardware compatibility report](docs/compatibility-step3.md) records earlier results. The [Step 2 report](docs/compatibility-step2.md) and earlier device notes are historical.

The System Settings tile and footer open the same settings screen. Brightness and volume use 10% steps for touch and left/right keypad input; up/down selects a control. Dragging updates the control live, coalesces pending changes, and filters small touch jitter. PocketCHIP brightness spans 10–100%, matching its ten lit hardware levels; volume spans 0–100%. Wi-Fi opens the configured connection manager and returns to settings on exit. Restart/power-off require a separate confirmation with Cancel selected initially. Escape/Home or the footer returns. F1 has no binding. SDL Power opens the panel when delivered to the application; the supervised Awesome session supplies the physical Home binding.

Choose **More** in System Settings for screen timeout (Never, 30 seconds, 1, 2, 5, 10, or 30 minutes), time-zone selection, and touchscreen calibration. The settings header shows the running build version. Time-zone changes use the device’s existing password authentication when required; calibration opens the installed PocketCHIP utility. See [settings controls and device validation](docs/settings-expansion.md) for persistence, recovery, and current limits.

App activation displays an “Opening…” panel until the app takes focus. Window discovery retries for up to 30 seconds; an app that remains alive without a window returns to the grid with a retry hint, retaining ownership and avoiding duplicate processes.

The status bar shows the Wi-Fi IPv4 address (USB IPv4 fallback), plus a filled battery icon and percentage, a charging/external-power bolt, a Wi-Fi connection icon, and local time in the configured 12/24-hour format. Low battery is amber; off/disconnected/unavailable Wi-Fi has a slash. System Settings uses embedded GPT Image artwork with transparent edges. Unavailable battery reads `--`. Bluetooth is omitted because this image has no validated backend. Desktop mode exposes time and no hardware controls. PocketCHIP reads the kernel AXP20x battery sysfs interface when present (legacy images can use optional `i2cget`), plus installed `nmcli`, `amixer`, `systemctl` and backlight sysfs. It does not install hardware packages or permissions. All hardware work runs off the UI thread, full refresh is ten seconds after the preceding refresh, and commands have bounded output and a two-second timeout. See [system audit and validation](docs/system-status.md) for exact mechanisms, assumptions, test evidence, and remaining parity gaps.

## Development and release checks

Install SDL2 development libraries and pkg-config on the development host. Rustup
selects the checked-in Rust 1.91.1 toolchain automatically. Run:

```sh
sh scripts/validate.sh
# Additional release dependency checks (optional host tools):
cargo audit
cargo outdated --workspace
cargo tree --duplicates
```

The validation script runs strict Clippy, workspace and Python tests, release
build, SDL smoke test and diff checks. See [ARM setup](docs/session.md) for the
separate image-matched cross build. Keep `Cargo.lock`; application release builds
use `--locked`. No global Python dependency installation is necessary.

The renderer, input and catalogue state are independent of platform controls.
`platform::System` supplies typed status/control operations and `Platform` supplies
window and process-launch policy. Add a new SBC backend there and select it at configuration entry;
first supply missing-data behavior, mock tests, actual resolution/ABI validation,
and a reversible session adapter. PocketCHIP's Awesome/Tk focus integration is
specific to that session. A public SDK and general window identity protocol are
future work, not an existing abstraction to depend on.
