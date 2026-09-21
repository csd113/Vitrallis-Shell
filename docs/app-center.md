# App Center

App Center is a native Rust/SDL screen for Linux desktop sessions. Terminal,
Notepad and Files are bundled native utilities and cannot be removed here.
Third-party apps come from configured GitHub catalogs; no catalog payload or
third-party app is embedded in the shell.

## Browse and manage apps

Open **App Center** and choose **Refresh** on first use. Later openings immediately
show the saved catalog and check local installations without contacting repositories.
**Refresh** fetches new remote catalog information. Installing, updating, removing,
reopening, and checking installed state never trigger a remote catalog refresh.

Rows show an icon, name, description, and either the available version, installed
version, update transition, operation progress, or failure. Select a row, then use
its primary **Install**, **Update**, **Repair**, or **Open** action. **Details**
shows the selected app's description, installed and available versions, status,
repository, requirements, download size and last operation error. **What's New**
is accessible directly from Details before updating. Destructive **Remove** is
separate from the primary action and always opens a Cancel-default confirmation.

**Search apps** matches names and descriptions. The adjacent filter cycles through
**All**, **Installed**, and **Updates** and shows the matching count. Search's
**Clear**, then **Search**, restores an empty query. No-results text explains how
to clear search or choose All. Query, filter, selected app and browsing position
survive app mutations. Returning from Details restores the list position. An
updated app naturally leaves the Updates filter; that filter stays active.

Only one filesystem operation runs at a time. Browsing, paging, reading Details,
viewing release notes, and searching remain available during an operation.
Conflicting mutation/source actions are disabled. The active app shows progress;
other rows remain present. Download progress reports actual bytes, then verification
and installation. Cancel stops acquisition before commit. A stalled curl request
can take up to its 30-second deadline to stop. Commit finishes or rolls back once
filesystem mutation begins. Failures remain in the status area and the app's Details.

Keyboard arrows/keypad 8/2 move vertically; left/right or keypad 4/6 cycle button
rows. Tab reaches every visible control, including search, filters, paging,
confirmation and on-screen keyboard keys. Enter/keypad Enter or Space activates.
C refreshes and I installs outside text entry. Home returns to the shell. Escape
backs out, declines a confirmation, or cancels acquisition on the app list.
Page Up/Down and the mouse wheel scroll. Touch/mouse require a matching
press/release target. Losing focus declines pending confirmations.

## Apps page actions and folders

The Apps page bottom bar contains **Actions**. Add Shortcut is available in that
menu (F2 also opens its editor); system controls remain accessible through the
System Settings tile and Power key. Tab selects Actions; Enter or Space opens it.
The action label is centralized for a future rename.

Actions includes Create folder, Rename folder, Delete folder and Move app to
folder / Apps. Open a folder like an app. Inside a folder, the header shows its
name, **Back to Apps** is reachable with Tab and touch, and Escape returns to the
folder tile without closing its running app. Deletion defaults to Cancel and
returns contained apps to the unfiled Apps view. The app binaries, receipts and
saved data are untouched. The folder editor shares the shortcut editor's keyboard
and touchscreen text entry. Folder state is stored atomically in
`$XDG_DATA_HOME/vitrallis/folders.json`, independently of shell generations and
app installations. Malformed or unsafe state cannot be overwritten by an action.

Running apps show one filled **RUNNING** chip in a fixed corner of the tile, over
the icon rather than the name. The launcher's authoritative process state decides
it, the same chip appears in folders and the App Center list, and there is no
animation timer, so text positions and idle rendering behavior are unchanged.
Launching and exit notices appear in the lower-left status area instead of a
modal screen, and a launch that is in flight refuses a duplicate activation
while arrow keys keep working.

## Repositories and cached metadata

**Sources** opens repository management. Add/Edit accept `owner/repo` or an HTTPS
GitHub repository URL, with whitespace, commas or semicolons separating a batch.
Names normalize case, optional `.git` suffixes, and trailing URL slashes. The
built-in `csd113/Vitrallis-Apps` source appears once and cannot be removed or edited.
Up to 32 configured sources are saved in
`$XDG_CONFIG_HOME/vitrallis/app-center.json` (default `~/.config/vitrallis/`).

