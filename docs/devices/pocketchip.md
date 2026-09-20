# PocketCHIP installation and recovery

Vitrallis is an additional launch target inside the existing Awesome session.
PocketHome remains installed and remains the normal boot default. The supported
installer consumes one complete native bundle, never a source build or standalone
shell executable. The original menu and launcher files are preserved.

## Prerequisites and release selection

Use your normal desktop account on a PocketCHIP running Debian 12+ ARMv7
hard-float (`armhf`), with the existing Awesome 4 desktop, PocketHome, systemd
user manager and CHIP flash-kernel boot layout. The recorded physical image is
Debian 13; original Jessie is unsupported. The desktop account must already have
sudo access and be logged into the device desktop. Internet access to your Debian
repositories and GitHub is required. Keep at least 128 MiB free on the system,
home and temporary filesystems; APT may require more for missing dependencies.

Paste the **entire block** into Terminal on the PocketCHIP, or an SSH terminal
logged in as that same user. It checks the device and existing session, installs
missing prerequisites, downloads and verifies one complete release, and installs
it as your normal user. It may request your sudo password. Exit any running
Vitrallis session first; your open applications are never forcibly closed.

<!-- pocketchip-bootstrap -->
```sh
(
set -eu
PATH=/usr/sbin:/usr/bin:/sbin:/bin
export PATH
LC_ALL=C
export LC_ALL
umask 077
fail() { printf '%s\n' "Vitrallis setup: $*" >&2; exit 1; }
[ "$(id -u)" -ne 0 ] || fail 'Run as your normal desktop user, without sudo.'
[ "$(uname -s)" = Linux ] || fail 'Requires a PocketCHIP running Debian 12 or newer.'
case "$(uname -m)" in armv7l|armv8l) ;; *) fail 'Requires 32-bit ARMv7 Linux.' ;; esac
[ "$(dpkg --print-architecture)" = armhf ] || fail 'Requires Debian armhf.'
vitrallis_libc=$(getconf GNU_LIBC_VERSION)
case "$vitrallis_libc" in 'glibc '*) ;; *) fail 'Requires glibc.' ;; esac
dpkg --compare-versions "${vitrallis_libc#glibc }" ge 2.36 || fail 'Requires glibc 2.36 or newer.'
# Parse data; never source an OS/configuration file as shell code.
[ "$(sed -n 's/^ID=\([^" ]*\)$/\1/p;s/^ID="\([^" ]*\)"$/\1/p' /etc/os-release)" = debian ] || fail 'Requires Debian.'
vitrallis_debian=$(sed -n 's/^VERSION_ID=\([0-9]*\)$/\1/p;s/^VERSION_ID="\([0-9]*\)"$/\1/p' /etc/os-release)
case "$vitrallis_debian" in ''|*[!0-9]*) fail 'Cannot identify the Debian release.' ;; esac
[ "$vitrallis_debian" -ge 12 ] || fail 'Requires Debian 12 or newer; original Jessie is unsupported.'
tr '\000' '\n' < /sys/firmware/devicetree/base/compatible | grep -qx 'nextthing,pocketchip' || fail 'Requires a PocketCHIP.'
command -v sudo >/dev/null || fail 'sudo must be configured for your desktop account first.'
command -v awesome-client >/dev/null || fail 'Requires the existing Awesome desktop.'
awesome --version | grep -q '^awesome v4\.' || fail 'Requires the existing Awesome 4 desktop.'
[ -r /boot/boot.scr ] || fail 'Requires the supported CHIP flash-kernel boot layout.'
vitrallis_uid=$(id -u)
vitrallis_runtime=/run/user/$vitrallis_uid
[ "$(stat -c '%u:%a' "$vitrallis_runtime")" = "$vitrallis_uid:700" ] && [ ! -L "$vitrallis_runtime" ] || fail 'Log into the PocketCHIP desktop first; its private user runtime is unavailable.'
[ -S "$vitrallis_runtime/bus" ] && [ "$(stat -c '%u' "$vitrallis_runtime/bus")" = "$vitrallis_uid" ] && [ ! -L "$vitrallis_runtime/bus" ] || fail 'The desktop user bus is unavailable; log into the device desktop first.'
vitrallis_state=$(env -i PATH="$PATH" XDG_RUNTIME_DIR="$vitrallis_runtime" DBUS_SESSION_BUS_ADDRESS="unix:path=$vitrallis_runtime/bus" systemctl --user show vitrallis-session.service --property=ActiveState --value) || fail 'Cannot contact your desktop user manager. Log into the device desktop first.'
case "$vitrallis_state" in inactive|failed) ;; *) fail 'Save your work and exit Vitrallis before installing.' ;; esac
for vitrallis_path in / "$HOME" /tmp; do
    df -Pk "$vitrallis_path" | awk 'END {exit !($4 >= 131072)}' || fail "Keep at least 128 MiB free on $vitrallis_path."
done
vitrallis_tmp=$(mktemp -d /tmp/vitrallis-setup.XXXXXXXX)
trap 'rm -rf "$vitrallis_tmp"' 0
trap 'exit 130' 1 2 15
vitrallis_packages='curl ca-certificates python3 libsdl2-2.0-0 picom device-tree-compiler python3-tk python3-venv python3-packaging bubblewrap'
vitrallis_missing=''
for vitrallis_package in $vitrallis_packages; do
    if [ "$(dpkg-query -W -f='${Status}' "$vitrallis_package" 2>/dev/null || :)" != 'install ok installed' ]; then
        vitrallis_missing="$vitrallis_missing $vitrallis_package"
    fi
done
if [ -n "$vitrallis_missing" ]; then
    printf 'Missing prerequisites:%s\n' "$vitrallis_missing"
    printf '%s\n' 'Installing these Debian packages and their required dependencies. Your sudo password may be requested.'
    sudo -v || fail 'Administrator access is required to install missing prerequisites.'
    # The privileged commands use a clean environment and fixed executable paths.
    sudo -- /usr/bin/env -i PATH="$PATH" LC_ALL=C /usr/bin/apt-get -o DPkg::Lock::Timeout=30 -o APT::Update::Error-Mode=any update || fail 'Could not refresh Debian packages. Check your network/repositories, then rerun this block.'
    # Intentional word splitting: every requested name comes from the fixed list above.
    sudo -- /usr/bin/env -i PATH="$PATH" LC_ALL=C /usr/bin/apt-get --simulate --no-install-recommends --no-remove --no-upgrade install $vitrallis_missing > "$vitrallis_tmp/plan" || fail 'Package planning failed. Resolve the APT error and rerun this block.'
    cat "$vitrallis_tmp/plan"
    # Refuse upgrades, removals, reinstalls and unrelated pending configuration.
    awk '
        /^Inst / {
            if ($3 !~ /^\(/ || $2 !~ /^[a-z0-9][a-z0-9+.:_-]*$/) exit 1
            version = substr($3, 2)
            if (version !~ /^[a-zA-Z0-9.+:~_-]+$/) exit 1
            planned[$2] = 1
            print $2 "=" version
        }
        /^Remv / {exit 1}
        /^Conf / {if (!planned[$2]) exit 1}
    ' "$vitrallis_tmp/plan" > "$vitrallis_tmp/packages" || fail 'APT proposed upgrades, removals or unrelated repairs. Review your package state before retrying.'
    [ -s "$vitrallis_tmp/packages" ] || fail 'APT did not produce a usable installation plan.'
    vitrallis_plan=$(tr '\n' ' ' < "$vitrallis_tmp/packages")
    # Recheck actual archives under APT's lock, closing the simulation/install gap.
    # Only validated package/version tokens are embedded; no user-writable hook file.
    vitrallis_guard='audit=$(/usr/bin/dpkg --audit) || exit 1
    [ -z "$audit" ] || { echo "Vitrallis refused pending package repairs" >&2; exit 1; }
    while IFS= read -r archive; do
        package=$(/usr/bin/dpkg-deb -f "$archive" Package) || exit 1
        version=$(/usr/bin/dpkg-deb -f "$archive" Version) || exit 1
        architecture=$(/usr/bin/dpkg-deb -f "$archive" Architecture) || exit 1
        case "$architecture" in armhf|all) ;; *) exit 1 ;; esac
        case " '"$vitrallis_plan"' " in
            *" $package=$version "*|*" $package:$architecture=$version "*) ;;
            *) echo "Vitrallis refused an unplanned package: $package" >&2; exit 1 ;;
        esac
        status=$(/usr/bin/dpkg-query -W -f="\${Status}" "$package" 2>/dev/null) || {
            code=$?
            [ "$code" -eq 1 ] && [ -z "$status" ] || exit 1
        }
        case "$status" in ""|"unknown ok not-installed"|"deinstall ok config-files") ;;
            *) echo "Vitrallis refused to replace or repair $package" >&2; exit 1 ;;
        esac
    done'
    sudo -- /usr/bin/env -i PATH="$PATH" LC_ALL=C DEBIAN_FRONTEND=noninteractive /usr/bin/apt-get -o DPkg::Lock::Timeout=30 -o "DPkg::Pre-Install-Pkgs::=$vitrallis_guard" --no-install-recommends --no-remove --no-upgrade -y install $vitrallis_plan || fail 'Package installation failed. Some packages may already be installed; inspect the APT error before rerunning. Vitrallis has not been installed by this run.'
fi
for vitrallis_package in $vitrallis_packages; do
    [ "$(dpkg-query -W -f='${Status}' "$vitrallis_package" 2>/dev/null || :)" = 'install ok installed' ] || fail "Prerequisite verification failed: $vitrallis_package."
done
python3 -I -c '
import ctypes, sys, tkinter, venv, ensurepip, packaging
if sys.version_info < (3, 8):
    sys.exit("Requires Python 3.8+")
version = (ctypes.c_ubyte * 3)()
sdl = ctypes.CDLL("libSDL2-2.0.so.0")
sdl.SDL_GetVersion.argtypes = [ctypes.POINTER(ctypes.c_ubyte)]
sdl.SDL_GetVersion.restype = None
sdl.SDL_GetVersion(version)
if tuple(version) < (2, 26, 5):
    sys.exit("Requires SDL2 2.26.5+")
' || fail 'SDL2/Python/Tk/venv/packaging verification failed.'
for vitrallis_tool in curl python3 picom dtc fdtoverlay bwrap; do
    command -v "$vitrallis_tool" >/dev/null || fail "Missing runtime tool after preparation: $vitrallis_tool."
done
printf '%s\n' 'Prerequisites ready. Downloading the Vitrallis installer...'
curl -q -fSL --proto '=https' --proto-redir '=https' --max-redirs 5 --connect-timeout 10 --max-time 30 --max-filesize 262144 https://raw.githubusercontent.com/csd113/Vitrallis-Shell/main/integrations/pocketchip/bootstrap.py -o "$vitrallis_tmp/bootstrap.py"
python3 -I "$vitrallis_tmp/bootstrap.py"
)
```

