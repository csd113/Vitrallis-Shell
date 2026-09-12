> Original design reference, not the implemented application contract or a compatibility requirement. Use [application development](docs/app-development.md) and [AGENTS.md](AGENTS.md) for current engineering work.

# Vitrallis Project Reference

## Overview

**Vitrallis** is envisioned as a portable, lightweight software ecosystem for SBCs, embedded Linux devices, handheld computers, and future custom hardware.

The original project begins as a replacement for the Marshmallow PocketHome launcher on the PocketCHIP, but the architecture should be designed from the start so that Vitrallis can later support many different devices.

The core idea is:

- **Rust for the platform and graphical shell**
- **Python-first for apps and the app store ecosystem**
- **Hardware abstraction for portability across SBCs**
- **A simple app/package format that can later support more runtimes**
- **A polished app store and SDK so making software for Vitrallis is easy**

---

# Vitrallis Ecosystem

Vitrallis should be the umbrella brand rather than the name of only the launcher.

Possible components:

- **Vitrallis Shell** — graphical launcher and system shell
- **Vitrallis Store** — app marketplace
- **Vitrallis SDK** — developer tools and documentation
- **Vitrallis Runtime** — common APIs exposed to apps
- **Vitrallis Forge** — package/build/publishing tools
- **Vitrallis Hub** — developer/community portal
- **Vitrallis Platform** — hardware abstraction and device integration

Suggested GitHub organization:

```text
vitrallis
```

Suggested repositories:

```text
vitrallis-shell
vitrallis-runtime
vitrallis-sdk
vitrallis-store
vitrallis-forge
vitrallis-platform
```

The main launcher executable itself could simply be:

```text
vitrallis
```

---

# Core Technology Strategy

## Rust for Vitrallis Shell

The graphical shell should be written in Rust.

Rust is well suited to SBC hardware because it provides:

- Native compiled performance
- Low memory overhead
- Fast startup
- No garbage collector
- Strong memory safety
- Good cross-compilation support
- Excellent reliability for long-running system software
- Good separation between platform-specific and portable code

For old devices such as the PocketCHIP, efficiency matters significantly.

A purpose-built Rust renderer may also avoid much of the overhead associated with a general-purpose desktop GUI toolkit.

The shell should remain lightweight and event-driven rather than continuously rendering at high frame rates while idle.

---

# Python-First App Ecosystem

Vitrallis apps should initially be **Python-first**.

This gives the ecosystem a much lower barrier to entry than requiring every developer to write Rust.

Advantages:

- Easy to learn
- Easy to modify directly on a device
- Very fast app development
- Huge existing Python library ecosystem
- Excellent for hobbyist SBC projects
- Good fit for educational devices
- Easy for AI coding tools to generate and maintain
- Apps can often be inspected and modified without rebuilding them

The desired developer experience should eventually be close to:

> Write a Python app, add a manifest and icon, package it, and publish it to Vitrallis Store.

The important distinction is:

> **Python-first, not Python-only.**

The package system should be runtime-independent from the beginning so that Vitrallis can later distribute Rust, C, C++, Lua, web-based, or other native applications.

---

# Vitrallis App Structure

A simple Vitrallis Python application could look like:

```text
my-app/
├── app.toml
├── icon.png
├── main.py
├── requirements.txt
└── assets/
```

Example:

```text
dungeon-box/
├── app.toml
├── icon.png
├── main.py
├── requirements.txt
└── assets/
    ├── monsters/
    ├── tiles/
    └── sounds/
```

---

# App Manifest

A manifest should describe an application without requiring changes to the shell itself.

Example:

```toml
name = "Dungeon Box"
id = "io.vitrallis.dungeonbox"
version = "0.1.0"

runtime = "python"
entry = "main.py"

[permissions]
network = false
audio = true
storage = true
```

Possible future fields:

```toml
description = "An autonomous dungeon simulation."
author = "Example Developer"
license = "MIT"
minimum_vitrallis = "0.3.0"

[display]
fullscreen = true
orientation = "landscape"

[hardware]
touch = false
keyboard = true
audio = true
```

---

# Runtime-Agnostic Packages

The package specification should not assume Python internally.

Python:

```toml
runtime = "python"
entry = "main.py"
```

Native executable:

