# PocketCHIP installation and recovery

Vitrallis is an additional launch target inside the existing Awesome session.
PocketHome remains installed and remains the normal boot default. The supported
installer consumes one complete native bundle, never a source build or standalone
shell executable. The original menu and launcher files are preserved.

## Prerequisites and release selection

Use the normal desktop account on Debian 12+ ARMv7 hard-float (`armhf`), glibc
2.36+, SDL2 2.26.5+, Python 3.8+, `/usr/bin/curl` with HTTPS and valid CA certificates,
Awesome 4.x, stock PocketHome and a systemd user manager. The recorded
physical image was Debian 13; original Jessie is unsupported. Missing packages
must be resolved separately by the device owner. Installation never uses sudo,
apt, pip, Git, Rust, or device build tools.

On a compatible device, open a terminal as your normal desktop user. The bootstrap
for a published release can be fetched with:

```sh
(set -eu; t=$(mktemp); trap 'rm -f "$t"' 0; trap 'exit 130' 1 2 15; curl -q -fSL --proto '=https' --proto-redir '=https' --connect-timeout 10 --max-time 30 --max-filesize 262144 https://raw.githubusercontent.com/csd113/Vitrallis-Shell/main/integrations/pocketchip/bootstrap.py -o "$t"; python3 "$t")
```

**Current source versus published releases:** this branch changes the installation
and session helpers. The bootstrap requires the `install-session.py` release artifact,
so it will not select earlier releases containing the superseded installer. Until a
matching bundle and helpers are published, it reports no compatible release. Use a
complete locally built ARM bundle with this checkout's helpers for source validation:

```sh
python3 integrations/pocketchip/install-session.py /path/to/vitrallis-armv7-unknown-linux-gnueabihf-glibc2.36.vtrbundle
```

This source change does not bump versions or publish artifacts. The
[USB hardware follow-up](pocketchip/validation-usb-session.md) records fresh testing
of the local ARM bundle. The [stock source audit](pocketchip/stock-source.md) establishes
the config and command contract without requiring an obsolete OS image.

The command above executes Python only after curl exits successfully and removes
the temporary download on success, failure, or a handled interruption. HTTPS-only
redirects, connection and total deadlines, and a size limit constrain that first
request. The bootstrap then reads at most five pages of 30 official GitHub
releases, with bounded requests. It skips drafts and releases without the current
ARM bundle and session installer, choosing the highest semantic version among compatible published
releases, including beta prereleases. A release that advertises the bundle but
has missing, duplicate, corrupt or mismatched helpers fails instead of falling
back. It never reads or sends a GitHub token.

For a reviewed local copy of `bootstrap.py`, `python3 bootstrap.py --stable`
excludes prereleases. `--release` selects an exact published `v`-prefixed version;
that release must have the current complete asset inventory. No raw-binary format
fallback or automatic build exists. [Release assets](../releases.md) documents the
current inventory and the separate historical release evidence.

The bundle, its SHA-256 sidecar, `install-session.py`, `uninstall.py`,
`vitrallis-session.py`, and each helper's sidecar come from the **same release**.
All sizes, exact download URLs and checksums are checked before helper execution.
If GitHub supplies an asset digest it must agree. HTTPS GitHub publication is the
trust root; these hashes are not independent signatures.

The bootstrap on `main` creates downloaded files with mode `0600`, including
when the desktop account uses umask `002`. The originally published beta2.6
bootstrap asset predates this correction; use the command above for the current
bootstrap. Its bundle and installation helpers still come from one published
release.

## Installed files and repeat runs

Save and close Vitrallis apps and stop the previous session before reinstalling.
The installer checks the OS/ABI and runtime libraries, validates every bundled
ARM EABI5 hard-float executable, verifies per-file hashes, and probes all four
matching versions with bounded output and time. The bootstrap also binds that
version to the selected release tag.

```text
~/.local/share/vitrallis/
  generations/<bundle-sha256>/
    vitrallis
    vitrallis-terminal
    vitrallis-notepad
    vitrallis-files
  current -> generations/<active-bundle-sha256>
  previous -> generations/<previous-bundle-sha256>
  launch
  install-session.py
  uninstall.py
  vitrallis-session.py
  installed.json
  .vitrallis-update/lock
```