The block is maintained verbatim in
[`bootstrap.sh`](../../integrations/pocketchip/bootstrap.sh). It installs only
missing `curl`, `ca-certificates`, `python3`, `libsdl2-2.0-0`, `picom`,
`device-tree-compiler`, `python3-tk`, `python3-venv` and `python3-packaging`, plus
their required Debian dependencies. It prints the APT plan, rejects upgrades,
removals and unrelated repairs, pins planned versions, and checks actual package
archives again before dpkg runs. Existing sufficient packages are retained.
Python app-specific dependencies are still provisioned by App Center when needed;
this does not install every app, FFmpeg, development libraries or a compiler.

If APT fails, the block stops before installing Vitrallis. Packages already
installed and refreshed APT indexes remain; package operations and the later
Vitrallis transaction are not one atomic operation. Read the error, resolve a
network/repository or package-manager problem, and rerun the same block. The
script does not delete APT locks or automatically repair/upgrade the OS.
Normal Vitrallis uninstall retains these system packages and platform settings.

From the device's local graphical Terminal, setup opens Vitrallis and waits up to
20 seconds for its owned shell window. **Home** returns from an app;
**Exit Vitrallis** restores the original desktop. SSH installs, including X11
forwarding, finish with the on-device launch command instead. They use the
validated existing user manager; they do not invent display credentials or start
a new login session. Missing graphical access also defers launch. A launch failure
is reported separately from the successful installation, with the session log and
retry command. Automatic startup at boot is not enabled. A GPU reboot notice
still applies even if Vitrallis opens successfully.

