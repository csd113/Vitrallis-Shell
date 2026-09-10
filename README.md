# Vitrallis Shell

Vitrallis reads existing PocketHome/Marshmallow application metadata without converting apps or modifying Marshmallow. It runs as a separate SDL2 launcher, with an optional supervised PocketCHIP launch target that preserves Marshmallow as the normal boot default.

Build with Rust 1.85+, SDL2 development libraries, and pkg-config:

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

Discovery first reads `~/.pocket-home/config.json`. Only when it is absent does it read the default asset configuration. Asset lookup checks `/usr/share/pocket-home/`, then `../../assets/` relative to the launcher's working directory, then that working directory. Explicit `--app-config` and `--assets` override these locations. A broken user config is reported rather than silently replaced or merged with defaults. All `Apps` pages contribute their `items` in configured order. A configured `wifiCommand` provides a Wi-Fi Settings entry. Runtime discovery never writes source metadata; the explicit installer backs up and adds only a Vitrallis shortcut.

Arrows traverse the ordered 3×2 grid across page boundaries. Page Up/Down and the header arrows change pages, retaining the local slot when possible and clamping on a partial last page. Enter opens the selection. Mouse and touch require a matching press/release target. Repeated keydown and synthesized touch mouse events are ignored. Selection and page survive app exit. Escape/Home clear idle feedback. The supervised PocketCHIP session temporarily routes the physical Home key through Awesome; Enter/tapping the selected running app resumes its window.

Home returns to the grid while owned apps keep running, marked with an asterisk. Selecting a running app resumes it; other tiles can launch another app. Launching state blocks duplicate activation; a 400 ms input cooldown covers launch attempts and return. Spawn failures show a dismissible error panel; Enter/tap dismisses before a retry. App output is inherited, and stderr logs discovery source, app identity, command, arguments, cwd, environment **keys**, PID, exit and cleanup diagnostics. Avoid placing secrets in command arguments. Unavailable apps retain their labels and icons with a warning mark; the catalogue refreshes after each app closes, so Store installs appear immediately.

The launcher polls and reaps each owned child, retains selection, and requests foreground focus when the active app exits. Background app exits do not steal focus. Unix children receive their own process group. `/bin/kill` is an optional cleanup helper for remaining members of that group on direct-child exit or launcher shutdown; shutdown also kills/waits for the direct child. Close apps before closing Vitrallis to avoid losing their work. Apps reusing already-running external processes are outside direct-child ownership; the supervisor never claims ownership merely because a window has a matching title. The optional user systemd session supervises forced launcher termination and cleans the entire session cgroup, then restores Marshmallow. Manual direct invocation still lacks this outer supervisor.

`AppEntry` contains a stable ID, display name, icon and availability diagnostic. `AppManifest` contains optional runtime, absolute entry, arguments, cwd and per-child environment. An absent runtime executes the entry directly; a runtime receives the entry as its first argument. Existing PocketHome `name`/`icon`/`shell` entries need no changes. Optional `args`, `cwd`, and `env` fields are Vitrallis extensions. Discovery preserves the pinned JUCE command tokenizer's double-quote grouping and literal argument quotes; it does not silently interpret shell syntax. Use an explicit shell command only when intended by metadata.

`src/discovery/` is separate from platform window policy. `src/config.rs` centralizes filesystem conventions, including the reserved future `$XDG_DATA_HOME/vitrallis/apps` (fallback `~/.local/share/vitrallis/apps`) directory. Store reuses the existing PocketCHIP updater and its PocketHome entries. See [Store setup and safety](docs/store.md) and [installation, selection and recovery](docs/session.md).

PNG and bounded uncompressed BMP icons are decoded once per changed catalogue, retaining aspect ratio. The existing `background` color/PNG/BMP wallpaper, `showclock`, `timeformat` and `cursor` preferences are read without mutation. Missing icons use Marshmallow's default asset when available; broken/unsupported images use a built-in placeholder and log a warning. SVG/JPEG parity and full Unicode/font parity remain future work. No reference artwork is bundled. `serde_json` handles metadata and `png` handles the actual shipped icons; the lockfile keeps a single compression implementation version compatible with the strict Clippy checks.

See the current [Step 3 hardware compatibility report](docs/compatibility-step3.md) for evidence and outstanding checks. The [Step 2 report](docs/compatibility-step2.md) and earlier device notes are historical.

F1 or a tap on the footer opens the system panel. Arrows select, Enter/tap activates, and Escape/Home/F1 or the footer returns. Brightness and volume have bounded steps; reboot/shutdown require a separate confirmation with Cancel selected initially. SDL Power opens the panel when delivered to the application; the supervised Awesome session supplies the physical Home binding.

The status line shows battery percentage (`B`), charging (`C`), usable external power (`P`), Wi-Fi connection (`W`) and local time in the existing configured 12/24-hour format. `?`/`--` means unavailable; Bluetooth is unavailable because the reference only supplies a fixture UI. Desktop mode exposes time and no hardware controls. PocketCHIP reads the kernel AXP20x battery sysfs interface when present (legacy images can use optional `i2cget`), plus installed `nmcli`, `amixer`, `systemctl` and backlight sysfs. It does not install hardware packages or permissions. All hardware work runs off the UI thread, full refresh is ten seconds after the preceding refresh, and commands have bounded output and a two-second timeout. See [system audit and validation](docs/system-status.md) for exact mechanisms, assumptions, test evidence, and remaining parity gaps.
