# Application-contract audit and validation

Completed 2026-09-12 in Vitrallis-Shell. The authoritative Vitrallis Apps checkout
was commit [`a86ae57450d52fd779e0ddcdd794b57213ca8eb8`](https://github.com/csd113/Vitrallis-Apps/commit/a86ae57450d52fd779e0ddcdd794b57213ca8eb8).
Its catalog v1, manifest v1, schema, publication tools, runtime integration guidance,
and migrated Bitcoin package were inspected. Git confirmed the current repository
state; the browser's cached repository overview still showed the superseded import.

## Behaviors removed

- Uppercase `Apps/<name>` source directories alongside canonical lowercase slugs.
- Bitcoin-specific detection, install location, launch filename, desktop filename,
  menu registration/removal, and uninstall without an ownership receipt.
- Python literal and AST version parsers, recognition of a historical source hash,
  foreign updater receipts, and the extra Bitcoin-only source backup.
- Icon selection from a historical icon filename or the first published PNG.
- Catalogs that include development tests in device payloads.
- Python store discovery/suppression rules, private Bitcoin/updater Tk/Tcl library
  discovery, and window-title matching for their wrappers.
- Vitrallis-specific `args`, `cwd`, and `env` extensions to the device menu format.
- Automatic desktop PocketHome discovery, an unused reserved app-directory field,
  and a repository-relative asset search inherited from an older source layout.
- The installer forwarding shim, reviewed Python store patch and hash manifest,
  patch installer/tests, and CI job that fetched and patched the superseded store.

## Resulting architecture

There is one `Package` path through catalog parsing, acquisition, manifest checks,
installation, repair, discovery and uninstall. Source directories are validated as
`apps/<app-slug>` and downloads use the authoritative catalog path, repository and
commit. Device inventories are sorted and exclude app-local `tests/`; other files
cannot be omitted. Manifest metadata and the canonical `icon.png` supply app
identity, launch entry and artwork. Installed apps use the XDG `vitrallis/apps/<id>`
directory, generated launchers and publisher-bound receipts. Unknown local source
is unmanaged; source code cannot establish an installed version or ownership.

Removed the `Package::legacy` discriminator and conditional source-file copy,
legacy installation/uninstall branches, menu-warning result plumbing, unused
runtime environment map and version-inspection output pipe. Renamed the device
`store` module to `recovery`, which describes its remaining purpose. No dependency
or shell release version changes were needed.

Current filesystem checks, bounded downloads, source trust, hash/manifest/icon
validation, local-edit protection, rollback journals, process identity checks,
Cancel-default confirmations, and keyboard/touch actions remain in force.

## Tests changed

Removed tests that asserted successful installation/removal of unreceipted Bitcoin,
optional Bitcoin PocketHome registration/cleanup, old Python version parsing,
extended device-menu fields, store patch application and installer shim layouts.
Updated the catalog fixture to the published manifest-based Bitcoin entry.
Converted shared custom-launcher, pending-marker, stale-plan and unsafe-Git-tree
checks to canonical packages. Current UI fixtures explicitly opt into installable
status instead of inheriting a live publisher's readiness flag.

Added canonical path/mandatory-file/ordered-inventory rejection checks and a
regression proving Python `VERSION` text grants neither version nor ownership.
Device discovery tests cover empty storage, manifest IDs, deduplication and missing
launchers. An install/discovery/launch integration test executes a headless fixture's
custom manifest entry and verifies its identity and icon path. Desktop integration
checks the manifest-only default and the explicitly selected device-menu boundary.
The explicit online test verifies the current published catalog and package bytes
without executing applications or installing them.

## Repository policy

The canonical rule is in root [AGENTS.md](../AGENTS.md), referenced by README and
development/layout guidance. Its exact wording, with line wrapping removed, is:

> Vitrallis is pre-release. Backward compatibility with obsolete pre-release builds is not a project requirement unless explicitly requested. When an internal API, package format, path, configuration format, architecture, or behavior is replaced, update current consumers and remove the superseded implementation. Do not introduce compatibility layers, legacy fallbacks, aliases, dual code paths, migration shims, deprecated formats, or version-gated support unless the task specifically requires them.

## Validation results

| Command/check | Final result |
| --- | --- |
| `cargo fmt --check` | Passed |
| `cargo fmt --all --check` | Passed |
| `cargo clippy --workspace --all-targets --all-features -- -D warnings -D clippy::all -D clippy::pedantic -D clippy::nursery -D clippy::cargo` | Passed |
| `cargo test --workspace --all-features app_center` | Passed: 40 tests at that stage, one explicit online test ignored; the subsequently added launch test passed separately and in the full suite |
| `cargo test --workspace --all-features installed_manifest_supplies_identity_icon_and_launch_entry` | Passed: 1 test |
| `cargo test --workspace --all-features published_catalog_packages_match_the_current_contract -- --ignored --nocapture` | Passed: 1 online test; Bitcoin 1.2.2, 9 files at source commit `c45e941396adc42f5aee5ddaa6342b171a3d3734`, `apps/bitcoin-dashboard`, `installable=false` |
| `sh scripts/validate.sh` | Passed, exit 0: formatting; strict locked Clippy; complete locked Rust suite (145 unit tests, 6 desktop integration tests, 0 doc tests; online test intentionally ignored); 32 Python tests; locked release build; SDL dummy-driver smoke; shell syntax, Python compilation, and diff whitespace checks |
| `python3 -m unittest discover -s tests -p 'test_*.py'` | Passed: 32 tests, including installer/session/release/source-package validation |
| `cargo +1.91.0 check --locked --workspace --all-features` | Passed on declared MSRV |
| ARM Linux strict Clippy command below | Passed: compile/lint only |
| `git diff --check` | Passed |

```sh
PKG_CONFIG_LIBDIR="$PWD/target/arm-libs/pkgconfig" \
PKG_CONFIG_ALLOW_CROSS=1 PKG_CONFIG_PATH= \
cargo clippy --locked --workspace --all-targets --all-features \
  --target armv7-unknown-linux-gnueabihf -- \
  -D warnings -D clippy::all -D clippy::pedantic -D clippy::nursery -D clippy::cargo
```

Initial development runs caught formatting/lint issues, test helpers removed with
an obsolete fixture, fixture readiness assumptions, a test-inventory expectation,
and a macOS test path containing a symlink. These were corrected; the full final
validation script exited successfully. No physical-device test, remote Actions
run, deployment, commit, or upstream edit is claimed.

## Retained current boundaries and limits

- The read-only PocketHome reader is required for the independent PocketCHIP OS
  menu (Terminal, file tools, Wi-Fi command, display preferences). Its JUCE token
  rules, trailing commas and OS asset precedence describe that existing component.
  It accepts OS `name`/`shell`/`icon` items, not a second Vitrallis package contract.
  Only PocketCHIP mode or explicit `--app-config` loads it.
- The device installer/session still registers the shell with Marshmallow and
  restores it during recovery. The calibration window exception and system
  hardware/ABI fallbacks support current device services, not old Vitrallis builds.
- Standard XDG defaults, supported system/virtual-environment Python locations,
  optional host tools, GitHub URL input normalization and broken-image placeholders
  remain current platform/UI behavior. Existing user-customized launchers and
  desktop shortcuts are preserved as user data, not pre-release package adapters.
- Catalog/manifest/settings version validation rejects unsupported contracts;
  shell-release SemVer/prerelease selection is the current updater policy. Neither
  switches to an older application implementation.
- Historical hardware evidence and reports remain as observations supporting the
  retained device integration. They are prominently marked historical and link
  to this report; their former tools and app formats are not shipped or supported.

The published Bitcoin entry remains disabled by its publisher. Source verification
is not a substitute for runtime/device certification. PocketCHIP must provide a
working system Python/Tk or app-local virtual environment. Apps need `_NET_WM_PID`
for automatic process-group window resume; Tk builds lacking it require the window
manager. Physical package launch/return, touch and keyboard verification remain
outstanding. Existing obsolete installations are neither migrated nor deleted.

Final source/tool/CI searches found no Bitcoin/updater-specific paths, legacy
version parsers or compatibility dispatch. Uppercase source paths appear only as
rejected test inputs; removed path names appear in source-package exclusion
assertions and the clearly marked historical reports.

## Files changed

- `.github/workflows/validate.yml`
- `AGENTS.md`
- `README.md`
- `Vitrallis_Project_Reference.md`
- `devices/pocketchip/apply-store-patch.py`
- `devices/pocketchip/install.py`
- `devices/pocketchip/integration/pocketchip-store.patch`
- `devices/pocketchip/integration/store-patch-manifest.json`
- `devices/pocketchip/run-pocketchip.sh`
- `docs/app-center-validation.md`
- `docs/app-center.md`
- `docs/app-development.md`
- `docs/compatibility-step2.md`
- `docs/compatibility-step3.md`
- `docs/dependencies.md`
- `docs/devices/pocketchip.md`
- `docs/devices/pocketchip/store.md`
- `docs/devices/pocketchip/validation.md`
- `docs/engineering-report.md`
- `docs/marshmallow-step1.md`
- `docs/release-candidate.md`
- `docs/repository-cleanup-validation.md`
- `docs/repository-layout.md`
- `docs/validation.md`
- `scripts/install-pocketchip.py`
- `src/app_center/discovery.rs`
- `src/app_center/install.rs`
- `src/app_center/metadata.rs`
- `src/app_center/mod.rs`
- `src/app_center/network.rs`
- `src/app_center/runtime.rs`
- `src/app_center/screen.rs`
- `src/app_center/sources.rs`
- `src/app_center/storage.rs`
- `src/app_center/tests.rs`
- `src/app_center/uninstall.rs`
- `src/config.rs`
- `src/discovery/catalog.rs`
- `src/discovery/mod.rs`
- `src/discovery/pockethome.rs`
- `src/lib.rs`
- `src/platform/mod.rs`
- `src/platform/pocketchip.rs`
- `src/platform/pocketchip/recovery.rs`
- `src/platform/pocketchip/store.rs`
- `tests/desktop.rs`
- `tests/fixtures/app-center/catalog.json`
- `tests/test_installer.py`
- `tests/test_repository_layout.py`
- `tests/test_store_patch.py`

Deleted: `scripts/install-pocketchip.py`,
`devices/pocketchip/apply-store-patch.py`,
`devices/pocketchip/integration/pocketchip-store.patch`,
`devices/pocketchip/integration/store-patch-manifest.json`, and
`tests/test_store_patch.py`. `src/platform/pocketchip/store.rs` was renamed to
`src/platform/pocketchip/recovery.rs`.