**Source and published releases:** beta3.1 already provides the complete ARM
bundle and matching session/platform helpers. The prerequisite and checked-launch
entry-point changes in this checkout become public only when the updated source
is published at the URL above. This work does not bump versions or publish a
release. To validate a complete locally built ARM bundle with this checkout's
helpers after preparing prerequisites:

```sh
python3 integrations/pocketchip/install-session.py /path/to/vitrallis-armv7-unknown-linux-gnueabihf-glibc2.36-v2.vtrbundle
```

That local installer is the bundle transaction; automatic package preparation and
first launch belong to the public bootstrap flow. See the
[USB hardware follow-up](pocketchip/validation-usb-session.md) for earlier device
testing and the [stock source audit](pocketchip/stock-source.md) for the original
session contract. See the [bootstrap validation record](pocketchip/bootstrap-validation.md) for
fixture and simulator results. Fresh-device validation of this new entry point
is still needed.

The command above executes Python only after curl exits successfully and removes
the temporary directory on success, failure, or a handled interruption. HTTPS-only
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
`vitrallis-session.py`, `platform-setup.py`, `media-setup.py`, and each helper's sidecar come from the **same release**.
All sizes, exact download URLs and checksums are checked before helper execution.
If GitHub supplies an asset digest it must agree. HTTPS GitHub publication is the
trust root; these hashes are not independent signatures.

