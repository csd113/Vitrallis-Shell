# App Center

App Center is a built-in Rust/SDL screen on desktop and PocketCHIP. Every Vitrallis
installation includes **Terminal, Notepad, and Files** as native Rust utilities.
They need no catalog, download, Python runtime, or App Center receipt, and cannot
be removed through App Center. See [native applications](native-apps.md).
Third-party packages and their artwork are acquired separately; no third-party
package or catalog payload is embedded.
The only built-in catalog setting is `csd113/Vitrallis-Apps`. Check resolves that
repository's current default branch through GitHub, pins its commit, and fetches
root `apps.json`. Check downloads only catalog metadata, including the pinned file
inventory, sizes, and hashes. App payload files are downloaded only after the user
selects an app and chooses Install (also used for updates and repairs).

Open **App Center**, choose **Check**, select an app, then **Install**.
Rows show installed/latest versions; **Details** shows the full status, origin,
source, download size, compatibility notes, and declared requirements. Only one
app can be selected at a time. Enter, Space, or a tap selects an app and replaces
the previous selection; activating the selected row again clears it. Keyboard
focus and paging do not change the selection. Details, Install, and Uninstall
all use the same selected app, even when its row is on another page. Details is
disabled until an app is selected. Up-to-date and unavailable apps can be selected
for information; Install is enabled only when the selected app is ready, and
Uninstall only when that app is installed.
Download progress shows actual bytes received / total bytes and
percentage, followed by Verifying and Installing. Cancel (or Escape while acquiring)
stops the remaining downloads before installation; a stalled request can take up to
its 30-second deadline to stop. Once filesystem commit starts, it finishes or rolls
back safely. Check, editing, and installation
cannot overlap. Progress and errors remain visible; Details also exposes long
operation errors after an unsuccessful installation.

Up/down (or keypad 8/2) moves vertically through the app list, scrolling as needed.
Left/right (or keypad 4/6) cycles the button rows: right from Home goes straight
to Previous, Details, then Next, without traversing the list. Tab visits every
visible control. Enter/keypad Enter activates, and Space
toggles a focused app row. C checks and I installs outside text entry. Home/Escape
returns when idle. Previous/Next, Page Up/Down, and the mouse wheel scroll lists
and details. Touch and mouse activate only matching press/release targets. Every
visible control has keyboard focus, including confirmation and paging buttons.
The repository editor accepts SDL text input and includes an on-screen keyboard,
Clear and Delete controls, and a visible insertion end for long batches.

Select an installed app, open **Details**, and choose **Uninstall**. The confirmation
defaults to **Cancel**; keyboard and touch use the same confirmation. Close the app
before uninstalling. Uninstall needs no network requests and removes the receipt's
app files, the managed launcher, and matching desktop entries. Other files,
such as saves and app-local virtual environments not listed in the receipt, remain.
Uninstall requires a validated receipt bound to the app ID and publisher. Removed
files (including locally edited package files) are backed up in the transaction
journal. Custom shortcuts pointing elsewhere are preserved. Uninstall failures
roll back conditionally and keep the incomplete marker for recovery. The app list
and home grid refresh after removal; the catalog app can be installed again.

An **Update available** badge appears beside the latest version when an installable
catalog entry has a newer numeric version than the installed app. Equal versions,
downgrades, unknown local versions, and apps not yet installed have no badge. Details
also displays the update indicator. Repairs at the same version remain selectable
without claiming a newer version exists.

## Catalog sources

**Sources** provides persistent Add, Edit, and Remove controls. Enter `owner/repo`
or an HTTPS GitHub repository URL; whitespace, commas, or semicolons separate a
batch. GitHub names normalize case, an optional `.git` suffix, and trailing URL
slashes. The default appears exactly once and cannot be edited or removed.
Custom sources supplement it. At most 32 catalogs are retained in
`$XDG_CONFIG_HOME/vitrallis/app-center.json` (fallback `~/.config/vitrallis/`).

