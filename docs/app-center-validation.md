# App Center handoff and validation

The native Rust App Center fetches the default catalog at runtime and supports
persistent supplemental GitHub catalogs. No app manifest, app source, or app image
is embedded in production builds, and fresh storage has no preinstalled apps.
Only the catalog repository setting and narrow legacy compatibility rules are
built in. App assets are installed only with the selected app.

See [behavior, parity mapping, architecture and limits](app-center.md). The main
changes cover verified package installation/recovery, provenance and source
approval, native discovery, SDL controls, and the existing shell-update route.

## Validation results

| Check | Result |
| --- | --- |
| `sh scripts/validate.sh` | Passed: formatting, strict Clippy, 108 Rust unit tests, 6 desktop integration tests, 38 Python tests, release build, SDL smoke, shell/Python syntax checks, and diff whitespace |
| `cargo +1.91.0 check --locked --workspace --all-features` | Passed on the declared minimum Rust release |
| ARM Linux strict Clippy, all targets/features | Passed using the existing `target/arm-libs/pkgconfig` SDL metadata; compilation/lint only, no device connection |
| Existing renderer QA harness | Passed; App Center list, sources, editor, details and confirmation frames generated; 480×272 and 800×480 frames visually inspected |
| `git diff --check` | Passed |

The ARM command was:

```sh
PKG_CONFIG_LIBDIR="$PWD/target/arm-libs/pkgconfig" \
PKG_CONFIG_ALLOW_CROSS=1 PKG_CONFIG_PATH= \
cargo clippy --locked --workspace --all-targets --all-features \
  --target armv7-unknown-linux-gnueabihf -- \
  -D warnings -D clippy::all -D clippy::pedantic -D clippy::nursery -D clippy::cargo
```

Renderer QA used `VITRALLIS_QA_DIR` with
`cargo test --locked renderer::system_tests::system_panels_render_at_device_and_scaled_sizes -- --nocapture`.
The generated frames are ignored local artifacts under `target/app-center-qa-final`.
Their sample app rows are test data, not shipped applications.

Earlier validation runs found and corrected old catalog-count expectations,
macOS process-query exit races, legacy version text ambiguity, clipped UI labels,
and lint issues. Results above describe the final checks.

## Limits and scope

The initial implementation handoff included no physical-device testing, deployment,
upstream edits, commits, or version bumps. The subsequently authorized
`0.1.0-beta2.1` release is recorded in [release notes](release-candidate.md);
physical-device testing remains pending. App runtime/dependency availability remains a prerequisite;
unsupported dependency declarations are reported rather than globally installed.
Python syntax inspection currently requires UTF-8 source. Ambiguous script paths
on non-Linux process-query backends fail closed. See the guide for recovery
conflicts and trust assumptions; checksums do not provide sandboxing or independent
publisher signatures. A missing legacy Bitcoin icon comes from verified published
imagery (currently a screenshot fallback), while existing working icons remain.

## Files changed

- `Cargo.lock`
- `Cargo.toml`
- `README.md`
- `docs/app-center-validation.md`
- `docs/app-center.md`
- `docs/dependencies.md`
- `docs/devices/pocketchip/store.md`
- `docs/repository-layout.md`
- `docs/release-candidate.md`
- `src/app_center/discovery.rs`
- `src/app_center/install.rs`
- `src/app_center/metadata.rs`
- `src/app_center/mod.rs`
- `src/app_center/network.rs`
- `src/app_center/running.rs`
- `src/app_center/runtime.rs`
- `src/app_center/screen.rs`
- `src/app_center/sources.rs`
- `src/app_center/storage.rs`
- `src/app_center/tests.rs`
- `src/app_center/transaction.rs`
- `src/discovery/mod.rs`
- `src/launcher.rs`
- `src/lib.rs`
- `src/platform/command.rs`
- `src/platform/mod.rs`
- `src/platform/pocketchip/store.rs`
- `src/renderer.rs`
- `src/renderer/app_center.rs`
- `src/ui.rs`
- `src/updater/tests.rs`
- `tests/desktop.rs`
- `tests/fixtures/app-center/app.toml`
- `tests/fixtures/app-center/catalog.json`