The bootstrap on `main` creates downloaded files with mode `0600`, including
when the desktop account uses umask `002`. The originally published beta2.6
bootstrap asset predates this correction; use the command above for the current
bootstrap. Its bundle and installation helpers still come from one published
release.

## GPU platform provisioning

The installer automatically runs the [PocketCHIP GPU setup](pocketchip/gpu-utilization.md)
after validating the native bundle. It patches only missing GPU OPP data in the
selected boot DTB and kernel source DTB, preserves existing OPP configurations,
and installs a boot oneshot to expose a single read-only utilization pipe.
Reboot notices appear in the installer and System Settings → Updates until the
running tree has the OPP. Already configured systems do not need a reboot.
The root-owned GPU support is system configuration: normal user uninstall retains
it along with system packages. Disable its trace service explicitly if no longer
needed; the document above lists the exact installed paths.

The separate `media-setup.py` step installs Carousel's fixed FFmpeg installation
action at `/usr/local/libexec/vitrallis-carousel-install-media` and a validated,
account-specific rule under `/etc/sudoers.d/vitrallis-carousel-media-<user>`.
The root-owned action accepts no arguments and installs only the fixed `ffmpeg`
package request when invoked from Carousel. Setup itself does not install FFmpeg.
Normal user uninstall retains these system-owned files.

## Graphics runtime and diagnostics

For upstream Debian 13 Mali-400/Lima acceleration, the physical target uses
`libsdl2-2.0-0`, `libgl1-mesa-dri`, `libegl1`, `libegl-mesa0`, `libgles2` and
`libdrm2`, plus `picom` for synchronized window composition. These names were checked against its installed packages. The owner
provisions distro packages; Vitrallis never installs proprietary Mali blobs or
replaces the graphics driver. Installer checks for optional EGL/GLES/DRM libraries
and DRM node presence are advisory; missing hardware cannot block software use.

The session supervises an effects-free `picom --config /dev/null --backend xrender
--vsync` process when X11 has no existing compositor. Its full-screen backbuffers
are presented through X Present; XRender uses the existing glamor acceleration.
An existing compositor is preserved and its synchronization remains externally
managed. The owned compositor exits with the session; PocketHome startup and
system Xorg configuration are unchanged. Failures are logged to `session.log`
as a possible-tearing fallback, without preventing the desktop from opening.

From the existing desktop session after installation:

```sh
~/.local/share/vitrallis/current/vitrallis --graphics-info
~/.local/share/vitrallis/current/vitrallis --graphics-test --renderer auto
~/.local/share/vitrallis/current/vitrallis --graphics-test --renderer software
```

The test does not load apps or mutate user data. `--renderer hardware` requires
an accelerated SDL backend without a known software Mesa identity. Auto preserves
hardware errors and falls back to software. See [hardware acceleration](../hardware-acceleration.md)
for identities, permissions, troubleshooting, package scope and the explicitly
separate physical/architectural validation matrix. The [Phase 3 record](pocketchip/graphics-phase3.md)
tracks the current device results; it does not claim Raspberry Pi hardware testing.

## Installed files and repeat runs

Save and close Vitrallis apps and stop the previous session before reinstalling.
The installer checks the OS/ABI and runtime libraries, validates every bundled
ARM EABI5 hard-float executable, verifies per-file hashes, and probes all five
matching versions with bounded output and time. The bootstrap also binds that
version to the selected release tag.

```text
~/.local/share/vitrallis/
  generations/<bundle-sha256>/
    vitrallis
    vitrallis-terminal
    vitrallis-notepad
    vitrallis-files
    arti
  current -> generations/<active-bundle-sha256>
  previous -> generations/<previous-bundle-sha256>
  launch
  install-session.py
  uninstall.py
  vitrallis-session.py
  platform-setup.py
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

## Tor networking

The complete bundle includes a separate shared Arti executable. Bootstrap also
installs Bubblewrap for required-app network isolation. Tor defaults to On demand.
See [Tor service](../tor.md) for Settings, app manifests and troubleshooting.
