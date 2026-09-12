# Shell behavior and development preview

The native registry supplies Terminal, Notepad and Files. App Center discovers
installed manifest packages; PocketCHIP mode additionally reads the independent
PocketHome menu and provides System Settings and Marshmallow recovery.
[Native apps](native-apps.md), [App Center](app-center.md),
[settings](devices/pocketchip/settings.md) and [updates](shell-updates.md) document
their respective controls and limits.

## Preview and discovery

Build all workspace binaries before running. Generic desktop mode defaults to
800×480; `--pocketchip` selects the hardware backend, fullscreen and 480×272.
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

PocketCHIP first reads `~/.pocket-home/config.json`; only an absent user file permits
its default asset configuration. Asset lookup checks `/usr/share/pocket-home/`,
then `../../assets/` relative to the working directory, then that working directory.
Explicit paths override these choices. Malformed user metadata is reported instead
of silently merged with defaults. All Apps pages contribute items in order.
Device-menu items use the OS `name`, `icon`, and `shell` contract. Their command
parser preserves the pinned JUCE tokenization; it does not implicitly execute a
shell. Use an explicit shell entry only when that is intended.

Existing background color/PNG/BMP wallpaper, clock visibility, 12/24-hour time and
cursor preferences are read without runtime mutation. Missing icons use the OS
fallback when available; unsupported artwork uses a visible placeholder. Retained
icon textures are capped at 16 MiB. SVG/JPEG and full font shaping are not supported.

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
escape that group after forced termination; the installed PocketCHIP systemd
session owns its cgroup and restores Awesome's temporary bindings on exit.
Externally owned windows are not made safe to kill merely by matching a title.

App diagnostics can contain command arguments; keep secrets out of them.
Environment keys are logged, not their values. Status work runs off the UI thread
with bounded helper output and timeouts. Local catalog/image reads remain bounded
filesystem work on the UI thread; a stalled filesystem can still stall them.
