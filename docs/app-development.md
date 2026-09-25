# Application and package development

Follow the [project engineering policy](../CONTRIBUTING.md). The current Vitrallis
third-party package contract is catalog v1 plus `app.toml` manifest v1 from
[Vitrallis Apps](https://github.com/csd113/Vitrallis-Apps/blob/main/docs/creating-apps.md).
App Center parses this contract directly in Rust; there is no public Python SDK.

## Bundled native applications

The Shell workspace owns `apps/terminal`, `apps/notepad`, and `apps/files`, plus
`crates/vitrallis-native`. These are native binaries, not manifest-v1 packages.
The fixed native registry supplies stable IDs, same-generation executables,
original embedded icons and diagnostics. The shell's existing process owner
handles launch, resume, delivery and reaping. Build all members with
`cargo build --workspace --locked`; plain `cargo run` still starts the shell.
See [native architecture and controls](native-apps.md) before extending a bundled
utility. The `apps/<slug>` package instructions below refer to the separate
Vitrallis Apps repository, not these Rust workspace directories.

## Package layout

Create `apps/<app-slug>/` using a lowercase slug with single hyphens. Include
`app.toml`, `main.py`, `icon.png`, `requirements.txt`, `README.md`, populated
`assets/`, and meaningful app-local `tests/`. Publication excludes `tests/` and
includes every other committed file. An alternate Python entry is permitted;
`main.py` is still required. Files resolve relative to the installed package,
independently of the shell's working directory.

```toml
manifest_version = 1
name = "Example App"
id = "org.example.myapp"
version = "0.1.0"
runtime = "python"
entry = "main.py"

[permissions]
network = false
audio = false
storage = false
```

For Python, these seven top-level fields and three boolean permissions are the complete v1
vocabulary. Rust packages use the alternative below. Unknown or duplicate fields are errors. IDs are lowercase reverse-domain
identifiers; versions are stable numeric `MAJOR.MINOR.PATCH`. Permissions declare
requirements, not a sandbox or a grant of authority. Package icons are noninterlaced
PNGs of 1–512 pixels per dimension. App Center validates their decoded contents.

Use the upstream [publishing workflow](https://github.com/csd113/Vitrallis-Apps/blob/main/docs/publishing-apps.md)
to pin a source commit and generate the complete sorted inventory, byte sizes and
SHA-256 values. Catalog and manifest metadata must agree. Source paths come from
the catalog and must meet `apps/<app-slug>`; the shell never guesses directories.
Keep `installable` false until the app's target runtime and client integration
have been verified. Existing packages follow the same workflow as new ones.

## Precompiled Rust packages

Native Rust apps use `runtime = "rust"` and a `binaries` table instead of `entry`.
The catalog has the same `runtime` and `binaries` values. No Python supervisor,
`main.py`, `requirements.txt`, Cargo or compiler is required on the target.

```toml
manifest_version = 1
name = "Native Example"
id = "org.example.nativeapp"
version = "0.1.0"
runtime = "rust"

[binaries]
armv7-unknown-linux-gnueabihf = "bin/armv7/app"
aarch64-unknown-linux-gnu = "bin/aarch64/app"
x86_64-unknown-linux-gnu = "bin/x86_64/app"

[permissions]
network = false
audio = false
storage = false
```

Include only targets you actually build. Each executable must be in the sorted
package inventory. App Center selects the exact host OS/architecture/ABI, checks
ELF class and machine (and ARM EABI5 hard-float), and refuses an incompatible
package before execution. The publisher must also build for the target's libc
baseline and document dynamic library prerequisites; an ELF header does not prove
that all runtime libraries are available. PocketCHIP builds use ARMv7 hard-float,
Cortex-A8 and a compatible GNU libc baseline (the Shell build uses 2.36).

`app.toml`, `icon.png`, `README.md` and populated `assets/` remain required.
The current limits are 2 MiB per file, 16 MiB per package and 256 files.
Strip release executables. Declared binaries are installed executable (0755),
regardless of the source file mode. Generated launchers exec the selected binary
directly. Updates and removals use the same receipt-scoped transactions and
local-edit protection as Python apps. Native process checks use the user's exact
`/proc/<pid>/exe` and process start identity on Linux, rather than a process name.

Folder membership is user state, outside the package manifest and receipt. Apps
must not modify it or assume a fixed folder. Uninstalling an app does not delete
its saved folder preference, and deleting a folder never uninstalls an app.

## Installation, launch and data

Configure the catalog in App Center and explicitly trust any separate source
repository. Refresh fetches metadata; selecting Install acquires, verifies, and
stages that app. Packages install under `$XDG_DATA_HOME/vitrallis/apps/<id>`
(default `~/.local/share/vitrallis/apps/<id>`). Manifests supply identity, name,
entry and permissions; receipts bind installed versions and file ownership to the
publisher. Locally generated launchers live in the App Center state directory.
Unmanaged files never become owned merely because Python source contains a version.

Declare Python dependencies in `requirements.txt`; App Center installs missing
distributions in an app-local environment. It can also use a compatible system
Python or existing `.venv`. The shell does not install dependencies globally,
import app code during validation, or reuse another application's private runtime. Declare Tk and
other system prerequisites in the README. App code must not launch a window,
perform network requests or write files when imported. Keep writes in documented
app-private data paths, respect storage requirements, and treat package files as
read-only. See [App Center safety and recovery](app-center.md).

Apps should publish `_NET_WM_PID` for process-group window resume on the target device.
No app name or window-title matching is used. Tk builds that omit this property
need the window manager to resume; test that behavior on the target image.
Keyboard and touch activation, close/return, and display geometry must be tested
on the actual target. Desktop validation alone does not certify the target device.

## Device integration

The read-only PocketHome importer exposes existing OS applications while suppressing
verified stock equivalents of the three native utilities. It is a boundary with the independent device
image, not the Vitrallis package API. `--app-config` and `--assets` allow explicit
inspection of an exported device menu; the desktop default discovers manifest
packages without loading PocketHome configuration. The original desktop remains the
supervised device session's recovery destination.

Build and install the shell using [device setup](devices/pocketchip.md).
`integrations/pocketchip/install-session.py` and its adjacent `vitrallis-session.py` are the
installer payload. Run `sh scripts/validate.sh` for the repository's host gates.
