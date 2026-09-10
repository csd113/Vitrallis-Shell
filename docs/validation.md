# Step 1 validation report

Validated on the development Mac with Rust/Cargo 1.98.1 and SDL2 2.32.72. No PocketCHIP connection, USB enumeration, SSH, live-device commands, deployment, or startup changes occurred. No commit was created. The original `Vitrallis_Project_Reference.md` was preserved.

## Commands and outcomes

| Command/check | Result |
| --- | --- |
| `cargo fmt --all --check` | Passed after formatting the newly created Rust files with `cargo fmt --all` |
| `cargo clippy --workspace --all-targets --all-features -- -D warnings -D clippy::all -D clippy::pedantic -D clippy::nursery -D clippy::cargo` | Passed, no lint suppressions added |
| `cargo test --workspace --all-features` | Passed: 11 unit tests, 2 integration tests; no failures |
| `cargo build --locked --workspace --all-features` | Passed, development-host binary |
| `cargo build --locked --release --workspace --all-features` | Passed, optimized development-host binary |
| `SDL_VIDEODRIVER=dummy cargo run -- --smoke-test` | Passed: SDL Enter event → spawn → successful exit/reap → rendered Ready state |
| `cargo run -- --smoke-test` and `target/release/vitrallis --smoke-test` | Passed using a real native desktop window |
| SDL dummy screenshots at 480×272, 800×480, 1024×600, 1280×720 | Generated successfully; 480, 800 and 1280 frames visually inspected for layout/text/focus |
| `sh -n scripts/run-pocketchip.sh` | Passed syntax check only; script not executed |
| `git diff --check` | Passed; new untracked text files additionally checked using `git diff --no-index --check` against `/dev/null` |
| Source review for unsafe blocks, `unwrap()`, `expect()`, lint suppressions | None in owned production code |

Unit tests cover malformed app data, duplicate/empty catalogs, invalid configuration, flat-grid navigation, launch guards/recovery, a mock platform profile, four display resolutions, touch/gap/boundary hit testing, input translation and repeated/synthetic event exclusion, bounded BMP headers, structured command arguments/cwd, failed spawn recovery, and a real child returning exit status 7 and being reaped. Integration tests execute the actual binary using SDL's dummy video backend, validate launch/exit logs and rendered BMP dimensions/content, and ensure screenshot output cannot overwrite an existing file.

## Files created or changed

- Changed: `README.md`.
- Created foundation: `.gitignore`, `Cargo.toml`, `Cargo.lock`.
- Created entry/core: `src/main.rs`, `src/lib.rs`, `src/app.rs`, `src/launcher.rs`, `src/navigation.rs`.
- Created layout/input/rendering: `src/config.rs`, `src/layout.rs`, `src/input.rs`, `src/renderer.rs`, `src/ui.rs`.
- Created lifecycle/platform: `src/process.rs`, `src/platform/mod.rs`, `src/platform/generic.rs`, `src/platform/pocketchip.rs`.
- Created tests: `tests/desktop.rs` (unit tests are colocated with their modules).
- Created documentation: `docs/marshmallow-step1.md`, `docs/device-validation.md`, `docs/validation.md`.
- Created future manual-use helper: `scripts/run-pocketchip.sh`.

Generated build output and final QA frames reside under ignored `target/`; temporary reference downloads and early QA frames are outside the repository. No copied third-party assets, device logs, or local configuration are included.

## Limits of this evidence

The project builds and runs on the development host. It has **not** been cross-linked or executed on ARM/PocketCHIP; no matching offline target sysroot was available. Rust 1.85 minimum compatibility is declared but was not independently tested. The real device's ABI/native library compatibility, six executable locations, application cwd/environment requirements, fullscreen stacking, physical Home key, touchscreen mapping and low-resource performance remain unverified.

The Step 1 launcher tracks one direct foreground child; it does not provide cross-process singleton enforcement, running-window refocus, daemon/descendant supervision, authentication, default-session management, app discovery or full Marshmallow parity. Orderly shell exit terminates its direct child. The exact manual launch and later validation checklist are in [device-validation.md](device-validation.md).
