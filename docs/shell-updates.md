# Vitrallis Shell self-updates

Open **System Settings → More → Check for Updates**. The Updates page displays
`CARGO_PKG_VERSION` from the running build. Select **Check for Updates** to contact
only the official `csd113/Vitrallis-Shell` GitHub releases API. Beta and other
prerelease builds receive newer published prereleases as well as stable releases.
Stable builds receive stable releases. Drafts are always excluded. The highest
eligible semantic version wins regardless of publication order: beta.10 is newer
than beta.2, and a stable 0.1.0 is newer than 0.1.0-beta.10. Equal versions,
downgrades, and changes only to build metadata are not installed.
No eligible published release, inaccessible/private releases, malformed metadata,
missing builds, and network errors produce a useful failure instead of claiming
that the shell is current. No GitHub token is read or sent.

Available updates show the complete bundle download size in decimal MB (1 MB =
1,000,000 bytes), including on the install confirmation. During download, the
progress bar and percentage follow actual bytes received, with downloaded/total
MB shown beneath. The bar does not advance while the connection stalls. After
the transfer completes, the status changes to verification and installation;
100% downloaded does not mean installation has succeeded.

**Install Update** opens a second confirmation with **Cancel** selected. Escape,
Home, focus loss, or a 15-second timeout cancels confirmation. Checking and
installation run on a worker thread; leaving settings does not cancel an active
installation. The original shell keeps running until **Relaunch Shell** is selected
by keyboard, mouse, or touch. Relaunch executes the verified installed replacement
at its saved installation path, preserving the process ID, launch arguments, and
session environment so the the target device supervisor remains attached. The installed
SHA-256 and filesystem safety checks run again before execution. Failed attempts
show a diagnostic and retain the relaunch button for retry. Close running apps and
wait for App Center/system operations to finish before relaunching; the action
does not kill apps or interrupt an installation.

The updater replaces the shell and its three bundled native applications as one
complete build generation. It does not change App Center packages, user documents,
catalogs, preferences, session scripts or system packages.

## Platform and installation contract

The build script records Cargo's exact target triple. The Linux adapter recognizes
`x86_64-unknown-linux-gnu`, `aarch64-unknown-linux-gnu` and
`armv7-unknown-linux-gnueabihf`. Releases use the ABI baseline **glibc 2.36** and
**SDL2 2.26.5**. A device must have a matching artifact; other targets, C libraries,
and older glibc installations fail closed. Add future target/ABI adapters in
`src/platform/update.rs`; release parsing has no device or installation-layout
assumptions. macOS development builds can check release versions but cannot
install Linux artifacts.

Updates require `/usr/bin/curl` with HTTPS support and a valid system CA store.
Curl's configuration file is disabled, redirects require HTTPS, and requests
have connection, total-time, low-speed and byte limits. Missing curl or TLS/network
failures are visible. The transport does not execute release-provided commands.

An install requires a managed layout owned by the current user:

```text
~/.local/share/vitrallis/
  generations/<bundle-sha256>/
    vitrallis
    vitrallis-terminal
    vitrallis-notepad
    vitrallis-files
  current -> generations/<active-bundle-sha256>
  previous -> generations/<previous-bundle-sha256>
  .vitrallis-update/lock
```

The running executable must resolve physically inside the active generation.
All four files must be regular, single-link executables with matching ownership
and safe modes; installation directories and pointers are validated. Read-only
or administrator-owned installs need the administrator's installation process.
No privilege escalation or release-provided destination is accepted.

Installer and updater share `.vitrallis-update/lock`. The updater streams the
bounded download there, verifies whole-bundle SHA-256 and exact length, extracts
only the four fixed binary names into a private generation, checks each digest
and ELF target, and runs each bounded `--version` probe. All versions must match.
There are no archive paths, compression, executable install hooks or optional
missing companion files. Download and temporary generation cleanup runs on
failure and the next locked attempt after interruption.

Only after all four binaries pass does the updater sync the generation, retain
the previous pointer, and atomically rename the new `current` symlink. A failure
before this final rename leaves the entire active build unchanged. A sync failure
after the rename is explicitly reported as installed with uncertain durability.
Running apps keep their old physical generation and locate companions there;
relaunch starts the verified new generation. Previous generations are retained,
not pruned while processes may still use them.

