# ARMv7 PocketCHIP-like simulator

A small, deterministic Docker environment that runs the real Vitrallis ARMv7
binaries and test suites under ARMv7 Linux emulation. It complements
`tests/simulator/` (the x86-64 App Center/GUI simulator) by validating the
**software runtime environment** of the known target: ARMv7 userspace, Debian 13
armhf, glibc, SDL2, unprivileged ownership, umask behavior and the production
generation/update/restore filesystem code.

```sh
sh tests/simulator/armv7/run.sh
```

Results are written to `target/armv7-audit/results/` (architecture evidence,
`summary.txt`, per-check logs, screenshots and the Python test output).

## What it does

1. Builds `Dockerfile.toolchain` (host architecture, like the release workflow)
   with Debian's `arm-linux-gnueabihf-gcc`, `libsdl2-dev:armhf` and
   `qemu-user`, then cross-builds the workspace and test executables for
   `armv7-unknown-linux-gnueabihf`.
2. Builds the real release bundle with `scripts/package-shell-release.py`
   (`--target armv7-unknown-linux-gnueabihf --runner /usr/bin/qemu-arm`),
   including session helpers and the five bundled executables.
3. Builds `Dockerfile.runtime`: `arm32v7/debian:trixie` (Debian 13 armhf) with
   the SDL2 runtime and an unprivileged `chip` account (uid 1000), matching the
   device-like `002` umask conventions.
4. Executes, as `chip` and headless:
   - `file`/`readelf`/`ldd` evidence for every ARMv7 executable;
   - `--version` for all bundled executables;
   - SDL dummy-driver shell frames at 480×272 and 800×480 and native-app smokes;
   - every cross-built Rust test executable, including the focused generation,
     update, restore and App Center storage tests under umask `002` and `022`;
   - the real five-executable bundle through the production unpack,
     verification and generation-commit path (`release_bundle_upgrade_probe`);
   - the Python installer/uninstaller/session suites under ARMv7 Python;
   - a persistent simulated `generations/` + `current` layout owned by `chip`.

## Limits (do not overstate)

- Docker's ARM execution is **emulation**, not a PocketCHIP. It does not model
  the Mali-400/Lima GPU, the display controller, VSync, NAND/UBIFS, power
  behavior, USB or exact kernel/device-tree behavior.
- QEMU timings are not PocketCHIP performance measurements. Use them for
  correctness, call counts, process counts and constrained-resource behavior
  only.
- The container's `chip` account is a simulation account; it is not the real
  device account or session.

## Known emulation-only test differences

User-mode emulation runs target processes through `qemu-arm` via binfmt, so
`/proc/<pid>/exe` points at the emulator, and `exec` of a missing file appears
to start before the loader fails. Five tests assert those native exec
semantics and can never pass under emulation (each fails for the reason shown):

- `app_center::native_tests::native_install_launch_process_detection_update_and_uninstall`
  — native process identity; the v1/v2 fixture binaries are prebuilt by the
  toolchain stage so the install and update paths still run before detection fails.
- `app_center::tests::running_app_identity_is_rechecked_and_only_exact_script_is_closed`
  — `/proc/<pid>/exe` is `qemu-arm`.
- `app_center::tests::uninstall_refuses_a_running_app_without_removing_files`
  — `/proc/<pid>/exe` is `qemu-arm`.
- `process::tests::failed_spawn_allows_retry` — exec of a missing file appears
  to start and then fails inside the loader.
- `shortcuts::tests::vanished_executable_cwd_and_permissions_return_to_a_dismissible_error`
  — same missing-executable semantics.

`container-run.sh` runs the full suite and only accepts a failure whose failed
test names are exactly this documented set; any other failure remains a
failure. The list is recorded next to the results in
`results/emulation-artifacts.txt`. These tests still need native/hardware
execution.

## Cost and cleanup

The first run downloads base images and compiles the workspace and Arti for
ARMv7; later runs reuse the `vitrallis-armv7-target` and
`vitrallis-armv7-cargo` volumes. Set `VITRALLIS_ARTI=0` to skip Arti (the
five-executable probe is then skipped).

Containers are `--rm`. Remove the reusable resources with:

```sh
docker image rm vitrallis-armv7-toolchain:local vitrallis-armv7-runtime:local
docker volume rm vitrallis-armv7-target vitrallis-armv7-cargo
```

Each run replaces `target/armv7-audit/results/`; copy it elsewhere if you need
to compare runs.
