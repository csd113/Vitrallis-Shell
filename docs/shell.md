# Shell behavior and development preview

The native registry supplies Terminal, Notepad and Files. App Center discovers
installed manifest packages; Linux handheld mode additionally reads the independent
PocketHome menu and provides System Settings and an Exit Vitrallis tile.
[Native apps](native-apps.md), [App Center](app-center.md),
[settings](devices/pocketchip/settings.md) and [updates](shell-updates.md) document
their respective controls and limits.

## Preview and discovery

Build all workspace binaries before running. Generic desktop mode defaults to
800×480; `--linux-handheld` selects the hardware backend, fullscreen and 480×272.
`--size WIDTHxHEIGHT` changes layout independently of hardware selection.
Keyboard/mouse/touch capabilities come from SDL events, not screen dimensions.

```sh
cargo run --locked -- --demo --size 480x272
SDL_VIDEODRIVER=dummy cargo run --locked -- --smoke-test
```

`--app-config` accepts an explicit PocketHome JSON path, and `--assets` an explicit
asset root. `--list-apps` prints normalized discovery diagnostics without a window.
`--screenshot NEW.bmp` captures the initial frame and exits, refusing to overwrite
an existing file. The README screenshot was captured from the current desktop
release build at 480×272 through this real renderer, with no device connection.

The handheld backend reads stock PocketHome's `/usr/share/pocket-home/config.json`.
`--app-config` overrides that path explicitly; no per-user launcher config is
assumed. `--assets` overrides the asset directory for exported fixtures. Missing or
malformed catalogs produce diagnostics; an explicit failed refresh preserves the
current catalog. All Apps pages contribute items in order. Entries use the OS
`name`, `icon`, and `shell` contract and JUCE command tokenization, without implicit
shell execution. No modified-launcher appearance preferences are imported.

Only verified stock utility command signatures in this importer are suppressed:
the exact terminal/editor/file-browser invocations documented in the
[upstream source audit](devices/pocketchip/stock-source.md). Renamed or localized
labels do not affect matching. Custom commands, arguments, PATH shadows, and
App Center packages are retained. Filtering runs on load and every refresh.
Native utilities remain present once even when missing; their repair diagnostic
is shown instead of substituting the stock app. Original menu files and packages
are never modified. Missing icons use a placeholder; retained textures are capped
at 16 MiB. SVG/JPEG and full font shaping are not supported.

## SDL renderer selection

The shell now defaults to hardware acceleration when SDL can initialize an
accelerated renderer. Its existing Canvas/Texture drawing path prefers SDL's
`opengles2` backend when advertised, then tries other advertised accelerated
backends in SDL order. Each backend is attempted with vsync, then without vsync
if initialization fails. SDL's resulting flags are checked before acceptance.
Auto falls back to a fresh SDL software canvas if all hardware attempts fail.
SDL remains responsible for presenting software window surfaces.

```sh
cargo run --locked -- --renderer auto
cargo run --locked -- --renderer hardware
cargo run --locked -- --renderer software
SDL_VIDEODRIVER=dummy cargo run --locked -- --renderer software --demo --size 480x272 --screenshot /tmp/vitrallis-new.bmp
```

`--renderer auto` is the default; `hardware` requires SDL acceleration and returns
an actionable startup error if unavailable; `software` skips hardware attempts.
These options use the existing CLI parser. Selection is independent of
`--linux-handheld`, window dimensions, and the SDL video driver. Each attempted
backend gets a fresh hidden window; only the accepted window is shown. Failed
hardware initialization cannot prevent software startup, provided SDL can create
a software window in the current display environment.

The minimum GPU API target remains **OpenGL ES 2.0**: PocketCHIP's Mali-400 with
upstream Mesa Lima; VideoCore IV with Mesa vc4 on Raspberry Pi 0/1/2/3/Zero/Zero 2
class devices; and VideoCore VI/VII with Mesa V3D on Pi 4/5. These are architectural
compatibility targets, **not newly validated device combinations**. GPU support
does not establish OS, CPU ABI, installer or system-control support: existing
Linux bundles target ARMv7/x86-64, so the ARMv6 Pi 0/1/Zero require separate build
and platform validation. No Vulkan, GLES3-only features, custom EGL/GLES, or
Wayland integration is introduced. A future SDL Wayland video backend can use
the same renderer; the current installed session still selects X11 independently.
Native Terminal/Notepad/Files retain their separate existing software UI renderer.

Inspect stderr (or the captured shell session log) for the single
`event=renderer_initialized` line. It records requested/actual mode, SDL renderer
name, acceleration/software/vsync flags, maximum texture dimensions, SDL video
driver, window/output/display dimensions, and `fallback=true hardware_error="..."`
when Auto rejected hardware. Unknown display dimensions are marked `unknown`;
zero maximum texture dimensions mean SDL did not provide a limit. Vsync describes
SDL's reported flag, not a measured refresh rate. These diagnostics do not identify
the physical GPU: Mesa software rasterizers can also sit behind an SDL accelerated
backend. The shell retains the typed `Launcher::renderer_info` snapshot, including
the full SDL capabilities, for future system/debug consumers without log parsing.

Dirty-frame rendering and the existing frame limiter are unchanged. Screenshots
read the completed backbuffer before presentation, as required by
[SDL's readback contract](https://wiki.libsdl.org/SDL2/SDL_RenderReadPixels), for both
software and accelerated canvases. `--demo`, `--smoke-test`, and `--screenshot`
use the selected renderer; deterministic CI can explicitly request software.

## Navigation and application lifecycle

Arrows traverse the 3×2 grid across pages. Page Up/Down and header arrows retain
the local slot where possible. Enter or a matching click/tap opens the selection.
Key repeats and duplicate synthesized touch/mouse events are ignored. Selection
survives app exit; Home returns to the grid while owned apps remain running.
Selecting a running tile resumes its window instead of spawning another process.
Window discovery requires process identity, and a missing window does not by itself
justify another launch. Startup failures show a dismissible error panel.

Each owned Unix child has a process group and is polled/reaped. Background exits
do not steal focus. Manual direct invocation cannot contain descendants that
escape that group after forced termination; the installed systemd
session owns its cgroup and restores Awesome's temporary bindings on exit.
Externally owned windows are not made safe to kill merely by matching a title.

App diagnostics can contain command arguments; keep secrets out of them.
Environment keys are logged, not their values. Status work runs off the UI thread
with bounded helper output and timeouts. Local catalog/image reads remain bounded
filesystem work on the UI thread; a stalled filesystem can still stall them.