```toml
runtime = "native"
entry = "dungeon"
```

Potential future runtimes:

```text
python
native
rust
lua
web
wasm
```

This prevents the initial Python decision from restricting the future ecosystem.

---

# Python Vitrallis API

Vitrallis Runtime could expose a very simple Python API.

Example:

```python
import vitrallis

battery = vitrallis.system.battery_percent()

vitrallis.display.brightness(70)

vitrallis.notify("Dungeon complete")
```

Possible modules:

```text
vitrallis.system
vitrallis.display
vitrallis.input
vitrallis.audio
vitrallis.storage
vitrallis.network
vitrallis.notifications
vitrallis.power
vitrallis.device
```

The runtime API should hide hardware differences from application developers.

A Python app should ideally work on multiple Vitrallis devices without knowing which SBC it is running on.

---

# App Isolation and Dependencies

The Store should make installation simple for the user.

Possible approaches include:

- Per-app virtual environments
- A curated shared Python runtime
- Dependency caching
- Sandboxed app directories
- Explicit permissions
- Store-side compatibility validation

The user should not normally have to manually use `pip`.

A Vitrallis package should contain enough metadata for the Store to install and launch the application automatically.

---

# Portable Architecture

Vitrallis should not contain PocketCHIP-specific assumptions in the main UI code.

A conceptual architecture:

```text
Vitrallis
│
├── Shell
│   ├── Launcher
│   ├── Navigation
│   ├── Widgets
│   ├── Themes
│   └── App lifecycle
│
├── Runtime
│   ├── App APIs
│   ├── Permissions
│   └── Package integration
│
├── Renderer
│   ├── Text
│   ├── Sprites
│   ├── Animations
│   └── Scaling
│
├── Input
│   ├── Touch
│   ├── Keyboard
│   ├── Buttons
│   └── Controllers
│
└── Platform
    ├── PocketCHIP
    ├── Generic Linux
    ├── Raspberry Pi
    ├── Radxa
    └── Future custom hardware
```

---

# Hardware Abstraction

Hardware-specific operations should be hidden behind a common interface.

Conceptually:

```rust
trait Platform {
    fn battery_percent(&self) -> Option<u8>;
    fn set_brightness(&self, value: u8);
    fn wifi_enabled(&self) -> bool;
    fn set_wifi(&self, enabled: bool);
    fn shutdown(&self);
}
```

The shell should not care whether battery information comes from:

- `/sys`
- a power-management daemon
- I2C hardware
- a custom microcontroller
- another platform API

Only the platform backend should need to know.

Possible layout:

```text
platform/
├── generic_linux/
├── pocketchip/
├── raspberry_pi/
├── radxa/
└── custom/
```

---

# Screen and Resolution Independence

The original PocketCHIP uses a small fixed-resolution display, but Vitrallis should not be locked to it.

The UI should use:

- Logical coordinates
- Relative layout
- Configurable grid dimensions
- DPI scaling
- Integer scaling where appropriate
- Resolution-independent fonts
- Scalable icons
- Device-specific display profiles

Potential supported resolutions might include:

```text
480×272
800×480
1024×600
1280×720
1920×1080
```

A high-resolution device could still intentionally preserve a chunky retro PocketCHIP-style appearance using integer scaling.

---

# Rendering Philosophy

The shell should behave more like a tiny embedded UI/game engine than a desktop environment.

Priorities:

- Cache icons and text
- Avoid unnecessary allocations during rendering
- Redraw only when something changes
- Use dirty-region rendering where useful
- Allow low frame rates for simple animations
- Stop continuously rendering while idle
- Keep memory use predictable
- Optimize for responsiveness rather than visual complexity

SDL2 or another lightweight rendering backend could be used, but the renderer itself should ideally remain abstract enough to change later.

---

# Themes and Customization

Themes should be data-driven rather than compiled into Vitrallis.

Possible theme structure:

```text
themes/
└── crystal/
    ├── theme.toml
    ├── wallpaper.png
    ├── icons/
    ├── fonts/
    └── sounds/
```

Potential theme settings:

- Wallpaper
- App tile appearance
- Grid dimensions
- Fonts
- Font sizes
- Animation speed
- Status bar appearance
- Icon styles
- Highlight effects
- Sounds
- Transparency
- Crystal/facet visual effects

