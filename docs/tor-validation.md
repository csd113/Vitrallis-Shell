# Tor integration validation

Implementation date: 2026-09-19. No Shell version bump, commit, release publication,
or replacement of the device's installed Shell was performed.

## Coverage

Rust tests cover strict manifest parsing (required/preferred/none/absent/invalid),
startup mode persistence and malformed JSON, launch deferral until readiness,
required failure closure, preferred fallback, duplicate starts, on-demand grace,
disabled startup, bounded crash retries, repeated cleanup, exported-launcher
isolation, keyboard selection and matched pointer-release actions. Existing
renderer tests include the Tor panel, details, footer selection and connected,
bootstrapping, disabled/error states at 320×200, 480×272, 800×480 and 1280×720.

Python tests exercise the real guardian against a fake Arti/SOCKS executable:
bootstrap notification parsing, missing/wrong binary, duplicate ownership,
occupied port, repeated start/stop, unexpected exit, Linux guardian death and
restart, symlinks, private files, live isolated SOCKS access, direct IPv4/IPv6/DNS
failure and bounded-relay half-close/backpressure integrity. Installation tests
verify the five-executable inventory and independent Arti version check, rollback,
private default Tor directories/configuration, idempotence and mode preservation.

The eight pre-existing wireless reference images per platform changed only for
the new Tor footer action. All other pre-existing Settings reference hashes
remain unchanged. New Tor references are added rather than weakening the
renderer reference comparison. Images were visually inspected at 480×272.

## Executed checks

| Command/check | Result |
| --- | --- |
| `cargo fmt --all --check` | Passed |
| `cargo clippy --workspace --all-targets --all-features -- -D warnings -D clippy::all -D clippy::pedantic -D clippy::nursery -D clippy::cargo` (also `--locked`) | Passed |
| `cargo test --workspace --all-features` (also `--locked`) | Passed |
| `python3 -m unittest discover -s tests -p 'test_*.py'` | Passed; platform/real-network tests skip where unsupported |
| `sh scripts/validate.sh` | Passed, including release builds, native renderer checks, SDL smoke, shell syntax, Python compilation and documentation links |
| Docker simulator, `tests/simulator/run.sh` | Passed: Rust/Python suites, stock session, App Center lifecycle and desktop shortcuts |
| `cargo test -p vitrallis-shell --all-features idle_loop_stops_after_startup -- --ignored --nocapture` | Passed on macOS and Linux for Home and Tor Settings |
| ARM release Shell and standalone Arti cross-build | Passed |
| PocketCHIP `VITRALLIS_TEST_ARTI=… python3 -m unittest discover -s tests -p test_tor.py` | All 15 passed, no skips |
| PocketCHIP live 480×272 Tor UI | Passed; start, two real restarts, details, stop, mode persistence and disable |

The opt-in idle test initially failed its assumption of at most two startup
frames. A control run without Tor initialization or refresh also produced three
frames, with the final frame at 286 ms. The benchmark now tests the relevant
invariant directly: rendering must stop within the first 500 ms of a two-second
run, for both Home and Tor Settings. It retains that strict settling deadline;
it does not permit continuous redraws. Final host/Linux samples stopped before 310 ms.

The simulator's container denies unprivileged network namespaces, so its
required-network isolation case is skipped. That case ran successfully on the
physical PocketCHIP. The normal suite never requires the public Tor network.
Working logs and screenshots are under `target/tor-qa/` (ignored generated output).

## PocketCHIP

Hardware: USB-connected PocketCHIP, normal `chip` user, Linux
`6.12.107+deb13-chip`, 463096 KiB RAM. Bubblewrap was already installed and its
unprivileged user/PID/network namespaces were verified. ARM Shell and Arti were
cross-built using the existing Debian 12 ARM hard-float toolchain. The Arti build
uses static SQLite; the initial cross-build exposed and resolved an unintended
target SQLite link dependency.

Tests were staged in `/home/chip/vitrallis-tor-qa/`, with a separate HOME and XDG
configuration/data root. The installed generation pointer and Awesome config
were recorded before and after. No existing app, wallet, session configuration,
installed generation, or package was replaced.

The real Arti test bootstrapped and stopped successfully. The required-app test
connected through the shared SOCKS relay while direct IPv4, IPv6 and DNS failed.
The initial device test caught discarded final bytes when a relay endpoint
closed; the fix preserves half-close semantics and drains bounded buffers.
A dedicated 512 KiB bidirectional regression protects the corrected behavior.

