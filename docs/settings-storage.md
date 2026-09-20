# Settings and storage

Open **Settings → Storage** with touch or the arrow keys and Enter. Storage is
also available from Device settings. Escape returns to the previous screen.
Power and software installation confirmations still start on Cancel.

The overview measures the filesystem mounted at `/`, displaying capacity, used
bytes, space available to the user, percentage used, and a static usage bar.
Available space can be smaller than total minus used because filesystems reserve
space. Sizes use decimal B/KB/MB/GB. On Unix, app sizes use allocated filesystem
blocks, including directory allocation; sparse files therefore reflect allocated
space rather than their apparent length. Filesystem compression, snapshots and
shared extents can make filesystem totals differ from file allocation estimates.

## Ownership and classification

Launcher discovery and accounting use the same validated installed-manifest
inventory in App Manager. Python and Rust manifests use the same accounting
path, including Rust packages for another architecture that cannot run on the
current host. No application code is executed to obtain storage information.

For each installed app, the scanner measures:

- Installed files listed in its local receipt.
- Private runtimes inside `runtime` or `.venv`.
- Python caches in `.vitrallis-bytecode` and `__pycache__`.
- Other files under that app's installation directory, including saved data and
  configuration, plus its managed launcher, verified desktop entries and its
  transaction backups when a receipt establishes ownership.

An app without a valid receipt still appears, but its classification is marked
incomplete. Missing manifests are treated like App Manager discovery: leftover
files are retained data, not an installed application. Invalid manifests produce
an explicit incomplete-list warning. Missing optional data directories mean zero
bytes; failed reads and missing required roots mean an unavailable or partial
size. `>=` marks a partial byte count. Apps sort by measured bytes, largest first;
unknown zero-byte entries appear after measured entries.

The additional details measure the running shell executable, App Center catalog
and presentation caches, retained app data, manager state/backups, and the defined
Vitrallis data/configuration directories. Already-attributed roots and hard-linked
files are excluded from subsequent categories. Shared files are attributed once
in stable app-ID order; a later app does not add another copy to its size.

The app total may include explicit locations on separate volumes. The estimate
for **Other / unscanned on /** subtracts only allocation on the root filesystem's
device. It includes system software, unclassified files, and filesystem overhead;
it is not a claim that those bytes can be deleted. There is no recursive scan of
`/`, arbitrary home folders, shared system libraries, or generic log/temp folders.
Data written outside known managed locations cannot be attributed reliably and
is not included in app sizes. Symlink targets and nested mounts are never scanned.

## Scheduling and limits

Storage opens immediately. The existing bounded system-command runner queries
`df -kP /` with a two-second deadline. One background worker then scans owned
locations. Rendering and input only read snapshots and a nonblocking channel.
There are no storage timers, animations, deletion controls or notifications.

Snapshots persist across Settings visits. Opening Storage after two minutes, a
manual Refresh, or an App Manager installation/update/removal attempt requests a
new scan. A changed App Manager generation cancels and rejects any older in-flight
result. A failed scan shows an error and retains the previous sizes with an
explicit warning; it does not continuously retry. Changes made by external tools
are picked up by Refresh or by reopening after the cache expires.

Traversal streams directory entries and is bounded to 200,000 entries, 64 levels,
and 20 seconds across the scan. Hard-link identity storage is bounded by the same
entry limit. Read failures, files disappearing, nested mounts, and limits produce
partial results. Closing Storage signals cancellation without joining the worker
on the UI thread. Reopening waits for that worker to finish before starting
another. An operating-system filesystem call blocked in the kernel cannot be
forcibly interrupted, but it cannot block Settings rendering or create duplicate
workers.

Low-storage policy is centralized in `storage::LowSpace`: by default, warn at or
below 100 MB available **or** 5% available. The typed policy can be configured by a
consumer without changing rendering. No disk polling is added for this warning.

## Verification

Unit tests cover units, reserved capacity, malformed capacity results, thresholds,
aggregation/sorting, empty and corrupt metadata, Python/Rust ownership, multiple
locations, missing paths, permission/read errors, symlinks, hard links, overlapping
roots, disappearing directories, traversal limits, cache invalidation, single
worker behavior and cancellation. Keyboard/touch tests cover Storage navigation,
refresh, paging and details. Software-rendered screenshot fixtures exercise
480×272 and larger/smaller layouts, including loading/error/empty/partial states.

For a PocketCHIP hardware check: open Storage while installing an app, leave and
reopen while scanning, refresh after changing app data, navigate every action with
keys and touch, and confirm that unavailable media and low free space do not
freeze input. Hardware timing and resistive-touch accuracy require a device test.
