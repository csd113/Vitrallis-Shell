# Reversible PocketCHIP installation and session selection

This integration targets the inspected Awesome 4.x session with a systemd user manager. Marshmallow remains installed, running as the fallback home, and the normal boot default. Vitrallis is an additional launch target, not a login manager or an OS replacement.

Build with the pinned Rust **1.91.1** toolchain. A Rust 1.91 MSRV, edition 2024
and resolver 3 apply to every owned workspace package. Host builds require
SDL2 development libraries and pkg-config. For PocketCHIP, use little-endian
ARMv7 hard-float and the actual image's libc/SDL ABI:

```sh
rustup target add --toolchain 1.91.1 armv7-unknown-linux-gnueabihf
# Optional cross-linker on the development host, isolated from global tools:
cargo +stable install cargo-zigbuild --version 0.23.4 --locked --root target/cross-tools
# Install/use Zig 0.16.0 on the host; the script never installs device packages.
PATH="$PWD/target/cross-tools/bin:$PATH" \
  PKG_CONFIG_LIBDIR=/absolute/path/to/private-arm-libs/pkgconfig \
  sh scripts/build-pocketchip.sh
```

The private `sdl2.pc` must declare `Name`, `Description`, `Version: 2.32.4`, and
`Libs: -L/absolute/path/to/private-arm-libs -lSDL2`; the matching `libSDL2.so`
comes from the inspected target image. Keep the copied native library outside
Git. This release was cross-linked using Zig 0.16.0/cargo-zigbuild 0.23.4 and
`armv7-unknown-linux-gnueabihf.2.36`. The script clears host `PKG_CONFIG_PATH` and
requires the explicit private target directory. It cannot authenticate an
arbitrary supplied sysroot: review the input and compare the library SHA-256
with the device. Do not use macOS SDL. Changing `VITRALLIS_ARM_GLIBC` requires
validating the new target image; this Debian 13 result does not prove original
Jessie compatibility.

Copy the ARM binary and both `devices/pocketchip/install.py` and
`devices/pocketchip/vitrallis-session.py` to a staging directory on the
PocketCHIP. Keep the two Python files together: the installer resolves its
session helper beside its own file, independent of the caller's working
directory. After closing a previous Vitrallis session, run as the normal user:

```sh
python3 /absolute/path/to/staging/install.py /absolute/path/to/arm/vitrallis
```

The installer checks the ELF architecture, validates the PocketHome menu and paths before mutation, refuses symlinks/unmanaged or edited installed files, preserves the menu file permissions, refuses edited desktop shortcuts and symlinked backup roots, preserves previous files under `~/.local/share/vitrallis-backups/`, and installs under `~/.local/share/vitrallis/`. It adds an app menu entry and `~/.local/share/applications/vitrallis.desktop`. It does not modify Awesome, greetd, X startup, kernel/input calibration, recovery services, system packages or Marshmallow's binary. The original user config was also copied off-device before testing.

The original `scripts/install-pocketchip.py` entry point remains a forwarding
shim. It locates the canonical installer in a source checkout. For standalone
use, put `install.py` and `vitrallis-session.py` beside the downloaded
`install-pocketchip.py`; the shim does not download or execute remote code.
Both entry points preserve the binary argument and exit status.

## Select Vitrallis or Marshmallow

From a terminal inside the existing graphical session:

```sh
~/.local/share/vitrallis/launch
```

Or select **Vitrallis** from Marshmallow's Apps menu after its menu has been reloaded. No session/boot changes are necessary. The session launcher requires the existing `DISPLAY`, `XAUTHORITY` and `DBUS_SESSION_BUS_ADDRESS`; do not guess another user's X authorization. It starts a single transient user systemd unit and refuses a duplicate active session.

The session temporarily routes Awesome's physical Home/Power key to Vitrallis. It leaves the remaining window-manager shortcuts intact. In Vitrallis, select **Marshmallow** to stop the Vitrallis unit and return to the original home. A terminal can always request the same operation:

```sh
systemctl --user stop vitrallis-session.service
```

The unit uses `KillMode=control-group`, a five-second stop timeout and no automatic restart. Descendants, including applications that leave the launcher's Unix process group, are still in its systemd cgroup. Closing the Vitrallis window, launcher crashes and supervisor crashes trigger cleanup and `ExecStopPost` restoration of the saved Awesome key bindings. Both home launchers remain separate processes. Two rotated session logs at `~/.local/share/vitrallis/session.log` and `.log.1` are bounded to 128 KiB each.

## Optional reversible default

The inspected device uses greetd's `initial_session` to run `startx`; it does not provide a graphical session chooser. Do not replace that working configuration. If you choose to start Vitrallis automatically later, first back up `~/.config/awesome/rc.lua` and append an explicitly marked block after its existing startup code:

```lua
-- BEGIN optional Vitrallis startup
require('gears').timer.start_new(5, function()
    require('awful').spawn({os.getenv('HOME') .. '/.local/share/vitrallis/launch'}, false)
    return false
end)
-- END optional Vitrallis startup
```

Keep the existing `launch_home_screen()` call. The five-second one-shot starts Vitrallis after the existing session starts; failure leaves Marshmallow available and does not relaunch indefinitely. To restore Marshmallow as the only default home, remove precisely that marked block (preserving unrelated later edits), or restore the backup only if no intervening edits need preserving. This opt-in block was not installed during the task; normal boot remains Marshmallow.

## Recovery

If Vitrallis fails, stop its user unit as above; the independent Marshmallow process and USB serial login remain available. Over the inspected USB serial console, log in as the device user and run:

```sh
XDG_RUNTIME_DIR="/run/user/$(id -u)" systemctl --user stop vitrallis-session.service
```

If a window manager restart erased the temporary hook state, its original `rc.lua` still supplies Marshmallow's Home binding. Reboot starts the unchanged original session. If an installer was interrupted, rerun the same reviewed installer; do not remove `.installation-pending` just to launch incomplete files. Preserve `vitrallis-backups` and the original Bitcoin backup until satisfied with the installation.

If later opting into startup, remove the marked optional block over serial before rebooting to recover. No change to boot media, recovery mode, autologin, calibration or SSH authentication policy is required.

See [the compatibility report](../compatibility-step3.md) for what was actually tested, including limits. These instructions do not establish cold-start or physical-key validation without recorded evidence.

## Existing preferences and settings

Vitrallis reads `background` (six uppercase RGB hex digits or a PNG/BMP asset path), `showclock`, `timeformat` (`ampm`), and `cursor` from the PocketHome document. App order follows its Apps item order. Use Marshmallow's existing personalization controls or carefully edit that user document with a backup; Vitrallis refreshes it after app return, and rejects malformed refreshes while keeping the last valid catalogue. It does not overwrite Marshmallow's preferences. A configured `wifiCommand` supplies the Wi-Fi connection manager inside System Settings, using the same validated, shell-free command parser as app entries. The System Settings tile and footer open the same slider/control screen; F1 is unbound. Both touch and keypad use 10% steps, with live drag updates. PocketCHIP brightness maps its native levels 1–10 to 10–100%, keeping the screen lit at the minimum.