Live X11 pointer input opened Settings → Device → Wireless → Tor and exercised
Start, real bootstrap, two Restarts, Details, Stop, Always on and Disabled.
Each restart produced exactly one new Arti PID and removed the old PID. Screenshots show
the 480×272 connected, stopped and disabled panels. A ten-second stopped-panel
measurement consumed 0.07 CPU seconds (~0.7% of one CPU); RSS was about 38.1 MiB
for the complete Shell, including rendering and existing system workers. No
Arti or Tor guardian exists in the idle on-demand state. This is a short device
sample, not a claim about long-term battery consumption. Connected Arti used
about 30.1 MiB RSS with two threads in this sample.

[Connected panel](devices/pocketchip/evidence/tor/connected.png) ·
[Disabled panel](devices/pocketchip/evidence/tor/disabled.png)

Tor's reported Connected state is local bootstrap/proxy readiness, not a
continuous external reachability probe. Unsupported namespace environments fail
closed. Tests do not claim adversarial isolation from the same-user X11 desktop.

## Files changed for this integration

Existing user edits in other validation/release documents were preserved.

- [.github/workflows/shell-release.yml](../.github/workflows/shell-release.yml)
- [docs/devices/pocketchip.md](../docs/devices/pocketchip.md)
- [docs/native-apps.md](../docs/native-apps.md)
- [docs/shell-updates.md](../docs/shell-updates.md)
- [integrations/pocketchip/bootstrap.py](../integrations/pocketchip/bootstrap.py)
- [integrations/pocketchip/bootstrap.sh](../integrations/pocketchip/bootstrap.sh)
- [integrations/pocketchip/install-session.py](../integrations/pocketchip/install-session.py)
- [integrations/pocketchip/uninstall.py](../integrations/pocketchip/uninstall.py)
- [scripts/package-shell-release.py](../scripts/package-shell-release.py)
- [src/app.rs](../src/app.rs)
- [src/app_center/discovery.rs](../src/app_center/discovery.rs)
- [src/app_center/install.rs](../src/app_center/install.rs)
- [src/app_center/metadata.rs](../src/app_center/metadata.rs)
- [src/app_center/tests.rs](../src/app_center/tests.rs)
- [src/lib.rs](../src/lib.rs)
- [src/process.rs](../src/process.rs)
- [src/renderer.rs](../src/renderer.rs)
- [src/renderer/system.rs](../src/renderer/system.rs)
- [src/settings.rs](../src/settings.rs)
- [src/settings/footer.rs](../src/settings/footer.rs)
- [src/settings/geometry.rs](../src/settings/geometry.rs)
- [src/settings/pointer.rs](../src/settings/pointer.rs)
- [src/ui.rs](../src/ui.rs)
- [src/updater/bundle.rs](../src/updater/bundle.rs)
- [tests/bootstrap_fixture.py](../tests/bootstrap_fixture.py)
- [tests/fixtures/renderer/phase1-sha256.json](../tests/fixtures/renderer/phase1-sha256.json)
- [tests/simulator/Dockerfile](../tests/simulator/Dockerfile)
- [tests/test_installer.py](../tests/test_installer.py)
- [tests/test_shell_release.py](../tests/test_shell_release.py)
- [docs/devices/pocketchip/evidence/tor/connected.png](../docs/devices/pocketchip/evidence/tor/connected.png)
- [docs/devices/pocketchip/evidence/tor/disabled.png](../docs/devices/pocketchip/evidence/tor/disabled.png)
- [docs/tor-validation.md](../docs/tor-validation.md)
- [docs/tor.md](../docs/tor.md)
- [scripts/build-arti.sh](../scripts/build-arti.sh)
- [src/renderer/system_tor.rs](../src/renderer/system_tor.rs)
- [src/settings/tor.rs](../src/settings/tor.rs)
- [src/tor/mod.rs](../src/tor/mod.rs)
- [src/tor/sandbox.py](../src/tor/sandbox.py)
- [src/tor/supervisor.py](../src/tor/supervisor.py)
- [src/tor/tests.rs](../src/tor/tests.rs)
- [src/tor/worker.rs](../src/tor/worker.rs)
- [tests/test_tor.py](../tests/test_tor.py)
