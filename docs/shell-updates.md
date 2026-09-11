# Vitrallis Shell self-updates

Open **System Settings → More → Check for Updates**. The Updates page displays
`CARGO_PKG_VERSION` from the running build. Select **Check for Updates** to contact
only the official `csd113/Vitrallis-Shell` GitHub releases API. The highest stable
semantic version wins, regardless of release publication order; drafts and
prereleases are excluded. Build metadata does not change version precedence.
No published stable release, inaccessible/private releases, malformed metadata,
missing builds, and network errors produce a useful failure instead of claiming
that the shell is current. No GitHub token is read or sent.

**Install Update** opens a second confirmation with **Cancel** selected. Escape,
Home, focus loss, or a 15-second timeout cancels confirmation. Checking and
installation run on a worker thread; leaving settings does not cancel an active
installation. The original shell keeps running. Success requires a relaunch:
close Vitrallis and start it through the existing launcher/session mechanism.
Automatic relaunch is deliberately omitted because the shell owns live child
processes and device sessions have their own supervisor lifecycle.

Only the running shell executable is replaced. The updater does not enumerate,
check, download or change installed applications, catalogues, preferences,
session scripts, system packages or App Center contents.

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

An install requires a regular, single-link shell executable in an installation
directory with matching ownership and safe permissions. The user must be able
to write that directory; read-only or administrator-owned installations need
their administrator's normal installation process. No privilege escalation is
attempted. The adapter resolves the running executable, rather than accepting a
release-provided or environment-provided destination.

A private `.vitrallis-shell-update` directory beside the executable contains:

- `lock`: a persistent OS-locked file, released automatically on process exit;
- `download`: a temporary, size-limited payload, removed after failures;
- `previous`: the last working shell backup, retained for manual recovery.

An interrupted payload is discarded at the next locked installation attempt.
Concurrent updater instances are rejected. Symlink staging directories and unsafe
ownership/permissions are rejected. Before replacement, the installer verifies
exact download length, SHA-256, ELF architecture, and the executable's bounded
`--version` startup probe (which also checks native loader/library compatibility).
The staged file and backup are synced before an atomic rename over the installed
shell. Directory sync follows the rename; if it fails, the UI explicitly reports
that replacement happened but disk durability is uncertain. A failure before
rename leaves the installed shell in place. The backup is retained even after
success; an administrator can restore it with a same-filesystem staged copy and
rename while the shell is stopped. Updater-owned temporary files are cleaned on
ordinary failures, and stale downloads after abrupt process termination are
cleaned on retry. The tiny lock file and backup intentionally remain.

Filesystem crash guarantees depend on the filesystem and storage honoring sync
and atomic rename. Processes with the same account's full filesystem access are
inside the installation trust boundary. The checksum establishes integrity
against the official HTTPS release metadata; it is not an independent publisher
signature or protection against a compromised GitHub publishing account.

## Publishing compatible releases

Previously this repository had validation/cross-build scripts but no binary
release workflow. `.github/workflows/shell-release.yml` builds and validates a
native x86-64 executable in Debian 12, packages it, and creates a **draft** GitHub
release on a `v*` tag. Set the authoritative workspace version in `Cargo.toml`
before tagging. Tag and executable version must match Cargo metadata. Review and
test the draft before publishing; mark prerelease versions as prereleases.
Until a stable release and matching verified artifact are published, users will
see a missing stable release/build diagnostic.

Artifacts are raw executables (no archive extraction):

```
vitrallis-<target-triple>-glibc2.36
vitrallis-<target-triple>-glibc2.36.sha256
```

The sidecar is exactly one SHA-256 line naming that executable. GitHub's asset
`sha256:` digest is accepted as well; if both are available they must agree.
The updater refuses artifacts without either verification source. Only uploaded
assets with an exact expected name, size and official repository download URL
are eligible; duplicate or incomplete assets are rejected.

To add another supported architecture, build on a matching glibc 2.36/SDL2 2.26.5
baseline, run the same validation, then use `scripts/package-shell-release.py`
with its explicit `--target`, `--binary`, `--output` and `--tag` arguments. Packaging
runs the built executable's `--version`, so use a native build host or a configured
compatible binary-emulation environment. The existing ARM cross-build helper
can supply the binary, but its image-matched libraries must satisfy this release
ABI contract. Never relabel a newer ABI build as glibc 2.36. Upload both files to
the reviewed draft before publication. Additional architectures are opt-in and
are never inferred from device names.

Metadata fixtures and mock transports cover release/version/error policy without
live GitHub. Filesystem tests cover staging, failed installation, recovery,
permissions, backup retention, concurrency and unchanged application sentinels.
Settings tests cover explicit confirmation, cancellation and navigation.

Protocol references: [GitHub releases API](https://docs.github.com/en/rest/releases/releases)
and [curl options](https://curl.se/docs/manpage.html).