Each source resolves its current default branch and commit, then downloads root
`apps.json`. Its last validated snapshot is saved independently under
`$XDG_DATA_HOME/vitrallis/app-center/catalogs/`. A failed refresh preserves that
source's previous catalog, including pinned inventories. Other sources continue
working. A bad individual app becomes an unavailable diagnostic entry; malformed
catalog structure, duplicate JSON keys or duplicate IDs reject that source's new
snapshot. Repository errors are visible in source rows as well as diagnostic app
rows. Source removal drops only that source's displayed rows and approvals; it
does not uninstall apps or delete user data.

Entries are keyed by originating repository plus app ID. Duplicate IDs across
sources require explicit publisher selection. Receipts bind installations to the
app ID, catalog origin and package source; a different source cannot silently take
over an installation. A separate package source requires **Details → Trust source**
confirmation. Approvals apply to that originating catalog and are rechecked on
installation. Cached availability is not proof that an offline download will work.

## Changelog and icon contract

The current [Vitrallis Apps catalog contract](https://github.com/csd113/Vitrallis-Apps/blob/cd1cbf913044bfe7edd3e2ade656a85b90b06e9c/docs/catalog-format.md)
already inventories `CHANGELOG.md` and `icon.png` as pinned package files. App
Center uses that convention directly: **no manifest or catalog schema version
change, extra endpoint, or second changelog format is required**.

Publish `CHANGELOG.md` in the app directory and include its size and SHA-256 in the
normal sorted `files` inventory. The publisher tooling's current release policy
requires dated changelogs. Use UTF-8, newest release first, with a heading for the
available version and optional older sections:

```markdown
# Changelog

## 1.2.3 — 2026-09-12

- Describe the visible changes in this release.
- Preserve paragraph breaks and short bullet lists.

## 1.2.2 — 2026-09-01

- Earlier changes.
```

On an explicit repository refresh, App Center fetches the pinned changelog (up to
64 KiB) and icon (up to 256 KiB), verifies their inventory size/hash, and caches
them by content hash. Unchanged presentation files are reused on later refreshes.
Opening Details/What's New, browsing, and local scans make no presentation requests.
Missing, oversized, invalid UTF-8 or control-filled changelogs show a readable
no-release-notes message and cannot break the catalog. Markdown is displayed as
plain, scrollable multiline text; links or embedded HTML are never executed.
Icons are validated and reduced to 32×32 pixels off the UI thread. Decoded icons
and release notes are shared between worker and screen rather than repeatedly
cloned or loaded from disk during rendering.

## Package acquisition and installation

Only the selected package payload is acquired. Downloads use the declared
`source.repository`, full commit, and canonical `apps/<lowercase-hyphenated-slug>`
path. Before downloading, the worker walks the pinned Git tree to verify the
complete inventory and sizes. Device inventories exclude only app-local `tests/`.
Every payload file is fetched from its pinned commit and checked against SHA-256;
there is no mutable archive cache to reuse across releases. Pinned Git executable
modes are retained. Untrusted paths, symlinks, submodules, special files, incomplete
trees, collisions and reserved runtime/installer paths fail closed.

Catalog v1 and manifest v1 must agree on ID, name, version, runtime, runtime-specific
entry/binaries and network/audio/storage requirements. Python packages require `app.toml`, `main.py`,
`icon.png`, `requirements.txt`, `README.md` and populated `assets/`. The declared
entry must be an inventoried Python file. Runtime detection checks a managed
app-local environment, an existing `.venv/bin/python3`, then system Python
candidates. When declared Python dependencies are missing, installing an app
creates an app-local environment under `runtime/<requirements hash>` with access
to the base interpreter's system packages. Pip retains already-compatible system
distributions (for example Debian's Pillow on ARMv7), installs missing or
incompatible requirements locally, and never upgrades or removes system packages.
`requirements.txt` plus `packaging` are passed to pip without `--upgrade`. This is
an intentional extension of the existing system-Python trust boundary, not a
package sandbox: system package updates can affect these environments. Existing
private `.venv` packages are not copied into a new environment.

Probes and provisioning use Python isolated mode (`-I`); launchers disable user
site packages and Python startup/home/path overrides while retaining app-local
imports. Pip settings from the environment and all pip configuration files are
disabled, preventing inherited target/prefix/user settings from redirecting writes.
The shared requirement validator rejects options, URLs, paths, extras (including
inactive extras), and malformed specifiers before pip runs. Environment markers
and version ranges are checked again, with Tk when required, in the staged
interpreter before publication. Failed provisioning removes the staged environment; existing environments are preserved. Python/Tk and venv/pip
must be available on the system, and dependency downloads require network access.
The worker reports dependency checking/installation separately from package commit.
It waits for venv and pip completion, consumes bounded stdout/stderr through EOF,
and preserves the last 8 KiB of each output on failure. A useful compiler/pip
error is repeated first; the complete retained output is scrollable in Details.
Runtime checks wait for process completion with a 30-second deadline (provisioning: 720 seconds), including
the first check after provisioning on slow hardware. Pip options, paths, URLs and
extras in requirements are rejected. Declared
distributions, Tk imports and syntax are checked without importing app code.
Catalog checks do not install dependencies. App Center does not run apt or
publisher install scripts, and does not install Python dependencies globally.
Permissions are requirements; applications are not sandboxed.

Rust packages declare precompiled binaries by target ABI and do not run Python or
Cargo. See the [Rust package contract](app-development.md#precompiled-rust-packages)
for manifests, binary checks, executable permissions and publisher prerequisites.

Installed packages live at `$XDG_DATA_HOME/vitrallis/apps/<id>` (default
`~/.local/share/vitrallis/apps/<id>`). Receipts, generated launchers, application
shortcuts and icons use this canonical installation. The managed launcher is
regenerated from the current entry, runtime and source commit. A locally edited
launcher blocks replacement and is preserved with a diagnostic. Custom desktop
shortcuts remain user-owned. Unmanaged data and app-local virtual environments
are retained; package code should keep user data outside its read-only installation.

Updates remove old receipt-owned files absent from the new inventory. Managed
Python module caches are removed transactionally. Launchers use a release-specific
bytecode-cache namespace and disable bytecode writes, so interpreter-wide or
same-size/same-second caches cannot silently execute a previous release. Python
startup/home/path overrides are excluded consistently with runtime preflight.
Same-version republishing, downgrades, source switches and modified managed source
files are rejected before replacement.

Running-app detection reads the **installed manifest's runtime entry**, even when the
available version changes entry paths. **Close and update** defaults to Cancel.
Confirmation matches the exact Python script or native executable and process start identity, then sends TERM
and waits up to eight seconds. New or unclosed matching processes block mutation.
Updated apps remain closed until the user opens them.

## Transactions, discovery and recovery

A cross-process lock covers source settings, refresh, install, remove and recovery.
Writes are staged, synced and atomically renamed individually. A durable journal
records before/after bytes and modes, including removals. Final readback verifies
every planned output before marker cleanup and journal completion. A finalization
failure participates in rollback. Successful rollback releases the incomplete
marker; unresolved conflicts retain it and a recovery diagnostic.

These are journaled multi-file transactions, not an atomic directory swap. A
`.installation-pending` marker prevents launch/discovery of a partial installation.
Recovery restores a path only if it still matches the transaction's recorded
output; later user changes are preserved. Journals and backups remain under
`$XDG_DATA_HOME/vitrallis/app-center/transactions/`. Do not delete markers or journals
to bypass a recovery failure. Resolve the reported conflict and repair the app.

After each mutation the worker refreshes the affected row's local status and the
shell refreshes installed-app discovery independently of device-menu configuration.
A broken PocketHome config therefore cannot prevent an otherwise valid new app
from registering in the live Vitrallis menu. Refresh retains launcher selection
by ID. Removal validates the receipt, refuses a running app, removes only owned
package/support files and matching managed shortcuts, then updates the row/menu.

Catalogs are bounded to 8 MiB and 1,000 apps per source. Packages allow 256 files,
2 MiB per file and 16 MiB total. Transfer hosts are fixed GitHub API/raw HTTPS
hosts with verified TLS, no redirects, ten-second connection and thirty-second
request deadlines. Plans expire after 15 minutes. Hashes establish publisher
content integrity, not an independent signature. Filesystem checks reject unsafe
ancestors, links and special files; they do not isolate hostile same-user processes.
No obsolete package-layout migrations or compatibility paths are provided.

See [the validation report](app-center-validation.md) and
[Docker simulator instructions](../tests/simulator/README.md).

## Physical validation

See the [PocketCHIP App Manager validation record](devices/pocketchip/app-manager-validation.md)
for the isolated hardware deployment, dependency prerequisites, lifecycle results,
resource measurements and physical display verification limits.
