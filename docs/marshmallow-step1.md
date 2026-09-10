# Marshmallow Step 1 integration audit

Only launcher/session, layout, input, assets, and app lifecycle code was inspected. Battery, networking, audio management, settings and other subsystems were not audited or implemented. Incidental code in the startup file was not ported.

The authoritative reference is [o-marshmallow/PocketCHIP-pocket-home at dccbd38dc233f3f45ebd6ea130d6a787268e23f5](https://github.com/o-marshmallow/PocketCHIP-pocket-home/tree/dccbd38dc233f3f45ebd6ea130d6a787268e23f5). Static source was cloned outside this repository; no reference implementation code or artwork is vendored. Its JUCE gitlink pins [630ab88f8bf11c8389f99a9ffb6daa3a0e891f14](https://github.com/juce-framework/JUCE/tree/630ab88f8bf11c8389f99a9ffb6daa3a0e891f14).

## Startup, window, session

- [`debian/desktop/pocket-home.desktop`](https://github.com/o-marshmallow/PocketCHIP-pocket-home/blob/dccbd38dc233f3f45ebd6ea130d6a787268e23f5/debian/desktop/pocket-home.desktop) specifies `Exec=pocket-home`. This is an application desktop entry; it does not establish the complete display-manager/session startup chain.
- [`Source/Main.cpp`](https://github.com/o-marshmallow/PocketCHIP-pocket-home/blob/dccbd38dc233f3f45ebd6ea130d6a787268e23f5/Source/Main.cpp), `initialise` and `MainWindow`: JUCE application, disallows another instance, loads config, creates a native-titled, resizable, centered visible window. Linux `setFullScreen(true)` and titlebar removal are commented out. Fullscreen behavior must not be inferred from those comments.
- [`Source/MainComponent.cpp`](https://github.com/o-marshmallow/PocketCHIP-pocket-home/blob/dccbd38dc233f3f45ebd6ea130d6a787268e23f5/Source/MainComponent.cpp) sets 480×272 content, resizes its page stack with its bounds, and hides the launch spinner when the main window becomes inactive.
- Build documentation and `debian/control` establish X11/JUCE dependencies. The README mentions restarting LightDM after installation, but no complete Awesome keybinding/session config is provided here. The application launch/refocus code explicitly invokes Awesome and xdotool. There is no SDL rendering in the inspected launcher path.

**Step 1:** manual SDL2 X11 fullscreen window on PocketCHIP; normal fixed-size desktop window in generic mode. Inherit an existing user's graphical session. Do not install a desktop entry or start/replace a WM. Multiple Vitrallis instances are not prevented across processes; each independently owns its child. Launch only one instance during later manual tests. Authentication/default-session replacement is outside scope.

## Grid, representation, assets

[`Source/AppsPageComponent.cpp`](https://github.com/o-marshmallow/PocketCHIP-pocket-home/blob/dccbd38dc233f3f45ebd6ea130d6a787268e23f5/Source/AppsPageComponent.cpp) uses `Grid(3, 2)`. JSON items contain `name`, `icon`, and `shell`; malformed field types are skipped. `AppIconButton` uses JUCE's image-above-text label representation. Missing/empty icons fall back to `appIcons/default.png`.

[`Source/Grid.cpp`](https://github.com/o-marshmallow/PocketCHIP-pocket-home/blob/dccbd38dc233f3f45ebd6ea130d6a787268e23f5/Source/Grid.cpp) uses proportional row/column sizing and page-local selection, with a bitmap selection rectangle containing fixed coordinates. Selection initially exists at index zero but is visually hidden until navigation. Previous/next selection stops at the outer edges. Up/down use a stride of three, left/right a stride of one, including movement across row boundaries.

[`Source/Utils.cpp`](https://github.com/o-marshmallow/PocketCHIP-pocket-home/blob/dccbd38dc233f3f45ebd6ea130d6a787268e23f5/Source/Utils.cpp) locates Linux assets in `/usr/share/pocket-home/`, then development `../../assets/`, then a current-directory-relative fallback. Writable config is under `~/.pocket-home/`. Image loading handles SVG or JUCE-supported images. [`Source/PokeLookAndFeel.cpp`](https://github.com/o-marshmallow/PocketCHIP-pocket-home/blob/dccbd38dc233f3f45ebd6ea130d6a787268e23f5/Source/PokeLookAndFeel.cpp) creates its typeface from embedded Lato-Regular data.

**Step 1:** six fixed apps; proportional 3×2 grid; visible focus from startup as requested; built-in fallback icons/font independent of installed PocketHome assets. Optional BMP icons are cached. No automatic config import, Lato/PNG/SVG parity, customization or pagination.

## Keyboard, pointer, Home/back

`AppsPageComponent::keyPressed` handles only arrows and Return for launcher selection/activation. `mouseUp` routes ordinary release to `buttonClicked` and app activation; control/right-click and drag behavior support editing, which is out of scope. Coordinates are JUCE component-local mouse coordinates; this code does not establish raw touchscreen calibration or device event mappings. `PageStackComponent` handles page transitions but does not establish a global physical Home binding.

**Step 1:** reproduce flat-grid arrow navigation and Return activation. Mouse release uses pixel hitboxes; SDL finger release uses normalized coordinates mapped into the same grid. Reject out-of-range/NaN coordinates, blank cells and synthesized duplicate touch mouse events. No raw evdev/sysfs access. Key repeat is ignored for deterministic single actions. There is no drag editing or multitouch gesture support. Escape and SDL Home clear idle status only; this is an explicit local convenience, not a claim about the PocketCHIP physical Home key. An app keeps running and blocks another launch if the WM brings Vitrallis forward.

## Launching, process lifetime, cwd/environment

`AppsPageComponent::startApp` starts `xmodmap ${HOME}/.Xmodmap`, then starts the app command with JUCE `ChildProcess`. It stores children and button mappings, debounces for two seconds, and shows a spinner. `startOrFocusApp` searches by command-derived window class with xdotool and uses `sh -c` with `awesome-client` to focus an existing window. It does not suspend a process in this code.

The periodic `runningCheckTimer.startTimer`/stop calls are commented out with a FIXME. `checkRunningApps` contains intended removal logic but cannot be treated as verified functioning periodic cleanup. Window inactivity clears the spinner separately.

The pinned JUCE [`modules/juce_core/native/juce_posix_SharedCode.h`](https://github.com/juce-framework/JUCE/blob/630ab88f8bf11c8389f99a9ffb6daa3a0e891f14/modules/juce_core/native/juce_posix_SharedCode.h), lines 1064–1208, tokenizes string commands into arguments, forks, and calls `execvp`; it does **not** invoke a shell implicitly. It inherits cwd/environment and uses pipes for selected output streams; `isRunning` uses `waitpid(WNOHANG)`. In particular, the `${HOME}` text in the xmodmap command is not shell-expanded by this path. The inspected app-start path does not set cwd, detach a session, or pause/resume children. Reference launch success reflects fork success, so it is not necessarily confirmation that exec succeeded.

**Step 1:** validated absolute executable plus distinct arguments via Rust `Command`; optional absolute cwd and otherwise inherited session environment/cwd. No xmodmap, shell interpolation, xdotool, Awesome commands, session mutation or process suspension. The PocketCHIP backend centralizes the reference's six names and argument lists from [`assets/config.json`](https://github.com/o-marshmallow/PocketCHIP-pocket-home/blob/dccbd38dc233f3f45ebd6ea130d6a787268e23f5/assets/config.json). Its `/usr/bin` executable locations are assumptions requiring later verification; the source lists command names only.

A state guard prevents activation in Launching/Running, rather than relying on a timed debounce. The shell retains ownership of exactly one direct child and polls its exit at most four times per second while otherwise idle. Exit is reaped, status logged, focus retained, and the launcher requests foreground focus. Spawn failure restores Ready without terminating the shell. A wait failure retains child ownership and launch blocking, with deduplicated logs. Child stdout/stderr are inherited so undrained pipes cannot stall the app. Orderly shell shutdown kills/reaps its direct child; descendants and daemonization are deferred.

These are deliberate Step 1 compatibility limits. The launcher does not claim running-app refocus parity, default-session safety, physical Home behavior, or live-device validation.
