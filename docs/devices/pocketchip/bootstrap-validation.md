# Prerequisite preparation and first-launch validation

The copy-and-paste entry point is maintained in
[`bootstrap.sh`](../../../integrations/pocketchip/bootstrap.sh) and reproduced
verbatim in the [device guide](../pocketchip.md). Tests enforce their equality.
`bootstrap.py` retains the existing same-release download/checksum contract and
adds user-manager validation, space checks and a checked first launch.

## Behavior

- The shell block checks Debian/ARM/PocketCHIP identity, glibc, Awesome, the boot
  script's presence, existing user bus/session and baseline free space before APT.
  The existing platform helper still validates the actual boot script and DTBs
  before any privileged GPU changes; presence alone does not prove compatibility.
- Missing fixed prerequisites are planned through the configured Debian APT
  sources. The plan rejects upgrades, removals and unrelated configuration work.
  Planned versions are pinned. A root-side APT archive hook independently rejects
  unplanned packages, existing packages and pending repairs before dpkg executes,
  including if package state changed after simulation. Privileged processes use
  a clean environment and fixed executable paths.
- Python/Tk/venv/packaging imports, the SDL runtime version and required tools are
  checked before the bootstrap download. The entire block stops on a failed
  package step or download and cleans its temporary directory. Debian package
  installation is not transactional with the later Vitrallis bundle operation:
  installed prerequisites remain after a subsequent failure or Vitrallis removal.
- SSH installation uses the existing, ownership-checked desktop user's bus.
  Missing/insecure runtime directories, unavailable managers and running
  Vitrallis sessions stop installation. Display credentials are not fabricated.
- After installation, a local graphical Terminal can launch automatically.
  Readiness requires a visible Awesome window, the installed generation's exact
  executable, matching process ownership and the expected supervisor parent.
  Startup polling has a 20-second window; individual manager queries also have
  their own bounded deadlines. Failed startup reports that installation succeeded,
  names the log/retry command, and stops only an unchanged owned session.
- SSH, including forwarded X11, and unavailable graphical sessions print the
  on-device launch command. Boot startup remains opt-in, and GPU reboot notices
  remain relevant even when the desktop opens.

## Validation

Host/fixture evidence, 2026-09; not a physical fresh-image test (see Remaining scope).

`sh scripts/validate.sh` passed on the development host, including the required
Rust formatting, strict Clippy and workspace test commands, Python tests, source
archive rebuild, release builds, SDL smoke checks, script syntax and local links.
The focused bootstrap tests cover the literal guide block with mocked OS and
transport boundaries, missing/repeated prerequisites, package policy, partial
transport failure, corruption, installation/removal/reinstallation, SSH forwarding,
startup failure, session races, unsafe user buses and insufficient storage.

The final host Python discovery passed 123 tests with four explicit environment
skips. The bootstrap suites also passed under Python 3.8 as an unprivileged user
in a network-disabled container (33 tests, two opt-in integration skips):

```sh
docker run --rm --network none --user 65534:65534 \
  --mount type=bind,source="$PWD",target=/workspace,readonly \
  python:3.8-slim python3 -m unittest discover -s /workspace/tests -p 'test_bootstrap*.py'
```

The real APT archive-hook test passed in a disposable, network-disabled Debian
container. It installs a generated local package and verifies that a replacement
and an unplanned package both fail before their payloads are installed:

```sh
docker run --rm --network none -e VITRALLIS_APT_FIXTURE=1 \
  --mount type=bind,source="$PWD",target=/workspace,readonly \
  rust:1.91.1-bookworm python3 /workspace/tests/test_bootstrap_apt.py
```

The real Awesome/X11 readiness test passed in the existing simulator image:

```sh
docker run --rm --network none -e VITRALLIS_DESKTOP_FIXTURE=1 \
  --mount type=bind,source="$PWD",target=/workspace,readonly \
  --entrypoint dbus-run-session vitrallis-app-center-simulator \
  -- python3 /workspace/tests/test_bootstrap_desktop.py
```

That test uses Xvfb, Awesome and an xterm fixture named Vitrallis. Window visibility,
PID, executable and parent checks are real; the systemd ownership boundary is
stubbed because the container has no user manager. It is not a real shell or
physical-device installation test. Both integration tests are opt-in and refuse
to run their active fixtures outside a container.

## Remaining scope

No device was modified, no version was changed, and nothing was committed or
published in this implementation pass. The updated public entry point requires
publishing the changed source. The existing beta3.1 bundles and helper inventory
remain consumable; bootstrap changes do not replace them with mixed-release files.

Before advertising this flow, test the exact published block on a fresh supported
PocketCHIP image: missing-package preparation, sudo interaction, local first launch,
SSH installation, GPU reboot, app installation, interrupted/repeated setup and
offline removal. The tests here do not establish compatibility with every modern
Debian image, driver stack or customized boot layout.
