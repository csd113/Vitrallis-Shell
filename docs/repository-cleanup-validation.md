> Historical validation record. Application/store formats, paths and test counts below describe earlier implementations. For the supported contract and current results, see [the application-contract audit](app-center-validation.md). Superseded tools mentioned here are no longer shipped.

# Repository cleanup validation — 2026-09-10

PocketCHIP installation/session files and device documentation are now grouped
by responsibility. Runtime behavior and installation destinations are preserved.
The resulting tree and extension guidance are in the
[repository layout guide](repository-layout.md).

## Path map

| Previous path | Current path |
| --- | --- |
| `scripts/install-pocketchip.py` implementation | `devices/pocketchip/install.py` |
| `scripts/vitrallis-session.py` | `devices/pocketchip/vitrallis-session.py` |
| `scripts/run-pocketchip.sh` | `devices/pocketchip/run-pocketchip.sh` |
| `scripts/apply-store-patch.py` | `devices/pocketchip/apply-store-patch.py` |
| `integration/pocketchip-store.patch` | `devices/pocketchip/integration/pocketchip-store.patch` |
| `integration/store-patch-manifest.json` | `devices/pocketchip/integration/store-patch-manifest.json` |
| `docs/session.md` | `docs/devices/pocketchip.md` |
| `docs/device-validation.md` | `docs/devices/pocketchip/validation.md` |
| `docs/settings-expansion.md` | `docs/devices/pocketchip/settings.md` |
| `docs/system-status.md` | `docs/devices/pocketchip/system-status.md` |
| `docs/store.md` | `docs/devices/pocketchip/store.md` |

The original `scripts/install-pocketchip.py` path now contains a small forwarding
shim. All six relocated device files are byte-identical to their originals and
retain Git mode `100644`; their documented interpreter invocation is preserved.

Other changed/new files:

- `.github/workflows/validate.yml`, `scripts/validate.sh`, and the legacy shim:
  canonical integration paths, Python compilation, and moved shell checks.
- `src/config.rs`, `src/layout.rs`, `src/platform/mod.rs`,
  `src/platform/generic.rs`, `src/platform/pocketchip.rs`: dimension-named defaults
  and regression coverage for independent display/device selection.
- `tests/desktop.rs`, `tests/test_installer.py`, `tests/test_session.py`,
  `tests/test_store_patch.py`, new `tests/test_repository_layout.py`: moved
  imports, fixture staging, asset resolution, and source-package checks.
- `README.md`, `docs/app-development.md`, `docs/compatibility-step2.md`,
  `docs/compatibility-step3.md`, `docs/engineering-report.md`,
  `docs/release-candidate.md`, `docs/validation.md`, new
  `docs/repository-layout.md`, and this report: updated references and handoff.

## Compatibility decisions

The canonical standalone installer payload is `install.py` plus adjacent
`vitrallis-session.py`, with the ARM binary supplied as its argument. The legacy
shim locates that implementation in a checkout or beside itself in a standalone
bundle. Tests cover both layouts, missing companions, forwarded arguments and
exit status. No tracked automatic installer-download URL required rewriting;
the shim adds no network downloader. Installation, backups, rollback/repair,
session cleanup, and recovery keep their existing behavior. No general
uninstaller existed or was added.

`--pocketchip` still explicitly selects PocketCHIP hardware/session policy,
fullscreen, and the 480×272 default; generic desktop mode defaults to 800×480.
`--size` selects dimensions independently. Reusable dimension constants and
proportional scaling remain in `src/layout.rs`; SDL input stays independent of
resolution. No profile loader, new backend, or dependency was introduced.
PocketCHIP OS timeout/time-zone handling remains behind the platform interfaces.
Runtime PocketHome asset precedence and embedded `assets/system/` paths remain
unchanged. Shared host tools, including `scripts/build-pocketchip.sh`, stay in
`scripts/`; device-specific installer helpers remain with their implementation.

## Checks and results

The following repository-root commands describe the executed checks. The MSRV
validation was additionally invoked by absolute script path from an unrelated
working directory. Logs remain ignored under `target/repository-cleanup/`.

