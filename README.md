# Vitrallis Shell

Step 1: a small native Rust launcher with a six-app grid, keyboard/touch-coordinate input, and a single managed child process. The desktop catalog contains safe demo processes. The PocketCHIP catalog contains fixed command definitions derived from Marshmallow; it is not a replacement session yet.

No USB connection, SSH, live-device probing, deployment, or startup changes were performed for this implementation.

## Build and exercise on a development host

Use Rust 1.85+ and a C linker, with SDL2 development libraries and `pkg-config` installed. The implementation was validated with Rust 1.98.1 and SDL2 2.32.72 on macOS. The declared Rust minimum is not independently tested.

```sh
# macOS development host, if dependencies are missing:
brew install sdl2 pkg-config

# Debian/Ubuntu development host, if dependencies are missing:
sudo apt-get install build-essential pkg-config libsdl2-dev

cargo build --locked
cargo run --locked -- --size 480x272
cargo run --locked -- --size 800x480
```

Run the installation command appropriate to your development host only. The SDL binding uses `pkg-config`; if a nonstandard prefix is used, configure its `PKG_CONFIG_PATH`. See the [Rust-SDL2 build instructions](https://github.com/Rust-SDL2/rust-sdl2#requirements).

- Arrows move focus. Left/right traverse the flat grid, including row boundaries; outer edges stop.
- Enter or a left-button/touch release opens an app. Keyboard repeats are ignored.
- Escape/Home clear launcher status when idle. They do not quit the shell, kill an app, or intercept the device's global Home button.
- Close the desktop window to quit. **Quitting the shell while its child is still running kills and reaps that direct child.** Close applications first to avoid losing their work.
- Demo App, Notes Demo, and Tools Demo wait two seconds and exit. Quick Return exits immediately. Exit Error returns a failure status. Missing App demonstrates spawn failure and retry.
- Only one direct child runs at once. The launcher remains visible; another app's window can take focus. On child exit, the launcher requests focus and retains its selected tile.

Automated execution without a display server:

```sh
cargo test --workspace --all-features
SDL_VIDEODRIVER=dummy cargo run --locked -- --smoke-test
SDL_VIDEODRIVER=dummy cargo run --locked -- --size 480x272 --screenshot /tmp/vitrallis-preview.bmp
```

`--smoke-test` injects Enter through SDL, launches the built-in demo child, checks its successful exit, renders the recovered launcher, and terminates within a ten-second test deadline. `--screenshot` saves the initial rendered frame and exits without launching an app; the destination must not exist. The normal desktop window can also run `--smoke-test`. SDL must be installed for compilation and tests, but unit tests do not initialize a display.

## Architecture and dependencies

| File | Responsibility |
| --- | --- |
| `src/main.rs`, `src/lib.rs` | Small executable entry, CLI dispatch, self-contained demo child |
| `src/app.rs` | Runtime-independent validated app id, label, icon, executable, argument list, cwd |
| `src/launcher.rs` | Ready → Launching → Running → Ready, selection and feedback |
| `src/navigation.rs` | Renderer-independent focus transitions |
| `src/layout.rs`, `src/config.rs` | Proportional grid, hitboxes, text/icon dimensions, bounded CLI configuration |
| `src/input.rs` | Actions and SDL keyboard/mouse/finger translation |
| `src/renderer.rs` | Software drawing, bitmap text, bounded BMP loading/cache, fallback icons, screenshots |
| `src/ui.rs` | SDL window, dirty rendering, event waits and lifecycle coordination |
| `src/process.rs` | Structured command construction, process interface, child ownership/reaping |
| `src/platform/` | Generic desktop, test mock, and PocketCHIP app/session profiles |

The only public library entry is `run`; internal modules are private. `Platform` separates catalogs, display defaults, fullscreen, and post-exit focus policy. `Processes` separates spawning from launcher transitions. Neither the renderer nor the launcher imports PocketCHIP commands or paths.

Two direct crates are used: `sdl2` 0.38 and `font8x8` 0.3.1. SDL supplies native windows, event waits, and a software canvas suitable for an X11 embedded session without depending on GPU acceleration. The small compiled bitmap font avoids SDL_ttf, a font installation, and a font rasterizer. Cargo.lock pins nine direct/transitive dependency packages. There is no async runtime, application discovery library, JSON stack, SDL_image, audio initialization, or unsafe Rust in owned code. SDL and its bindings contain native/unsafe implementation code.

The renderer blocks on events while idle. It calls nonblocking child checks after events or a 250 ms wait while an app runs, and only redraws when state or window exposure changes. It does not target 60 FPS. Asset textures are loaded once; fallback icons and font glyphs require no filesystem assets. Layout is calculated once for the fixed window size (or actual fullscreen desktop size), with centralized proportional margins, gaps, icon bounds and text scale. Other resolutions, including 1024×600 and 1280×720, use the same code. Live resizing and pagination are intentionally absent; oversized catalogs are rejected instead of hiding apps.

Step 1 accepts optional absolute BMP icon paths in app definitions, restricted to uncompressed 24/32-bit Windows BMP, 1–512 pixels per dimension and at most 1 MiB. Missing, unreadable, or invalid icons fall back visibly and log once. Default catalogs use built-in placeholder icons. Labels use the font's basic character set; unsupported characters render as `?`, and long labels/status are clipped to their allotted width. Full launch diagnostics are on stderr. Marshmallow artwork and fonts are not copied.

Commands use absolute executables and separate OS-string arguments, with no shell expansion. No process-wide cwd, environment, keymap, or session changes occur. Children inherit the launcher's environment and cwd unless an app specifies an absolute cwd; stdin is closed, stdout/stderr inherited. This is a launcher, not a sandbox. Apps must stay in the foreground of their direct child process: daemonizing apps and their descendants are not tracked. Signal-driven/forced shell termination is not a supervised session lifecycle.

Read [the targeted reference audit](docs/marshmallow-step1.md) for compatibility decisions, [manual device build/launch guidance](docs/device-validation.md), and [validation results](docs/validation.md). Work beyond Step 1 is intentionally deferred.