Each catalog is fetched independently from its resolved default branch's root
`apps.json`. A failing source remains visible as an error row alongside successful
sources. No repository scraping, guessed package folders, or cached failed checks
are used. Entries are keyed by catalog origin plus app ID. Duplicate IDs display
`[!]` and selection asks explicitly which publisher to use. Installation receipts
bind the app ID to both the catalog origin and source repository; an existing
installation cannot silently switch publishers or repositories.

A configured catalog may supply files from its own repository. Another source
repository requires the explicit **Details → Trust source** confirmation, then a
new Check. Approvals are scoped to the originating catalog. Removing a catalog
removes its approvals, invalidates checked results, and does not uninstall apps
or remove saves. Re-add the same source to manage those installations again.

## Package contract

The service implements catalog v1 and package `app.toml` manifest v1 as documented
by [Vitrallis Apps](https://github.com/csd113/Vitrallis-Apps/blob/main/docs/creating-apps.md).
Packages require `app.toml`, `main.py`, `icon.png`, `requirements.txt`,
`README.md`, and a populated `assets/` directory. Source repositories also include
`tests/`, which device packages must exclude. The declared Python
entry must be in the inventory. Manifest ID, name, runtime, entry, version, and
network/audio/storage declarations must agree with the catalog. TOML uses the
standard Rust parser, including rejection of duplicate assignments and unknown
v1 fields. Permissions describe app requirements; **apps are not sandboxed**.

Python packages install under `$XDG_DATA_HOME/vitrallis/apps/<id>` (fallback
`~/.local/share/vitrallis/apps/<id>`). Shell discovery reads their manifests and
uses locally generated launchers. Desktop/application shortcuts register the
installed icon. An existing launcher or shortcut is preserved, including custom
arguments. Unmanaged files and saves are not deleted. Locally edited managed files
block replacement; restore or reconcile them before checking again. Same-version
republishing and downgrades are blocked. Stable version components compare
numerically, including components larger than machine integers.

All source directories must use `apps/<app-slug>` with a lowercase hyphenated
slug. Downloads use the catalog's validated `source.path`, repository, and commit;
no package-specific paths, icons, version inference, or installation modes exist.
Bitcoin uses the same manifest, inventory and ID-based storage as every other app.
The publisher's `installable` flag remains authoritative; a disabled package stays
disabled even when its manifest and bytes validate.

Runtime detection tries an app-local `.venv/bin/python3` and existing system Python.
Missing runtimes, Tk imports, and declared distributions block installation with a diagnostic. No pip, apt, global
installation, remote install script, or app import runs during inspection. Python
syntax is compiled without execution in the detected app runtime, off the UI
thread. Source text currently must be UTF-8. Simple distribution names and exact
pins need only Python's distribution metadata; complex PEP 508 constraints need
an already installed `packaging` module. URL dependencies and extras require
manual review. Toolkit requirements outside detected Tk imports and declared
requirements remain the publisher's responsibility; no device certification is
claimed.

## Verification and recovery

Catalogs are limited to 8 MiB and 1,000 apps per source. Packages allow 256 files,
2 MiB per file, and 16 MiB total. Check retains metadata only, so there is no
catalog-wide prepared-package RAM limit. Installed receipts and local hashes identify
current apps and repairs without fetching payloads. Metadata rows are invalidated on
checks/source edits or after an install batch. Source trust is rechecked on Install.
Only the selected package is acquired and held in memory for its installation;
each payload file is downloaded once during that operation. Runtime prerequisites,
package `app.toml` agreement, and content validation run after acquisition.
An installation plan older than 15 minutes is rejected before commit.

After selection and before mutation, the service walks the pinned Git directory tree and verifies
that the catalog lists its complete device-package inventory and sizes, then downloads and
verifies every SHA-256. Current catalog v1 packages omit only the app-local `tests/`
folder. Catalog inventories containing app-local tests or omitting other files are
rejected. Development tests are never downloaded.
Truncated Git trees, symlinks, submodules, special files,
duplicate JSON keys, duplicate IDs/paths, file/directory collisions, case-colliding
directories, traversal, invalid fields, and installer/runtime path collisions fail
closed. Downloads use system `/usr/bin/curl`, HTTPS, fixed GitHub API/raw hosts,
no redirects, verified TLS, ten-second connect and thirty-second total request
deadlines. GitHub and each configured catalog are trust roots; checksums are not
independent publisher signatures.

A cross-process file lock covers source saves, checks, installation, and recovery.
Writes are staged, synced, and atomically renamed. Existing contents and modes
are checked again before replacement. Receipts contain origin, source repository,
commit, version, ID, and installed hashes. Backups and journals live below
`$XDG_DATA_HOME/vitrallis/app-center/transactions/`; completed journals are retained.
A `.installation-pending` marker prevents discovery from offering a partial install
as healthy. Check marks incomplete installations as repairable. Selecting Install
acquires and verifies the package, then recovers unfinished journals before preparing
repair. Failed or cancelled acquisition never writes app files or install markers.

Rollback restores a file only if it still matches this installation's recorded
output. Later edits are preserved and conflicting recovery stops with a diagnostic.
Do not remove markers or journals to bypass a failed install. Resolve the reported
conflict using the retained before/after backups, then Check and repair. These are
recoverable multi-file transactions, not a claim of atomic visibility across an
entire directory. Filesystem validation rejects symlinks, hard links, special files,
and unsafe writable ancestors; this is not isolation against a hostile process
already running as the same user.

Running apps get a **Cancel-default Close and update** prompt. Confirmation is
bound to that specific request. The worker matches the user's exact Python script
argument and process start identity, rechecks identity before TERM, waits up to
eight seconds, and skips an app that remains running. It never kills by broad name
matching and does not relaunch updated apps. On Linux identity comes from `/proc`;
other Unix hosts use bounded `ps`. Ambiguous whitespace-containing Python arguments
on the latter hosts fail closed. Discovery refreshes after installation, retaining
the selected launcher ID.

The shell updater is accessible only through **System Settings → More → Check for
Updates**. Its separate
installation confirmation and relaunch behavior remain in effect. App Center does
not implement Python manager self-updates or a second shell updater.

## Contract and validation

The implementation follows Vitrallis Apps commit
`a86ae57450d52fd779e0ddcdd794b57213ca8eb8` (catalog/schema, manifest specification,
and publisher tooling). Test metadata is separate from runtime catalogs and is
not embedded in production builds. See [the audit and validation report](app-center-validation.md).

| Reference behavior | Native implementation / fixture evidence |
| --- | --- |
| Installed/latest status, disabled rows, independent checks | `check_all`, metadata/network fixtures; disabled entries make no package requests |
| Unchecked rows and sequential selected installs | screen selection tests, captured worker command, per-app batch error handling |
| First install/update/repair | manifest-package fixtures, missing icon and pending-marker repair |
| Numeric versions and local-copy protection | strict versions, downgrade, same-version inventory, origin and local-edit checks |
| Pinned complete bundle, path/hash checks | Git-tree fixtures, malicious metadata, size/hash mismatch, reserved paths |
| Runtime/dependency checks | real runtime syntax/dependency test; a source containing a file write is never executed |
| Launchers, icons, shortcuts, saves, backups and receipts | install fixtures; custom launcher and unmanaged save retained |
| Interrupted writes and conditional rollback | failure injection, repeatable recovery and later-edit conflict tests |
| Running app confirmation and eight-second wait | Cancel/default/token tests, real owned test process, stale identity and timeout tests |
| Keyboard/keypad/touch and 480×272/larger UI | target geometry/focus and text/gesture tests at 480×272 and 800×480 |
| Manager self-update | existing Settings → Updates service, no Python updater process |
| Additional catalogs | persistent default/batch normalization, scoped trust, per-source errors and explicit conflict selection |

Run `scripts/validate.sh` for formatting, strict Clippy, Rust/Python tests, release
build, SDL dummy-driver smoke, script syntax checks, and diff whitespace checks.
Use `cargo +1.91.0 check --locked --workspace --all-features` for the declared MSRV.
These host checks do not exercise hardware or deploy packages.