| Command | Result |
| --- | --- |
| `sh scripts/validate.sh` | Passed with Rust 1.91.1, including the final pass. |
| `RUSTUP_TOOLCHAIN=1.91.0 sh scripts/validate.sh` | Passed from the unrelated working directory. |
| `cargo fmt --all --check` | Passed on both toolchains. |
| `cargo clippy --locked --workspace --all-targets --all-features -- -D warnings -D clippy::all -D clippy::pedantic -D clippy::nursery -D clippy::cargo` | Passed on both toolchains. |
| `cargo test --locked --workspace --all-features` | 65 unit tests and 5 desktop integration tests passed on each toolchain. |
| `python3 -m unittest discover -s tests -p 'test_*.py'` | 33 tests passed. |
| `cargo build --locked --release --workspace --all-features` | Passed on both toolchains. |
| `SDL_VIDEODRIVER=dummy cargo run --locked -- --smoke-test` | Launch, child exit/reaping, and smoke completion passed. |
| `for script in scripts/*.sh devices/pocketchip/*.sh; do sh -n "$script"; done` | Passed. |
| `python3 -m compileall -q scripts devices` | Passed. |
| `cargo package --allow-dirty --locked --offline --target-dir 'target/repository-cleanup/package with spaces'` | Source archive created and rebuilt successfully from a path containing spaces. |
| `git diff HEAD --check` and `git diff --cached --check` | Passed. |

Installer/session tests use synthetic ELF inputs and temporary fixture homes;
no real user installation is modified. Checkout shim, canonical pair, and
standalone legacy bundle all stage successfully from unrelated directories with
spaces in their paths. Session tests check sibling binary lookup and wrapper
environment/cwd/argv/exit forwarding. Four repository-layout tests check source
archive contents and bytes, embedded assets, obsolete-path/cache exclusions,
and cross-build path/environment validation using a mock Cargo executable.

ARM release builds passed with both `RUSTUP_TOOLCHAIN=1.91.1` and `1.91.0`, using
`sh scripts/build-pocketchip.sh`, the existing cargo-zigbuild 0.23.4 and Zig
0.16.0, and `PKG_CONFIG_LIBDIR="$PWD/target/arm-libs/pkgconfig"`. The saved SDL
library is 2.32.4; the script targets `armv7-unknown-linux-gnueabihf.2.36`.
`file target/armv7-unknown-linux-gnueabihf/release/vitrallis` identified a stripped
ELF32 little-endian ARM EABI5 executable with `/lib/ld-linux-armhf.so.3` loader.

The Store workflow's pinned checkout and migrated patch were exercised locally:

```sh
git clone --no-checkout https://github.com/csd113/Pocketchip-update-apps target/repository-cleanup/store
git -C target/repository-cleanup/store checkout --detach 1f394452d6acd124d940154234b0eb8dd7150b70
git -C target/repository-cleanup/store apply "$PWD/devices/pocketchip/integration/pocketchip-store.patch"
python3 -m unittest discover -s target/repository-cleanup/store -p 'test_*.py'
```

All 50 upstream Store tests passed. The local Markdown link audit passed, and
stale-path searches found only intentional migration/history references and
negative regression assertions. Six device file contents/modes were compared
against their original Git versions. No tracked cache, build output, or
credential-pattern match required removal; all 30 intentional evidence files,
the original reference, assets, manifests/lockfile, and `.gitignore` are preserved.

## Validation limits

These are macOS host, fixture, source-package, and offline ARM build results.
The Ubuntu GitHub Actions runner and physical PocketCHIP were not exercised.
Neither `readelf` nor ShellCheck was available; ARM identification used `file`
and shell syntax used `sh -n`. No USB/SSH or Marshmallow access, deployment,
privileged installer execution, system configuration change, toolchain/dependency
upgrade, version bump, commit, push, or release occurred. Cross-linking does not
establish runtime compatibility with another device image.

Detailed local logs: `target/repository-cleanup-host.log` and
`target/repository-cleanup/{host-msrv.log,host-1.91.1-final.log,python-final.log,arm-1.91.1.log,arm-1.91.0.log,package-verify.log,store-tests.log}`.
