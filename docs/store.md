# Existing PocketCHIP Store integration

Vitrallis uses [Pocketchip-update-apps](https://github.com/csd113/Pocketchip-update-apps), not a second package ecosystem. In PocketCHIP mode its existing Update Apps launcher is shown as **Store**; the original PocketHome JSON is not renamed. If it is missing, Store remains visible with a setup diagnostic. Installation is deliberately not a remote shell one-liner.

The upstream catalogue is the explicit `APPS` list in `update_apps.py`. Each entry identifies a GitHub repository, branch, source filename, install path and known historical hashes. Bitcoin CAD is a Python/Tk application; its source is fetched from a resolved 40-digit Git commit and verified against GitHub's blob hash and declared size before syntax validation. Versions are read as Python literals with AST parsing, never by importing downloaded application code. Downloads and installed sources have a 2 MiB bound. HTTPS requests have a 20-second socket timeout. Ordinary launch uses a fixed local wrapper and explicit arguments.

## Setup and compatibility patch

The audited upstream revision is `1f394452d6acd124d940154234b0eb8dd7150b70`. Check out that exact revision in a separate directory, inspect it, then apply `integration/pocketchip-store.patch` with `git apply`. `integration/store-patch-manifest.json` records exact original and patched hashes, including intermediate reviewed revisions used during hardware testing. To patch an already installed copy, copy the five patched Python files, manifest and `scripts/apply-store-patch.py` to the device, then run as the normal user:

```sh
python3 apply-store-patch.py /absolute/path/to/patched-source store-patch-manifest.json
```

The installer accepts only those exact reviewed files, refuses unknown local edits/symlinks, takes the updater's own lock, backs up the previous files and atomically replaces individual files. A persistent incomplete-install marker prevents Vitrallis from launching a partially patched updater. Re-running the same installer repairs an interrupted patch. The manifest is a local review record, not a signature or a claim that an arbitrary repository is trustworthy.

For a first installation of the upstream store, inspect the pinned repository's `install.py` and `deployment.py` and run its Python installer as the normal PocketCHIP user. The upstream installer preserves the PocketHome config and can reuse an existing app-local Python/Tk runtime. A working Python 3.7+ and Tk runtime are prerequisites. Do not run its optional apt fallback or install a broad OS upgrade merely for this integration; use the device's already working app-local runtime where available. The tested Debian 13 device uses Python 3.13 with private Tk/Tcl libraries under `~/.local/share/pocket-update-apps/runtime/usr`; the upstream launch wrappers configure those paths. After the base store exists, apply the reviewed compatibility patch above. No packages or runtime libraries were installed during this task.

## Installing and updating applications

Open Store, choose **Check for updates** (C), tick Bitcoin CAD (tap its row or arrows/Space), then choose **Install selected** (I). The updater shows installed and latest versions, reports network/checksum/metadata/install errors, and never preselects an application. Home/Escape closes an idle store. Closing Store refreshes Vitrallis's app catalogue without restarting the launcher.

Bitcoin lives in `~/.local/share/pocket-bitcoin/`: `bitcoin.py`, executable `launch` and `bitcoin.png`. The updater's own files and content-addressed receipts live in `~/.local/share/pocket-update-apps/`. Installation creates/updates the existing Apps item in `~/.pocket-home/config.json` and desktop/application menu shortcuts. Vitrallis discovers that same entry, preserving insertion order and icon path. Existing installs compare source content; unchanged packages are not needlessly reinstalled. Updates preserve a `.py.before-update` backup and a receipt keyed by the new source hash.

The compatibility patch creates `.installation-pending` before multi-file first installs. Interrupted installs are shown as **incomplete / repair** and cannot launch from Vitrallis. Check again and select Install to repair missing files/shortcuts. The marker is removed only when all writes finish. On a reported ordinary exception, the upstream rollback restores the previous files; the marker remains conservatively until a successful repair. Disk-full or read-only errors are reported rather than shown as installation success. Do not delete the marker to bypass a failed installation.

Local Store self-updates are marked **manual** while this patch is required: blindly replacing the updater from upstream would discard the interruption fix. Bitcoin updates remain enabled. Review/apply a newer compatible store bundle using the installer instead. No upstream commits or pushes were made.

The current ecosystem has no safe uninstall API. Vitrallis therefore does not offer automatic uninstall. Preserve user data and backups; manual removal requires reviewing the particular application directory and removing only its matching menu/desktop entries. The task did not delete the pre-existing Bitcoin installation: it remains in `pocket-bitcoin.before-vitrallis-validation` for recovery/comparison.

## Adding applications to the existing repository

This upstream version is not a generic arbitrary-script installer. Adding an `APPS` row alone is insufficient because the first-install path currently supplies Bitcoin's reviewed wrapper/icon. Extend the upstream Python catalogue and its installer adapter together: choose a fixed repository/branch/source, specify the required files and an explicit launcher argument vector, validate hashes/sizes/types/versions, stage all required files, retain an incomplete marker until completion, and add focused check/install/failure tests. Do not add shell fragments to downloaded metadata or reuse the Bitcoin adapter for an unrelated app. Keep new file conventions compatible with PocketHome's name/icon/shell entry so both launchers discover the app. A future multi-file package should use a complete verified bundle and its own safe transactional installer.

Hardware results and evidence are recorded in [the compatibility report](compatibility-step3.md).
