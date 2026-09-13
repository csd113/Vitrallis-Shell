# Stock session integration validation

Validated from the current working branch on 2026-09-12 (America/Vancouver).
No commit, version bump, publication, USB access or real-device modification occurred.

## Result and scope

The shell uses the `linux_handheld` backend and `--linux-handheld` flag. ARMv7
Awesome helpers live in `integrations/pocketchip/`; cross-building uses
`scripts/build-armhf.sh`. General docs use neutral names and link to device setup.
Historical hardware reports and all 30 original evidence files are under this
device's documentation. Evidence logs and images were compared byte-for-byte with
HEAD; none changed. Historical report prose/commands remain, with repaired links
and an archive-scope note on the earlier source audit.

Stock PocketHome's system config is read without assuming a writable user config.
Its original menus, packages, config bytes and assets are untouched by install/remove.
The modified-launcher appearance import was removed. Installation provides a user-local
launch command and desktop shortcut; stock PocketHome users start it from Terminal.
The bootstrap requires the current `install-session.py` artifact and refuses bundles
with the superseded installer. Existing published beta2.6 assets do not satisfy this
contract; matching release artifacts must be published separately.

Only the PocketHome importer suppresses exact verified utility commands; labels,
translations and icons do not identify duplicates. PATH shadows, argument-bearing
custom commands, other stock apps and App Center packages survive. Native utilities
remain once, including repair diagnostics if a companion executable is missing.

The Awesome hook restores only its displaced bindings and the previous window,
retains concurrently added bindings, and needs no launcher-specific Lua globals.
The Exit Vitrallis tile invokes the owned-session helper, which validates transient
unit/process identity before stopping; it does not directly stop a unit by name.
Existing receipts lacking the current helper inventory fail closed without adopting
or modifying their obsolete layout.

## Requirement evidence

| Requirement | Evidence |
| --- | --- |
| Stock format and executable provenance | [Pinned source audit](stock-source.md); original and current upstream command inventory fixtures |
| Generic desktop without either launcher | `catalog_paths_and_device_session_entries_survive_import_boundaries` starts generic discovery before supplying a launcher config |
| Stock config without modified-launcher files | Simulator uses system config and asserts no `.pocket-home` directory; installer/remover tests pass with no launcher config |
| Renamed/localized entries and repeated refresh | `stock_catalog_refreshes_keep_natives_once_and_preserve_custom_apps` exercises public load/refresh repeatedly with both stock inventories |
| Custom names, arguments, executable shadows | Importer tests and `app_center_utility_commands_and_custom_names_survive_native_registration` |
| Missing native diagnostics | Native registry tests, supervisor missing-companion fixture, simulator removes only its staged Files executable and verifies its repair tile |
| Native launch/Home/resume/close | Real SDL binaries in Xvfb/Awesome; all three applications return Home, resume their same window and close gracefully |
| Original session and binding preservation | Real Awesome fixture checks all original keys, a concurrent extra key, prior focus, original Home callback and repeated restore |
| Safe install/exit/uninstall | Temporary-home tests cover receipt ownership, rollback, interruption, concurrent edits, locks, symlinks/hardlinks, malformed inputs, optional startup removal, exact stop identity and staged commands |
| App Center remains discoverable | Existing Docker HTTPS lifecycle runs 20 scenarios, including immediate menu updates and failed catalog refresh preservation |
| Rename consumers and package contents | CLI integration tests, staged installer tests, cross-build invocation fixtures, release inventory/checksum tests, complete source archive rebuild and Markdown link check |

## Commands and outcomes

- `cargo fmt --all --check`: passed.
- `cargo clippy --workspace --all-targets --all-features -- -D warnings -D clippy::all -D clippy::pedantic -D clippy::nursery -D clippy::cargo`: passed.
- `cargo test --workspace --all-features`: passed. Host total: 194 passed, zero failed,
  one existing opt-in online catalog check ignored.
- `sh scripts/validate.sh`: passed. This also checks/builds the workspace with the
  lockfile, runs 81 Python tests, rebuilds the complete source archive, tests release
  packaging and staged helpers, builds release binaries, runs shell/native SDL smokes
  at 480×272 and 800×480, checks shell/Python syntax, doc links and whitespace.
