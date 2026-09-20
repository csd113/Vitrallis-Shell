#!/bin/sh
# This entire block is the copy-and-paste entry point in the device guide.
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
vitrallis_packages='curl ca-certificates python3 libsdl2-2.0-0 picom device-tree-compiler python3-tk python3-venv python3-packaging'
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
for vitrallis_tool in curl python3 picom dtc fdtoverlay; do
    command -v "$vitrallis_tool" >/dev/null || fail "Missing runtime tool after preparation: $vitrallis_tool."
done
printf '%s\n' 'Prerequisites ready. Downloading the Vitrallis installer...'
curl -q -fSL --proto '=https' --proto-redir '=https' --max-redirs 5 --connect-timeout 10 --max-time 30 --max-filesize 262144 https://raw.githubusercontent.com/csd113/Vitrallis-Shell/main/integrations/pocketchip/bootstrap.py -o "$vitrallis_tmp/bootstrap.py"
python3 -I "$vitrallis_tmp/bootstrap.py"
)
