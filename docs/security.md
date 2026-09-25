# Beta trust model

Vitrallis runs as a normal desktop user. Native and third-party apps can access
that user's files, display, environment and network: **the shell is not an app
sandbox**. See [SECURITY.md](../SECURITY.md) for the verified contact route.

## Distribution and packages

The target device bootstrap executes a successfully downloaded official HTTPS script,
then verifies a complete native bundle and matching helper checksums from one
published release. GitHub and the publishing account are trust roots. Checksums
catch corruption and mismatched assets; they are not independent signatures or
protection against a compromised publisher. Bundle contents have five fixed
executable names, bounded sizes, ARM ELF checks and matching version probes.

App Center fetches bounded catalog/manifest metadata and commit-pinned payloads,
checks complete inventories and hashes, and records publisher-bound receipts.
Catalog source trust requires explicit confirmation. Permission metadata is
informational, not enforcement. Runtime inspection does not import app code or install dependencies. Explicit app
installation may provision app-local Python dependencies with read access to
system packages; user-site packages and pip configuration are excluded. App
Center never installs system packages. Review the [App Center contract](app-center.md)
before trusting a new catalog or application.

## Filesystem and execution boundaries

Installation/removal use validated fixed paths, safe ownership/modes, no-follow
file checks, single-link regular files, exclusive locks, staged writes, syncs and
conditional rollback. Receipts do not authorize arbitrary destination paths.
Managed generation pointers accept only the exact relative generation format.
Edited or unrecognized files are preserved; recovery conflicts stop with evidence
retained. Backups and user data are not broadly deleted. See
[offline removal](devices/pocketchip.md#offline-removal-and-recovery).

Application commands use explicit argv. Device-menu command tokenization does
not silently introduce a shell. Command arguments can appear in diagnostics;
do not put secrets in them. Per-child environments intentionally inherit the
existing GUI session. Logs omit environment values and session logs rotate with
bounded sizes.

The supervised target device session owns its systemd cgroup. Stopping validates the
transient user unit and exact supervisor process identity before acting, then
restores temporary Awesome bindings. A window title never grants permission to
kill an unrelated process. The original desktop startup, original configuration and serial
recovery remain available. Power/time-zone actions use existing OS authorization.
PocketCHIP platform preparation separately installs the documented root-owned GPU
trace unit and device-tree provisioning, plus the account-specific Carousel media
sudoers rule; see [device installation](devices/pocketchip.md#gpu-platform-provisioning).

## Limits

These guards are not isolation against a hostile process already running with
full access to the same account. Concurrent ancestor-directory replacement can
still create filesystem races; use safely owned local installation directories.
Crash durability depends on storage honoring sync and atomic rename. An OS or
filesystem stall can outlive a userspace timeout.

Direct manual invocation lacks the systemd supervisor's outer cgroup cleanup.
Close manually started native apps before removing their installation. Local
catalog/image reads are bounded but still occur on the UI thread. Broader
hardware coverage, endurance testing and independently signed distribution remain
future work. [Dependency policy](dependencies.md) and [validation](validation.md)
describe the specific checks and evidence without claiming production readiness.
