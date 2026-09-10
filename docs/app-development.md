# Application and package development

## What this beta implements

Vitrallis launches existing PocketHome/Marshmallow applications. It reads JSON
menu entries, normalizes them to an internal Rust `AppManifest`, and launches
explicit argument vectors. The reserved XDG application directory is not yet a
second package catalogue. **There is no `app.toml` loader, public Python SDK,
SDK generator or generic `.vapp` archive installer in this repository.**
`Vitrallis_Project_Reference.md` is a design reference, not a list of delivered
features. Adding one of those formats requires a later versioned specification
and loader tests; a file called `app.toml` alone will not install an app.

## Quickstart using today's format

Keep application code/data in your normal user directory and add an item to an
existing `Apps` page in `~/.pocket-home/config.json`, preserving other pages and
preferences. Back up the document before editing. For example:

```json
{
  "name": "My Python App",
  "icon": "/home/chip/.local/share/my-app/icon.png",
  "shell": "/usr/bin/python3",
  "args": ["/home/chip/.local/share/my-app/main.py"],
  "cwd": "/home/chip/.local/share/my-app",
  "env": {"MY_APP_MODE": "normal"}
}
```

Substitute the actual home/path for your target; `/home/chip` is only an example.
The `args`, `cwd`, and `env` fields are Vitrallis extensions. To launch from both
Marshmallow and Vitrallis, use a reviewed executable wrapper as the `shell`
command instead. Its contents can be as simple as:

```sh
#!/bin/sh
set -eu
exec /usr/bin/python3 "$HOME/.local/share/my-app/main.py" "$@"
```

Choose an existing compatible Python/Tk runtime or an app-local virtual
environment; do not assume system Python contains Tk. Pin and test any
application-specific pip dependencies in that application's environment. Do not
run global pip, apt upgrades, downloaded shell fragments, or Rust installation
as a side effect of opening a Python app. The existing Bitcoin/Store wrappers
show how the tested image selects its already installed private Tk libraries.

A minimal Python/Tk application can use:

```python
import tkinter as tk

window = tk.Tk()
window.title("My Python App")
tk.Label(window, text="Hello from Vitrallis").pack(expand=True)
window.bind("<Escape>", lambda event: window.destroy())
window.mainloop()
```

This uses Tk directly, not a Vitrallis SDK. Public hardware APIs are future work;
do not copy PocketCHIP sysfs/I2C paths into portable apps. Request an explicit,
versioned hardware API before developing applications that need those controls.
The current Bitcoin display and Store do not directly access PocketCHIP hardware.
Native apps should expose `_NET_WM_PID` for process-group window resume. On the
inspected Tk image this property is missing, so only the two reviewed Bitcoin
and Store wrappers have the narrow class/title fallback; arbitrary Tk apps may
need the window manager to resume until a general identity protocol is added.

Preview and validate the menu without deploying:

```sh
cargo run --locked -- --app-config /absolute/config.json --assets /absolute/assets --list-apps
cargo run --locked -- --app-config /absolute/config.json --assets /absolute/assets --size 480x272
```

## Current manifest reference

| Field | Meaning and constraints |
| --- | --- |
| `name` | Nonempty visible label, no control characters |
| `shell` | Command string parsed with the existing JUCE-compatible tokenizer; executable must resolve to a regular executable file |
| `icon` | Required string; empty uses the default asset. Absolute paths or paths relative to asset roots. PNG/BMP only, bounded to 1 MiB and 512×512 |
| `args` | Optional array of strings appended literally, preserving spaces/empty strings; NUL rejected |
| `cwd` | Optional absolute working directory |
| `env` | Optional string map for this child only; invalid keys/NUL rejected |

Double quotes group tokens in `shell`; argument quotes remain literal to match
Marshmallow. Single quotes/backslashes are not shell escape syntax. Prefer
`args` for precise vectors; do not expect `$HOME`, pipes or command substitution
to expand. Explicit shell interpreters in locally reviewed metadata execute
with the user's privileges and are part of the beta trust model.

The internal Rust manifest also supports an optional runtime and absolute entry;
that does not mean those fields are accepted by a TOML loader. Stable catalogue
IDs derive from existing names/commands, with occurrence suffixes for intentional
duplicate menu entries. Selecting the same running tile resumes its owned
process instead of launching again. Renaming its command/name changes identity.

## Native Rust package baseline

A future member of this workspace should use:

```toml
[package]
name = "my-vitrallis-app"
version.workspace = true
edition.workspace = true
rust-version.workspace = true

[dependencies]
# Only inherit the workspace dependencies the application actually uses.
```

Add its directory to `workspace.members` deliberately. Standalone native
packages should declare `edition = "2024"` and `rust-version = "1.91"`, pin their
build toolchain, and retain a reviewed lockfile for application releases. Cargo
then rejects unsupported compilers with a clear MSRV error. Runtime formats and
app metadata do not require ARMv7; the target belongs in build/release metadata.

## Package build/install workflow

Build the launcher with `sh scripts/build-pocketchip.sh` using the target image's
SDL/sysroot and install with the reviewed user installer described in
[PocketCHIP installation and recovery](devices/pocketchip.md). This installer
handles the launcher/session, not arbitrary packages.

For application distribution, follow [Store adapters and integrity](devices/pocketchip/store.md).
The current Store has a fixed reviewed catalogue and per-application installer;
adding a row alone does not supply a safe generic package format. Extend its
adapter and tests together. Preserve user data, explicit argv, source hashes,
size/type checks, backups, and incomplete-install repair. Archive extraction,
signed repositories, uninstall, runtime provisioning and an `app.toml` schema
are future milestones, not beta capabilities.