Vitrallis could eventually support downloadable themes through Vitrallis Store.

---

# App Discovery

Apps should not require recompiling Vitrallis Shell.

The shell can scan known directories such as:

```text
~/.local/share/vitrallis/apps/
```

An installed application might be:

```text
~/.local/share/vitrallis/apps/dungeon-box/
├── app.toml
├── icon.png
├── main.py
└── assets/
```

The launcher reads `app.toml` and automatically adds the application.

---

# Possible Future Launcher Features

After reaching PocketHome feature parity, Vitrallis can go significantly beyond it.

Ideas include:

- Multiple app pages
- App folders
- Reorderable apps
- Custom layouts
- Widgets
- Animated transitions
- Screensavers
- Art/display mode
- Clock widgets
- Weather widgets
- System monitor widgets
- Battery widgets
- Custom status bars
- Theme packs
- Custom keyboard shortcuts
- Device profiles
- Automatic app discovery
- Search
- Favorites
- Recently used apps
- App permissions
- Crash recovery
- Per-app logs
- Automatic updates
- Store integration

---

# Vitrallis Store

Vitrallis Store should eventually be one of the defining parts of the ecosystem.

Potential responsibilities:

- Discover applications
- Install applications
- Remove applications
- Update applications
- Resolve Python dependencies
- Verify package signatures
- Display compatibility information
- Manage screenshots and metadata
- Manage application permissions
- Support themes and widgets
- Support multiple CPU architectures
- Handle developer publishing

Eventually a developer could publish one app with architecture-independent Python source while Vitrallis handles the hardware platform underneath it.

---

# Package Format

Vitrallis should eventually define its own portable application package format.

The exact extension can be decided later.

A package could contain:

```text
manifest
application code/binary
icons
screenshots
assets
dependency metadata
permissions
hardware requirements
runtime requirements
signatures
```

The package format should remain independent of the application's programming language.

---

# Vitrallis Forge

A future **Vitrallis Forge** command-line tool could manage development and packaging.

Potential commands:

```text
vitrallis new
vitrallis run
vitrallis build
vitrallis package
vitrallis install
vitrallis publish
```

For example:

```text
vitrallis new dungeon-box
```

could create:

```text
dungeon-box/
├── app.toml
├── icon.png
├── main.py
└── assets/
```

This would make Vitrallis development approachable even for beginners.

---

# Development Roadmap

## Stage 1 — Bare-Bones Rust Launcher

Build the smallest possible Rust replacement.

Requirements:

- Start correctly on the PocketCHIP
- Fullscreen rendering
- Background
- Fixed app icons
- Keyboard input
- Touch input
- Launch one or more existing applications
- Return safely to the launcher when an application exits

Explicitly exclude:

- Wi-Fi
- Bluetooth
- Battery
- Settings
- Themes
- Store
- Complex app discovery

The purpose is simply to prove the Rust shell works on real hardware.

---

## Stage 2 — Real PocketHome Launcher Replacement

Replace hard-coded behavior with proper launcher functionality.

Add:

- Existing application discovery
- App metadata
- Icons
- Labels
- Grid layout
- Multiple pages
- Touch hitboxes
- Keyboard navigation
- Focus behavior
- Launch error handling
- Proper process management

At this point Vitrallis Shell should be usable as an everyday PocketCHIP launcher.

---

## Stage 3 — PocketCHIP System Integration

Port PocketCHIP-specific functionality while keeping it isolated behind the platform layer.

Add:

- Battery state
- Charging state
- Wi-Fi state
- Bluetooth state if needed
- Brightness
- Volume
- Clock
- Hardware buttons
- Reboot
- Shutdown

This stage will require the most real-device validation.

---

## Stage 4 — Full Marshmallow Behavioral Replacement

Use Marshmallow PocketHome as the authoritative behavioral reference.

Create a compatibility checklist and reproduce anything still missing.

Potential areas:

- Launcher behavior
- App pages
- Status indicators
- Power UI
- Personalization hooks
- Settings entry points
- Error handling
- Touch behavior
- Keyboard shortcuts
- Process lifecycle
- Session startup

Only after this stage should Vitrallis become the default launcher.

---