- `docker build -t vitrallis-app-center-simulator -f tests/simulator/Dockerfile .`: passed.
- The documented simulator `docker run` with workspace and existing Cargo/target
  volumes: passed, exit 0. It runs current Linux workspace tests, all 80 Python tests,
  the real Awesome stock-session scenario and 20 App Center lifecycle scenarios.
  After the final installer receipt guard, all 81 Python tests were rerun successfully
  in the same Docker image; no UI or Rust code changed after the lifecycle pass.
- Final case-insensitive content and filename audit for Marshmallow and PocketCHIP
  spelling/separator variants: passed with only the [documented exceptions](stock-source.md#remaining-brand-literals).
  General docs retain neutral-labelled device links and the repository tree paths.
  Production code has one external calibration-path literal; the stock fixture
  retains the external Help document path. All other matches are device-specific
  documentation, historical evidence, or references to those documents.

Reproducible simulator artifacts are written to `target/app-center-audit/docker/`:
`stock-session.json`, `stock-session-menu.png`, session captures/logs and
`scenarios.json`. Generated artifacts are intentionally ignored by Git.

## Limits

The container uses an actual Awesome 4 instance but has no systemd user manager.
It runs the supervisor directly; systemd launch/stop/cgroup options and ownership
checks are covered by command and temporary-process fixtures. Actual user-manager
cleanup, physical Home/touch delivery, the original launcher's window behavior,
ARM responsiveness, device controls, power-loss durability and installation/removal
on hardware still require fresh validation. No obsolete OS support or lower runtime
requirements were added. No claim of support for another physical platform is made.

## Files changed

The following paths are the full current change inventory. Moved files are shown as
`old → new`; historical evidence is relocated, not deleted.

- `.github/workflows/shell-release.yml`
- `CONTRIBUTING.md`
- `README.md`
- `devices/pocketchip/bootstrap.py` → `integrations/pocketchip/bootstrap.py`
- `devices/pocketchip/install.py` → `integrations/pocketchip/install-session.py`
- `devices/pocketchip/run-pocketchip.sh` → `integrations/pocketchip/run-session.sh`
- `devices/pocketchip/uninstall.py` → `integrations/pocketchip/uninstall.py`
- `devices/pocketchip/vitrallis-session.py` → `integrations/pocketchip/vitrallis-session.py`
- `docs/README.md`
- `docs/app-center-validation.md`
- `docs/app-center.md`
- `docs/app-development.md`
- `docs/dependencies.md`
- `docs/design.md`
- `docs/devices/pocketchip.md`
- `docs/devices/pocketchip/settings.md`
- `docs/devices/pocketchip/store.md`
- `docs/devices/pocketchip/system-status.md`
- `docs/evidence/step3/crashed-app.png` → `docs/devices/pocketchip/evidence/step3/crashed-app.png`
- `docs/evidence/step3/device-access-cleanup.txt` → `docs/devices/pocketchip/evidence/step3/device-access-cleanup.txt`
- `docs/evidence/step3/device-apps.txt` → `docs/devices/pocketchip/evidence/step3/device-apps.txt`
- `docs/evidence/step3/device-bitcoin-install.txt` → `docs/devices/pocketchip/evidence/step3/device-bitcoin-install.txt`
- `docs/evidence/step3/device-boot-ready.txt` → `docs/devices/pocketchip/evidence/step3/device-boot-ready.txt`
- `docs/evidence/step3/device-boot-session.txt` → `docs/devices/pocketchip/evidence/step3/device-boot-session.txt`
- `docs/evidence/step3/device-cold-final-cleanup.txt` → `docs/devices/pocketchip/evidence/step3/device-cold-final-cleanup.txt`
- `docs/evidence/step3/device-cold-final.txt` → `docs/devices/pocketchip/evidence/step3/device-cold-final.txt`
- `docs/evidence/step3/device-controls.txt` → `docs/devices/pocketchip/evidence/step3/device-controls.txt`
- `docs/evidence/step3/device-failures.txt` → `docs/devices/pocketchip/evidence/step3/device-failures.txt`
- `docs/evidence/step3/device-final-state.txt` → `docs/devices/pocketchip/evidence/step3/device-final-state.txt`
- `docs/evidence/step3/device-fresh-bitcoin-launch.txt` → `docs/devices/pocketchip/evidence/step3/device-fresh-bitcoin-launch.txt`
- `docs/evidence/step3/device-key-revocation.txt` → `docs/devices/pocketchip/evidence/step3/device-key-revocation.txt`
- `docs/evidence/step3/device-multi.txt` → `docs/devices/pocketchip/evidence/step3/device-multi.txt`
- `docs/evidence/step3/device-physical-input.txt` → `docs/devices/pocketchip/evidence/step3/device-physical-input.txt`
- `docs/evidence/step3/device-postboot-launch.txt` → `docs/devices/pocketchip/evidence/step3/device-postboot-launch.txt`
- `docs/evidence/step3/device-postboot-resume.txt` → `docs/devices/pocketchip/evidence/step3/device-postboot-resume.txt`
- `docs/evidence/step3/device-postboot.txt` → `docs/devices/pocketchip/evidence/step3/device-postboot.txt`
- `docs/evidence/step3/device-power-and-preservation.txt` → `docs/devices/pocketchip/evidence/step3/device-power-and-preservation.txt`
- `docs/evidence/step3/device-preboot.txt` → `docs/devices/pocketchip/evidence/step3/device-preboot.txt`
- `docs/evidence/step3/device-reboot.txt` → `docs/devices/pocketchip/evidence/step3/device-reboot.txt`
- `docs/evidence/step3/device-restarts.txt` → `docs/devices/pocketchip/evidence/step3/device-restarts.txt`
- `docs/evidence/step3/device-resume-flow.txt` → `docs/devices/pocketchip/evidence/step3/device-resume-flow.txt`
- `docs/evidence/step3/device-store-tests-final.txt` → `docs/devices/pocketchip/evidence/step3/device-store-tests-final.txt`
- `docs/evidence/step3/device-supervisor-crash.txt` → `docs/devices/pocketchip/evidence/step3/device-supervisor-crash.txt`
- `docs/evidence/step3/final-marshmallow.png` → `docs/devices/pocketchip/evidence/step3/final-marshmallow.png`
- `docs/evidence/step3/fresh-bitcoin.png` → `docs/devices/pocketchip/evidence/step3/fresh-bitcoin.png`
- `docs/evidence/step3/missing-command.png` → `docs/devices/pocketchip/evidence/step3/missing-command.png`
- `docs/evidence/step3/offline-store.png` → `docs/devices/pocketchip/evidence/step3/offline-store.png`
- `docs/evidence/step3/store-installed.png` → `docs/devices/pocketchip/evidence/step3/store-installed.png`
- `docs/history/beta2.6-device-validation.md` → `docs/devices/pocketchip/history/beta2.6-device-validation.md`
- `docs/history/device-validation.md` → `docs/devices/pocketchip/history/device-validation.md`
- `docs/history/marshmallow-reference.md` → `docs/devices/pocketchip/history/marshmallow-reference.md`
- `docs/native-apps.md`
- `docs/native-validation.md` → `docs/devices/pocketchip/history/native-validation.md`
- `docs/releases.md`
- `docs/repository-layout.md`
- `docs/security.md`
- `docs/shell-updates.md`
- `docs/shell.md`
- `docs/validation.md`
- `scripts/build-pocketchip.sh` → `scripts/build-armhf.sh`
- `scripts/package-shell-release.py`
- `scripts/validate.sh`
- `src/app.rs`
- `src/config.rs`
- `src/discovery/catalog.rs`
- `src/discovery/mod.rs`
- `src/discovery/pockethome.rs`
- `src/lib.rs`
- `src/native.rs`
- `src/platform/mod.rs`
- `src/platform/pocketchip.rs` → `src/platform/linux_handheld.rs`
- `src/platform/pocketchip/display.rs` → `src/platform/linux_handheld/display.rs`
- `src/platform/pocketchip/recovery.rs` → `src/platform/linux_handheld/recovery.rs`
- `src/ui.rs`
- `src/updater/tests.rs`
- `tests/desktop.rs`
- `tests/fixtures/app-center/catalog.json`
- `tests/simulator/Dockerfile`
- `tests/simulator/README.md`
- `tests/simulator/run.sh`
- `tests/test_bootstrap.py`
- `tests/test_installer.py`
- `tests/test_repository_layout.py`
- `tests/test_session.py`
- `tests/test_shell_release.py`
- `tests/test_uninstaller.py`
- `docs/devices/pocketchip/stock-source.md` (added)
- `docs/devices/pocketchip/validation-stock-session.md` (added)
- `tests/fixtures/pockethome/current.json` (added)
- `tests/fixtures/pockethome/stock.json` (added)
- `tests/simulator/session.py` (added)