The installer adds `~/.local/share/applications/vitrallis.desktop`. Stock PocketHome
does not import desktop shortcuts and does not read `~/.pocket-home/config.json`.
Launch from its existing Terminal using the command below. No original menu or
launcher asset is edited, copied into a user override, or required for installation. Helpers and shortcuts are bound to receipt hashes;
local edits block replacement. Symlinks, hardlinks, special files, unsafe ownership
and writable ancestors are rejected before protected writes. Newly created
directories have explicit safe permissions even with a permissive umask.

An `Unsafe directory ownership or permissions` error refers to an existing path
that needs review. The installer preserves existing directory modes and refuses
group/world-writable ancestors. Verify the named directory belongs to the desktop
account and is not intentionally shared before removing its group/world write
permissions. Do not disable the ownership checks or run the installer as root.

Installer, shell updater and uninstaller share `.vitrallis-update/lock`. Files
are staged, synced and renamed; `current` is published only after the complete
helper/shortcut transaction succeeds. Repeat installation of the same bundle is safe.
The previous generation and `~/.local/share/vitrallis-backups/` are retained.
An interrupted installation leaves `.installation-pending`; rerun the matching
installer to repair it, or use the offline uninstaller. Do not remove markers to
bypass validation. Edited files require review, not a forced overwrite.

No Awesome startup file, greetd/login configuration, calibration, system package,
PocketHome binary or recovery service is replaced. Root-owned or obsolete
unreceipted installations require manual reconciliation; the installer does not
infer ownership or migrate a superseded layout.

The beta2.5 updater expects a standalone shell executable. When it sees beta2.6,
it can report that the version is available but no shell build exists for this
platform. The ARM bundle is present; the old updater cannot consume its format.
Replacing that obsolete installation requires a reviewed backup and fresh
installation, including reconciliation of its old receipt and managed shortcuts.
The current installer does not automatically migrate or erase old apps. The
[beta2.6 hardware record](pocketchip/history/beta2.6-device-validation.md) describes the
owner-authorized fresh installation used for validation.

## Launch and return home

From a terminal in the existing graphical session:

```sh
~/.local/share/vitrallis/launch
```

The launcher requires that session's `DISPLAY`, `XAUTHORITY` and
`DBUS_SESSION_BUS_ADDRESS`. It starts a transient `vitrallis-session.service` in
the systemd user manager, with cgroup cleanup, a five-second stop timeout and no
automatic restart. The physical Home/Power key temporarily routes through Awesome
to Vitrallis. Selecting **Exit Vitrallis** stops the owned session and restores the
displaced Home bindings, preserving unrelated new bindings. The previously focused
window is raised if still open; no launcher-specific Lua function is required. Two session logs are limited to 128 KiB each.

To stop from the installed helper:

```sh
python3 "$HOME/.local/share/vitrallis/vitrallis-session.py" stop
```

Stopping verifies the transient user unit, exact supervisor argv, process owner
and process start identity; it never kills processes by name. It checks that the
unit stopped and restores the saved bindings. Launcher/supervisor crash recovery
also uses systemd's `ExecStopPost` helper. PocketHome and serial login remain
available independently.

## Built-in Fn keyboard

Hold **Fn** with **1–0** for **F1–F10**, **minus** for **F11**, and
**equals** for **F12**. **Fn+2** opens Add shortcut on the home screen;
**Shift+Fn+0** selects Terminal's menu. Fn punctuation uses the installed X11
keyboard layout.

Vitrallis translates SDL2's base-key/Right-Alt events on devices identifying as
`nextthing,pocketchip` under X11. Terminal consumes that Fn modifier instead of
prefixing symbols with an Alt/Meta Escape byte. Other hardware keeps its normal
keyboard behavior. No system keymap is rewritten.

