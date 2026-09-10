# Vitrallis Shell

Vitrallis reads existing PocketHome/Marshmallow application metadata without converting apps or modifying Marshmallow. It runs as a separate SDL2 launcher; it does not replace the login session.

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

`--pocketchip` selects fullscreen and a 480×272 default. Default desktop size is 800×480. `--size WIDTHxHEIGHT` changes the proportional layout. `--screenshot NEW.bmp` writes the initial frame and exits, refusing to overwrite an existing file.

Discovery first reads `~/.pocket-home/config.json`. Only when it is absent does it read the default asset configuration. Asset lookup checks `/usr/share/pocket-home/`, then `../../assets/` relative to the launcher's working directory, then that working directory. Explicit `--app-config` and `--assets` override these locations. A broken user config is reported rather than silently replaced or merged with defaults. All `Apps` pages contribute their `items` in configured order; settings/library entries are excluded. Source metadata is never written.

Arrows traverse the ordered 3×2 grid across page boundaries. Page Up/Down and the header arrows change pages, retaining the local slot when possible and clamping on a partial last page. Enter opens the selection. Mouse and touch require a matching press/release target. Repeated keydown and synthesized touch mouse events are ignored. Selection and page survive app exit. Escape/Home clear idle feedback; they do not implement the hardware Home key or session switching.

One app runs at a time. Launching/running state blocks duplicate activation; a 400 ms input cooldown covers launch attempts and return. Spawn failures show a dismissible error panel; Enter/tap dismisses before a retry. App output is inherited, and stderr logs discovery source, app identity, command, arguments, cwd, environment **keys**, PID, exit and cleanup diagnostics. Avoid placing secrets in command arguments. Unavailable apps retain their labels and icons with a warning mark; restart discovery after installing or repairing a missing executable.

The launcher polls and reaps its direct child, retains focus selection, and requests foreground focus on exit. Unix children receive their own process group. `/bin/kill` is an optional cleanup helper for remaining members of that group on direct-child exit or launcher shutdown; shutdown also kills/waits for the direct child. Close apps before closing Vitrallis to avoid losing their work. Apps that daemonize into another session or reuse an already-running external process require later window/session supervision. Forced launcher termination is not supervised.

`AppEntry` contains a stable ID, display name, icon and availability diagnostic. `AppManifest` contains optional runtime, absolute entry, arguments, cwd and per-child environment. An absent runtime executes the entry directly; a runtime receives the entry as its first argument. Existing PocketHome `name`/`icon`/`shell` entries need no changes. Optional `args`, `cwd`, and `env` fields are Vitrallis extensions. Discovery preserves the pinned JUCE command tokenizer's double-quote grouping and literal argument quotes; it does not silently interpret shell syntax. Use an explicit shell command only when intended by metadata.

`src/discovery/` is separate from platform window policy. `src/config.rs` centralizes filesystem conventions, including the reserved future `$XDG_DATA_HOME/vitrallis/apps` (fallback `~/.local/share/vitrallis/apps`) directory. Native package discovery, Store, installer, system settings and status features are not implemented.

PNG and bounded uncompressed BMP icons are decoded once, retaining aspect ratio. Missing icons use Marshmallow's default asset when available; broken/unsupported images use a built-in placeholder and log a warning. SVG/JPEG parity and full Unicode/font parity remain future work. No reference artwork is bundled. `serde_json` handles metadata and `png` handles the actual shipped icons; the lockfile keeps a single compression implementation version compatible with the strict Clippy checks.

See [the Step 2 compatibility report](docs/compatibility-step2.md) for evidence and remaining differences, and [device validation](docs/device-validation.md) for deferred hardware checks.