## Stage 5 — Portable Vitrallis Architecture

Once compatibility is established, remove remaining PocketCHIP assumptions.

Add:

- Logical screen dimensions
- Resolution scaling
- Platform traits
- Generic Linux backend
- Configurable app manifests
- Theme system
- Configurable layouts
- Runtime abstraction
- Device profiles

At this point the project becomes genuinely useful for SBCs other than the PocketCHIP.

---

## Stage 6 — Vitrallis Ecosystem

Build capabilities beyond Marshmallow:

- Vitrallis Runtime
- Python APIs
- App packaging
- Vitrallis Store
- Vitrallis SDK
- Vitrallis Forge
- Themes
- Widgets
- Permissions
- Package signing
- Updates
- Multiple SBC backends
- Developer publishing

This is the point where Vitrallis becomes a platform rather than merely a launcher.

---

# Using Marshmallow as the Reference Implementation

The existing Marshmallow PocketHome implementation should dramatically reduce the amount of trial-and-error hardware testing required.

For each feature:

1. Locate the Marshmallow implementation.
2. Identify inputs, outputs, paths, commands, and behavior.
3. Document the expected behavior.
4. Implement the equivalent Rust module.
5. Create automated tests where possible.
6. Test on real PocketCHIP hardware only where necessary.

Marshmallow should be treated as the **authoritative behavioral reference** during the compatibility stages.

Codex should not rediscover through trial and error behavior that is already clearly defined in the existing C++ source.

---

# Differential Testing

Where practical, Codex can compare Marshmallow and Vitrallis directly.

Examples:

- Battery state
- Wi-Fi status
- App discovery
- Application metadata
- Grid behavior
- Input handling
- Process launching
- System controls

Run both implementations on the same hardware and compare their outputs.

This can significantly reduce exploratory testing.

---

# Real-Hardware Testing

Many parts can be tested away from the PocketCHIP.

Hardware validation should primarily focus on:

- Touchscreen event coordinates
- Keyboard/input device behavior
- Actual rendering
- X11/framebuffer behavior
- Battery/sysfs data
- Brightness controls
- Wi-Fi
- Bluetooth
- Audio
- Suspend/reboot/shutdown
- Returning correctly from launched applications

Once hardware-specific behavior is behind platform interfaces, most future Vitrallis development can happen without constant physical-device testing.

---

# Safe Migration from Marshmallow

Marshmallow should remain installed throughout the early rewrite.

During development:

```text
Marshmallow PocketHome
Vitrallis Shell
```

should both remain available.

Vitrallis should initially be:

- launched manually, or
- selectable as a separate session

Only after compatibility testing should Vitrallis become the default.

This ensures a broken development build cannot make the PocketCHIP difficult to use.

---

# Long-Term Device Vision

Vitrallis should eventually be able to move from the original PocketCHIP onto a modern custom handheld with minimal shell changes.

Possible future hardware:

- Raspberry Pi Compute Module
- Radxa SBC
- Orange Pi
- Modern Allwinner device
- Custom ARM Linux SBC
- RISC-V SBC
- Purpose-built handheld motherboard

Ideally most changes for a new device would be limited to:

```text
platform/new_device/
```

rather than requiring changes throughout the launcher.

---

# Design Principle

The central Vitrallis philosophy should be:

> **A fast native foundation with an easy, open application layer.**

Rust provides the reliable and efficient foundation.

Python makes application development accessible.

The platform abstraction allows Vitrallis to survive beyond any one SBC.

The Store, SDK, Runtime, package format, and Forge tooling turn the launcher into an ecosystem.

---

# Current Direction Summary

```text
                    VITRALLIS
                        │
          ┌─────────────┼─────────────┐
          │             │             │
        Shell          Store          SDK
       (Rust)            │             │
          │              │          Forge
          │          App Packages
          │              │
          └─────── Vitrallis Runtime ───────┐
                         │                  │
                    Python Apps        Native Apps
                         │                  │
                         └────────┬─────────┘
                                  │
                         Platform Abstraction
                                  │
             ┌────────────────────┼────────────────────┐
             │                    │                    │
         PocketCHIP         Generic Linux        Future SBCs
```

The original PocketCHIP is the **first Vitrallis target**, not the definition of Vitrallis itself.
