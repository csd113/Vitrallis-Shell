# App Manager physical validation — 2026-09-19

Implementation was tested on the USB-connected PocketCHIP after the local and
Linux simulator checks passed. These development-build checks preceded the
`0.1.0-beta3.1` version bump and release commit. The validated development shell
SHA-256 is
`a80372985ba8c59b8175dbe7c702c1ad9df384bc04982bebb033c50349c05166`.

## Deployment and preservation

The ARMv7 hard-float release was cross-built on the development machine using
`scripts/build-armhf.sh` and the device-matching SDL sysroot. The shell, bundled
native applications, ARM test binary and precompiled Rust fixtures were deployed
over USB SSH into
`~/.local/share/vitrallis-validation/app-manager-20260919/`.
The test shell used a separate HOME and XDG data/config tree beneath that directory.
The original shell, current generation, four installed applications and user data
were retained. Before/after hashes of the original generation link, configuration,
app manifests and receipts matched. The test shell was stopped and the original
shell restored to the foreground after validation. No Cargo or Rust compiler was installed or run on the PocketCHIP.

The device initially lacked Python venv/ensurepip. The first dependency attempt
returned the actual Debian prerequisite error and removed the pending environment.
Installing `python3.13-venv`, `python3-pip-whl` and `python3-setuptools-whl` added
three packages (about 3 MB), with **zero upgrades or removals**. This is a related
system prerequisite, not a global installation of app dependencies. Pip installed
app dependencies into each test app's private runtime.

The device had only its USB network route and no working Internet DNS. A temporary
loopback HTTP proxy forwarded Debian/PyPI downloads through SSH to the development
machine; forwarding was removed after testing. No device network configuration or trust store was changed.

## Validation results

| Area | Result |
| --- | --- |
| App Center tests on ARM | 60 passed, 3 opt-in tests skipped in the ordinary run |
| First-time Python dependencies | Three independent app-local venv/pip installs passed; pyfiglet imported successfully |
| Repeated installation checks | Each cold install followed by three idempotent prepare/check operations: nine passed, no false failures |
| Dependency timing | Cold fixture workflows took 189.18, 101.71 and 103.47 seconds, including repeat checks; completion was awaited rather than inferred from elapsed time |
| Footer and Actions | Keyboard Tab/Shift-Tab, arrows, Enter, F10, folder Back; pointer activation of Back and Actions |
| Folders | Create, rename, Cancel-default delete, delete populated folder without uninstalling, move into/between folders and back to Apps |
| Persistence | Folder names and Python membership retained across test-shell restart; persisted state is independent of package receipts |
| Python launch | Dependency-using Tk app launched from a folder; exact script process identified; closed normally |
| Native Rust lifecycle | Precompiled ARM ELF installed with executable permissions, launched from a folder, running update refused, v1 updated to v2, v2 launched, keyboard uninstall completed |
| Uninstall preservation | Native executable and generated launcher removed; unmanaged test user-data file retained |
| Running indicators | Both Python and Rust badges appeared while running and cleared on exit; tile-label pixel crops were identical before/after |
| App Center keyboard | Details, changelog, sources, source editor cancellation, search, paging, reverse Tab and Home exercised; source configuration unchanged |
| Hardware renderer | GLES2/Mali400, accelerated SDL, complete-frame backbuffer, VSync flag enabled; graphics self-test passed with GL swap interval 1 |

The physical display is 480×272. The existing Picom process remained active with
its XRender backend and VSync. Captured settled frames showed legible labels,
visible keyboard focus, the simplified footer and the static running badge.
Screenshots and synchronization diagnostics cannot establish absence of physical
scanout tearing. During a subsequent Shell-only folder, keyboard-focus and
Actions-menu transition demonstration on the physical PocketCHIP, the user
reported **"Shell looks clean"** in response to the explicit tearing, flicker
and missing-controls check. This confirms the observed Shell sequence; it is not
a claim about every application or display mode.

## Performance

Bounded 20-second `/proc` samples of the test shell:

| Sample | CPU, one-core scale | Resident memory |
| --- | --- | --- |
| Initial idle | 0.15% | 39,940 KiB, unchanged |
| After folder/app workflows, idle | 0.20% | 38,736 KiB, unchanged |

These are short workload samples, not a long-duration leak or battery test. A warm
launch of the dependency-using Python/Tk fixture took 7.78 seconds to expose its
window. The first Tk window needed roughly 40 seconds; installation had already
completed successfully. Native fixture launch/update were verified separately.
Cold Python provisioning is substantial work on this hardware, but the shell
continued to accept navigation while provisioning ran. No new animation timer,
background daemon, or continuous idle redraw was added.

## Local and simulator checks

Passed on macOS:

```sh
cargo fmt --all --check
cargo clippy --workspace --all-targets --all-features -- -D warnings -D clippy::all -D clippy::pedantic -D clippy::nursery -D clippy::cargo
cargo test --workspace --all-features
sh scripts/validate.sh
```

The full script passed 242 Rust tests (7 explicitly skipped opt-in tests), the
98-test Python suite (2 skips), release/SDL smoke checks, packaging and Markdown
link validation. Linux workspace tests and the same strict Clippy command passed
(243 Rust tests; Linux-only/opt-in skips differ). The real SDL/X11 simulator passed
20 App Center lifecycle scenarios and 10 desktop shortcut scenarios. The stock
Awesome session suite also passed during the implementation pass.

Hardware test entry points (with isolated fixture locations supplied):

```sh
shell-tests app_center --test-threads=1 --nocapture
shell-tests --exact app_center::tests::physical_python_first_installs --ignored --nocapture
shell-tests --exact app_center::native_tests::physical_native_fixture --ignored --nocapture
vitrallis --graphics-test --renderer hardware
```

`physical_python_first_installs` requires a new isolated `VITRALLIS_QA_HOME`.
The native fixture uses `VITRALLIS_QA_NATIVE_ACTION=install|update|uninstall` and
`VITRALLIS_QA_NATIVE_FIXTURES` containing host-built ARM executables named `v1` and
`v2`. `VITRALLIS_TEST_FIXTURES` points to the copied `tests/fixtures/app-center`
directory. Tests run with a private umask; no system Python packages are replaced.

Raw logs, input scripts, screenshots, device hashes and resource measurements are
retained locally in `target/app-manager-qa/` and on-device in the isolated staging
directory. They are test artifacts and are not part of the release payload.

## Changed files

- `README.md`
- `docs/app-center.md`
- `docs/app-development.md`
- `docs/desktop-shortcuts.md`
- `docs/rendering-performance.md`
- `src/app.rs`
- `src/app_center/discovery.rs`
- `src/app_center/install.rs`
- `src/app_center/metadata.rs`
- `src/app_center/mod.rs`
- `src/app_center/running.rs`
- `src/app_center/runtime.rs`
- `src/app_center/screen.rs`
- `src/app_center/tests.rs`
- `src/app_center/uninstall.rs`
- `src/input.rs`
- `src/launcher.rs`
- `src/layout.rs`
- `src/lib.rs`
- `src/native.rs`
- `src/renderer.rs`
- `src/renderer/artwork.rs`
- `src/shortcuts/mod.rs`
- `src/shortcuts/screen.rs`
- `src/shortcuts/screen_tests.rs`
- `src/test_support.rs`
- `src/ui.rs`
- `tests/fixtures/renderer/phase1-sha256.json`
- `tests/simulator/lifecycle.py`
- `tests/simulator/shortcuts.py`
- `docs/devices/pocketchip/app-manager-validation.md`
- `src/app_center/native.rs`
- `src/app_center/native_tests.rs`
- `src/folders.rs`