Device validation on 2026-09-12 reproduced Fn+2 as SDL key `2` with modifier
`0x200` and Fn+Y as text `{` with the same modifier. The updated beta2.7 bundle
opened Add shortcut with Fn+2 and delivered exact PTY bytes for F1–F12, all 14
tested Fn punctuation symbols, and ordinary `2`/`y`. This used injected physical
X11 keycodes on the device, not manual switch presses. The device was left on
Home; the installer retained its previous generation and backup. Formatting,
strict workspace Clippy, workspace tests and the ARM release build passed.

## Optional startup

Automatic startup is opt-in and is not installed by the bootstrap. If wanted,
back up `~/.config/awesome/rc.lua`, preserve its existing session startup, and
append exactly this block after the existing startup code:

```lua
-- BEGIN optional Vitrallis startup
require('gears').timer.start_new(5, function()
    require('awful').spawn({os.getenv('HOME') .. '/.local/share/vitrallis/launch'}, false)
    return false
end)
-- END optional Vitrallis startup
```

It starts once after five seconds; failure leaves PocketHome available. Removal
recognizes precisely this block and preserves surrounding edits. A customized
block is retained for manual review. Do not restore an entire old `rc.lua` over
later changes.

## Offline removal and recovery

The [README's final command](../../README.md#uninstall) invokes the locally
installed uninstaller. `--dry-run` validates and lists actions without stopping
sessions or writing files. Save work before actual removal: stopping the session
closes its owned applications. An unrelated or unidentifiable session blocks
removal; it is never stopped by broad name matching.

Removal validates the receipt and pending-install receipt, fixed managed paths,
generation contents and relative pointers under the update lock. Whole-generation
hashes identify current and OTA-installed builds without following pointers into
arbitrary directories. Missing/edited generations and modified helper/shortcut
files are preserved. Reconcile an edited session helper before removal; it is
not executed to stop the session. PocketHome configuration is never read or written.
Matching desktop/autostart shortcuts and the exact optional startup block are
removed. Known incomplete download and binary staging files are removed under the same
lock. Unknown contents are never recursively erased.

A private removal journal stages each file by rename, records its identity and
syncs changes. A write failure restores staged files conditionally; a later edit
blocks conflicting rollback and retains recovery evidence. After interruption,
rerun the installed uninstaller. Its normal entry stays in place until journal cleanup finishes, and it can run
even after the installer helper has been removed. A local recovery copy is also kept inside
the pending transaction:

```sh
python3 "$HOME/.local/share/vitrallis/.vitrallis-update/removal/uninstall.py"
```

The journal completes a committed removal or rolls back an unfinished one before
retrying. Preserve it if recovery reports a conflict. A successful removal deletes
the helpers themselves; the README command then reports a missing file if repeated,
with no further changes. The retained lock inode prevents overlapping operations
from accidentally locking different files. Reinstallation can reuse it.

By default, removal keeps all user data, App Center packages and saves,
`~/.local/share/vitrallis/app-center/` transaction backups,
`~/.local/share/vitrallis-backups/`, custom XDG locations, system packages,
PocketHome and unrelated files. `--purge` additionally removes **only** these
regular, safely owned files, after you type `PURGE`:

- `~/.local/share/vitrallis/session.log`
- `~/.local/share/vitrallis/session.log.1`
- `~/.config/vitrallis/screen-timeout`
- `~/.config/vitrallis/app-center.json`

It does not walk arbitrary data/config directories. Inspect the dry run before
purging. App-specific removal remains in [App Center](../app-center.md).

## Build and validation boundaries

Development hosts can build against a reviewed ARM sysroot using
`scripts/build-armhf.sh`; release CI builds against Debian 12's ABI baseline.
See [release packaging](../shell-updates.md#publishing-compatible-releases).
Never substitute host SDL libraries or relabel a newer ABI as glibc 2.36.

[Historical device evidence](pocketchip/history/device-validation.md) covers earlier shell
integration. [Native validation](pocketchip/history/native-validation.md) and the current
[host test suite](../validation.md) have separate scopes. The current installer,
uninstaller and native bundle still need fresh physical PocketCHIP validation.
