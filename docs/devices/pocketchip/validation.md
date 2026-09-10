> Historical Step 2 notes. Current results are in [release-candidate.md](../../release-candidate.md); installation and recovery are in [PocketCHIP installation and recovery](../pocketchip.md).

# Deferred device validation (Step 2)

The user deferred device access for Step 2. Use the current [compatibility report](../../compatibility-step2.md) and README for implemented behavior. The older build notes below remain guidance, not executed commands. Before live testing, compare `--list-apps` with the installed Marshmallow Apps pages, including order, paths, args and icons. Exercise every page with keyboard and physical touch, representative installed apps, and repeated launch/exit cycles while measuring processes, RSS and idle CPU. Verify Marshmallow normally launches afterward and its files/startup remain unchanged.

# Deferred PocketCHIP build and manual validation

No instructions in this document were executed against a PocketCHIP. No USB/SSH/device access is needed for the current desktop implementation. Marshmallow remains installed and its startup unchanged.

## Build preparation, entirely off-device

The expected CPU/ABI target is `armv7-unknown-linux-gnueabihf` for the original ARMv7-A hard-float PocketCHIP. Rust's [ARMv7 Linux target documentation](https://doc.rust-lang.org/rustc/platform-support/armv7-unknown-linux-gnueabi.html) describes the target. Toolchain and native-library runtime requirements must be checked against the intended saved firmware/sysroot; using a modern host's SDL/glibc does not establish compatibility with an older target image.

A future cross-build needs an ARM Linux linker plus an offline sysroot containing target headers, libc, SDL2 and its required native libraries. Obtain these from saved firmware/build artifacts or archived development packages, not live-device probing in this task. Use matching SDL headers/libraries and confirm the linked SDL APIs exist on the target. This repository dynamically links SDL; it does not download/build a bundled SDL or install device packages. An old installed SDL may need a separately built compatible runtime in a later authorized step.

Illustrative build on a **Linux cross-build host** after preparing `/opt/pocketchip-sysroot` and an `arm-linux-gnueabihf-gcc` configured for that sysroot:

```sh
rustup target add armv7-unknown-linux-gnueabihf
export CARGO_TARGET_ARMV7_UNKNOWN_LINUX_GNUEABIHF_LINKER=arm-linux-gnueabihf-gcc
export PKG_CONFIG_ALLOW_CROSS=1
export PKG_CONFIG_SYSROOT_DIR=/opt/pocketchip-sysroot
export PKG_CONFIG_LIBDIR=/opt/pocketchip-sysroot/usr/lib/arm-linux-gnueabihf/pkgconfig:/opt/pocketchip-sysroot/usr/share/pkgconfig
export PKG_CONFIG_PATH=
export RUSTFLAGS='-C link-arg=--sysroot=/opt/pocketchip-sysroot'
cargo build --locked --release --target armv7-unknown-linux-gnueabihf
```

These commands are guidance, not a completed cross-build. Do not substitute host SDL libraries through pkg-config. Do not use `target-cpu=native` on the development machine for an embedded build. On macOS, use a prepared Linux build environment or a properly configured ARM Linux cross-toolchain, not Apple's system linker.

Before eventual staging, inspect the artifact **on the build host** with `file`, `readelf -h`, `readelf -d` and `readelf --version-info`. Confirm ARM hard-float ABI, loader, libc symbol versions, native dependencies and SDL symbols against the offline image. This was not possible here because no compatible target sysroot was provided. The desktop host build is not a PocketCHIP binary.

## Exact future manual launch

After a later explicitly authorized staging step places the compatible executable at `/opt/vitrallis/vitrallis`, run from a terminal in the existing logged-in X11 session:

```sh
SDL_VIDEODRIVER=x11 /opt/vitrallis/vitrallis --pocketchip
```

Alternatively stage `devices/pocketchip/run-pocketchip.sh` alongside it and run `sh /opt/vitrallis/run-pocketchip.sh`. The script requires an inherited `DISPLAY`, preserves `XAUTHORITY` and the rest of the existing environment, and does not guess display authorization, change cwd, reload keymaps, or alter session startup. Do not run as root. The executable is asset-independent apart from its dynamically linked SDL runtime.

No deployment command or automatic session replacement is included. Keep a separate terminal available to close the test launcher. Start only one Vitrallis instance. Do not treat this shell as a login/authentication replacement.

## Exact later validation checklist

1. **ABI and libraries:** confirm the staged executable starts on the target's libc/loader/SDL2; record versions and startup failures. Confirm software rendering works through X11 without GPU assumptions.
2. **Display:** confirm actual fullscreen dimensions are 480×272; no titlebar/WM overlay or unintended scaling obscures tiles/footer; check text readability and selection visibility.
3. **Keyboard:** exercise all arrows across row/outer edges and Return once/held; check which SDL event the physical Home button actually produces and whether Awesome intercepts it. Confirm no keymap reload is required. Escape/Home currently only clear idle status.
4. **Touch:** verify all six tile centers, edges, gaps and empty/outside regions; confirm orientation, calibration and whether X11 supplies mouse events or SDL finger events. Ensure a tap launches exactly once with synthesized events excluded. Check release/drag behavior and touch while a child runs.
5. **Session policy:** observe focus and stacking when launching an app, pressing physical Home, and closing an app. The current policy requests launcher focus only on child exit; no Awesome/xdotool refocus or suspend/resume integration is implemented.
6. **App paths and arguments:** verify the six assumed `/usr/bin` executable paths against installed applications, especially PICO-8/SunVox wrappers. Confirm working-directory requirements and inherited HOME, DISPLAY, XAUTHORITY, locale and PATH. Update only the PocketCHIP backend for verified differences.
7. **Lifecycle:** test normal exit, nonzero exit, unavailable executable, immediate exit, repeated activation, and long-running apps. Confirm no direct-child zombies. Determine which apps daemonize or fork window-owning descendants; they require later supervision/refocus design before everyday use. Close test apps before closing Vitrallis; orderly shell shutdown kills its owned direct child.
8. **Performance:** measure cold start, resident memory, idle CPU, redraws, input latency and return latency. The 250 ms child wait timeout is a design choice, not a measured target result. Check stdout/stderr volume with real apps.
9. **Coexistence:** verify Marshmallow remains launchable and its session/startup config unchanged. Never enable Vitrallis as the default session during this validation step.

Battery, Wi-Fi, Bluetooth, brightness, volume, power, settings, discovery, store, manifests and SDK work remain outside Step 1.