For manual rollback, stop Vitrallis and its native apps, verify all four binaries
under `previous`, and replace `current` atomically with that relative generation
link. Do not copy individual binaries between generations. Retain the installation
backups and markers until any interrupted helper/config transaction is repaired.
Use the installed [offline uninstaller](devices/pocketchip.md#offline-removal-and-recovery)
for receipt-based removal. It preserves apps, saves, backups and later edits,
shares the update lock, and can recover a pending helper/removal transaction.

Filesystem crash guarantees depend on the filesystem and storage honoring sync
and atomic rename. Processes with the same account's full filesystem access are
inside the installation trust boundary. The checksum establishes integrity
against the official HTTPS release metadata; it is not an independent publisher
signature or protection against a compromised GitHub publishing account.

## Publishing compatible releases

`.github/workflows/shell-release.yml` builds native x86-64 and cross-builds ARMv7
hard-float executables against Debian 12's glibc 2.36 / SDL2 2.26.5 baseline.
It runs the host validation suite, then verifies ARM startup/version and a 480×272
SDL frame under QEMU with the Cortex-A8 CPU model. Both architectures are packaged
with the exact updater filenames and SHA-256 sidecars. No manual ARM rename or
image-specific SDL download is required for each release.

A `v*` tag creates a **draft** GitHub release; prerelease tags are marked as
prereleases. Existing drafts can receive reviewed artifacts, but the workflow
refuses to modify an already published release. Review/test the complete draft
before publishing. Change the workspace version only with explicit permission.
Tag and executable version must match Cargo metadata. Release beta2.6 uses this
complete bundle contract; older standalone beta2.5 assets cannot satisfy it. See
[release validation and assets](releases.md).

ARM packaging also emits `bootstrap.py`, `install-session.py`, `uninstall.py`,
`vitrallis-session.py`, and a `.sha256` sidecar for each. Initial installation
fetches the matching helpers from the same release as the bundle. Native OTA
updates do not replace these helpers; rerun the reviewed bootstrap with the
session closed when updating installation tooling.

Artifacts are complete, uncompressed Vitrallis bundles:

```
vitrallis-<target-triple>-glibc2.36.vtrbundle
vitrallis-<target-triple>-glibc2.36.vtrbundle.sha256
```

The sidecar is exactly one SHA-256 line naming that bundle. GitHub's asset
`sha256:` digest is accepted as well; if both are available they must agree.
The updater refuses artifacts without either verification source. Only uploaded
assets with an exact expected name, size and official repository download URL
are eligible; duplicate or incomplete assets are rejected.

To add another supported architecture, build on a matching glibc 2.36/SDL2 2.26.5
baseline, run the same validation, then use `scripts/package-shell-release.py`
with its explicit `--target`, `--bin-dir`, `--output` and `--tag` arguments. Packaging
runs every built executable's `--version`, either natively or through an explicitly
provided local emulator executable such as `--runner /usr/bin/qemu-arm`. The
runner is invoked directly without shell parsing. The existing ARM cross-build helper
can supply all four binaries, but its image-matched libraries must satisfy this release
ABI contract. Never relabel a newer ABI build as glibc 2.36. Upload the bundle/checksum and, for ARMv7, all matching helper assets to
the reviewed draft before publication. Additional architectures are opt-in and
are never inferred from device names.

Metadata fixtures and mock transports cover release/version/error policy without
live GitHub. Filesystem tests cover staging, failed installation, recovery,
permissions, backup retention, concurrency and unchanged application sentinels.
Settings tests cover explicit confirmation, cancellation and navigation.

Protocol references: [GitHub releases API](https://docs.github.com/en/rest/releases/releases)
and [curl options](https://curl.se/docs/manpage.html).

## Bundle format

`src/updater/bundle.rs`, the release packager and device installer share one fixed
format: the 16 bytes `VITRALLIS-BUNDLE`, then four records in the order shell,
Terminal, Notepad, Files. Each record contains an unsigned 64-bit little-endian
size, 32 raw SHA-256 bytes, then that executable's bytes. Each executable is
64 bytes–64 MiB; truncation, bad hashes, wrong target and trailing bytes fail.
The maximum total is 256 MiB plus 176 header bytes. Names never come from input.
There are no legacy raw-executable readers or migration paths. Install the current
complete bundle with the current installer when replacing an obsolete pre-release
layout; equal versions do not become self-updates.
