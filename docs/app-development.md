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

These seven top-level fields and three boolean permissions are the complete v1
vocabulary. Unknown or duplicate fields are errors. IDs are lowercase reverse-domain
identifiers; versions are stable numeric `MAJOR.MINOR.PATCH`. Permissions declare
requirements, not a sandbox or a grant of authority. Package icons are noninterlaced
PNGs of 1–512 pixels per dimension. App Center validates their decoded contents.

Use the upstream [publishing workflow](https://github.com/csd113/Vitrallis-Apps/blob/main/docs/publishing-apps.md)
to pin a source commit and generate the complete sorted inventory, byte sizes and
SHA-256 values. Catalog and manifest metadata must agree. Source paths come from
the catalog and must meet `apps/<app-slug>`; the shell never guesses directories.
Keep `installable` false until the app's target runtime and client integration
have been verified. Existing packages follow the same workflow as new ones.

## Installation, launch and data

Configure the catalog in App Center and explicitly trust any separate source
repository. Check fetches metadata; selecting Install acquires, verifies, and
stages that app. Packages install under `$XDG_DATA_HOME/vitrallis/apps/<id>`
(default `~/.local/share/vitrallis/apps/<id>`). Manifests supply identity, name,
entry and permissions; receipts bind installed versions and file ownership to the
publisher. Locally generated launchers live in the App Center state directory.
Unmanaged files never become owned merely because Python source contains a version.

Use system Python with the declared dependencies or an app-local `.venv`. The
shell does not provision runtimes, install dependencies globally, import app code
during validation, or reuse another application's private runtime. Declare Tk and
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
`integrations/armhf-awesome/install-session.py` and its adjacent `vitrallis-session.py` are the
installer payload. Run `sh scripts/validate.sh` for the repository's host gates.
