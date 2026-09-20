# Desktop shortcuts validation

Validated on 2026-09-12 using macOS and the existing Linux Docker simulator.
See [desktop-shortcuts.md](desktop-shortcuts.md) for usage and removal rules.

## Requirement audit

| Requirement | Implementation and evidence |
| --- | --- |
| Native creation and editing | Rust editor, executable browser, icon picker/preview, cwd, terminal and explicit shell controls. Docker creates, browses, edits and saves real records; saving and terminal smoke preview leave execution markers absent. |
| Keyboard, mouse and touch | Persistent Add/Actions footer, F2/F10/Menu, right-click and matched touch long-press. Docker verifies no accidental launch from context gestures and completes touch-only creation. Unit tests cover empty-desktop focus, all printable ASCII keys, every editor action, scrolling focus and safe Cancel defaults. |
| Small and larger screens | Render tests generate editor, text, picker and removal screens at 480×272, 800×480 and 1280×720. The final 480×272 text screen and 800×480 editor were visually inspected; Docker operates the real 480×272 UI. |
| Structured launching | Tests execute a PATH-resolved shebang script with spaces, empty arguments, literal `$HOME` and explicit cwd. A separate explicit shell test runs a pipeline/redirection. Docker launches the browsed script and a command through the native terminal as UID 65534 without elevation. |
| Launch errors and cleanup | Tests remove the executable/cwd or revoke permission after saving and assert a dismissible error with the launcher ready. Existing process tests cover immediate exit, return/resume, repeated launch/reap and shutdown. Terminal PTY tests cover output, EOF and reaping; terminal ownership now also cleans its child process group. |
| Independent atomic persistence | Stable namespaced IDs and explicit custom provenance; one bounded atomic JSON record contains both shortcut and copied icon. CRUD tests verify stable edits/restart, failed-save preservation and corrupt-record isolation. Docker deletes the original icon and restarts successfully. |
| Installs, refreshes and updates | App Center discovery preserves custom aliases and consistent ordering across refreshes. Docker installs, refreshes and uninstalls with a custom shortcut present. The real shell generation-switch test asserts the saved shortcut and icon are unchanged after commit. App Center's existing update/rollback suite passes. |
| Custom removal | Unit and Docker checks assert removal deletes the record but preserves the script and output data. Storage derives the deletion path only from validated custom identity; command paths never determine deletion. |
| Managed removal | Desktop requests local receipt identity and opens App Center's existing confirmation/worker/transaction. Docker verifies Cancel, receipt-tamper refusal with unchanged files, and successful uninstall with no catalog refresh. A custom-repository test asserts zero network fetches. Existing running-app and transaction rollback checks pass. |
| Unmanaged removal and authority | Tests verify hiding through per-user state, protected settings/App Center entries, managed hide/remove rejection, ID collisions and independent custom aliases targeting managed executables. Stock-session filters and discovery regression tests pass. |

## Commands and results

The following commands passed on both macOS and Linux:

```sh
cargo fmt --all --check
cargo clippy --workspace --all-targets --all-features -- -D warnings -D clippy::all -D clippy::pedantic -D clippy::nursery -D clippy::cargo
cargo test --workspace --all-features
cargo build --workspace --all-features
cargo build --workspace --all-features --release
```

Each full Rust suite reported **209 passed, 0 failed, 1 ignored**. The ignored
test is the existing opt-in online published-catalog contract check.

Additional Linux simulator commands passed:

```sh
/usr/bin/python3 -m unittest discover -s tests -p 'test_*.py'
dbus-run-session -- /usr/bin/python3 tests/simulator/session.py
/usr/bin/python3 tests/simulator/lifecycle.py
/usr/bin/python3 tests/simulator/shortcuts.py
```

Results: **81 Python tests**, the stock desktop/session launch-resume-close and
restoration scenario, **20 App Center lifecycle scenarios**, and **10 desktop
shortcut scenarios** passed. The shortcut suite was rerun after the final focus
and keyboard changes, including executable browsing.

The final render check passed after creating its output directory:

```sh
mkdir -p target/shortcut-audit/final-qa
VITRALLIS_QA_DIR="$PWD/target/shortcut-audit/final-qa" cargo test --workspace --all-features system_panels_render_at_device_and_scaled_sizes
python3 scripts/check-doc-links.py
git diff --check
```

Local logs and simulator screenshots are retained under the Git-ignored
`target/shortcut-audit/` directory. The simulator reproduction procedure is in
[tests/simulator/README.md](../tests/simulator/README.md).

## Files changed

- User documentation: `README.md`, `docs/desktop-shortcuts.md`, this report.
- Shortcut storage, command parsing, UI and tests: `src/shortcuts/mod.rs`,
  `src/shortcuts/command.rs`, `src/shortcuts/screen.rs`,
  `src/shortcuts/tests.rs`, `src/shortcuts/screen_tests.rs`.
- Desktop integration: `src/app.rs`, `src/lib.rs`, `src/discovery/mod.rs`,
  `src/discovery/executable.rs`, `src/input.rs`, `src/launcher.rs`,
  `src/layout.rs`, `src/native.rs`, `src/ui.rs`, `src/renderer.rs`,
  `src/renderer/shortcuts.rs`.
- App Center identity and existing uninstall integration:
  `src/app_center/discovery.rs`, `src/app_center/mod.rs`,
  `src/app_center/screen.rs`, `src/app_center/storage.rs`,
  `src/app_center/uninstall.rs`, `src/app_center/tests.rs`.
- Native terminal argv and PTY ownership: `apps/terminal/src/command.rs`,
  `apps/terminal/src/lib.rs`, `apps/terminal/src/pty.rs`,
  `crates/vitrallis-native/src/ui.rs`.
- Shell-update persistence regression: `src/platform/update/tests.rs`.
- Simulator: `tests/simulator/shortcuts.py`, `tests/simulator/touch.c`,
  `tests/simulator/run.sh`, `tests/simulator/README.md`.

## Limits of verification

No physical hardware or USB access was used. ARMv7 performance, physical touch
timing and arbitrary third-party program/session requirements remain unverified.
The simulator uses native SDL finger events rather than a physical touchscreen.
Process ownership covers ordinary child process groups; programs that deliberately
detach into independent sessions retain the existing supervision limitation.
Atomic storage uses the existing filesystem sync/rename implementation; this run
did not simulate sudden power loss. No dependencies changed. The implementation
audit preceded the beta2.7 version bump and release commit.
