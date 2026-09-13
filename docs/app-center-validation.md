# App Center stabilization audit

Completed 2026-09-12. Shell baseline:
`02df65ba08e694777031fe3bd7a0b8274ddf4e45`. Publisher checkout:
[`cd1cbf913044bfe7edd3e2ade656a85b90b06e9c`](https://github.com/csd113/Vitrallis-Apps/commit/cd1cbf913044bfe7edd3e2ade656a85b90b06e9c).
The stabilization audit changed no dependencies or neighboring publisher checkout.
The subsequent, separately authorized release updates the workspace version to
`0.1.0-beta2.6` and publishes this work; the validation below records the audit pass.

There was no existing Docker simulator. This pass adds a reproducible Linux SDL2,
Xvfb, Openbox and Python/Tk environment and uses it for the real shell, installer,
filesystem, discovery, menu activation and launched processes. See the
[runner instructions](../tests/simulator/README.md).

## Reproduced causes

| Symptom | Cause and evidence | Result |
| --- | --- | --- |
| Catalog disappears | The worker cleared all rows after an installation attempt; the screen also cleared them on repository checks and reset selection/position when replacing rows. The original binary's post-install empty catalog was reproduced in Docker. | Mutations update one local row. Existing remote catalogs and browsing state remain available; explicit Refresh alone fetches repository metadata. Docker install/update/remove sequences retain the catalog without manual rechecks. |
| Carousel absent from Apps | A post-install refresh reloaded the entire shell catalog, including unrelated PocketHome configuration. A device-menu read/parse failure discarded the refresh despite a valid Carousel installation. Corrupting that fixture configuration after startup reproduces the missing live tile on the original binary. | Installed-app discovery now refreshes independently of device configuration. The same Docker failure condition immediately registers and launches Carousel after installation. Actual published Carousel also installs and opens its real Tk window. |
| Debug uses old entry after update | The installer retained any executable launcher, even when the new manifest changed the entry. It also retained obsolete receipt-owned source files. The original Docker binary updated the receipt/manifest to 0.2.0 and `start.py`, but its launcher still executed `main.py` and printed `Debug OLD ENTRY 0.2.0`. | The managed launcher is regenerated from the current entry/runtime/commit and obsolete owned files are removed. The Docker update launches `start.py`, records version 0.2.0 from the actual process, and confirms obsolete files are absent. |

The original physical device's files and logs were unavailable. A correctly
packaged Carousel already registered on a clean baseline installation. Thus the
menu failure condition above is demonstrated, but cannot be asserted as the
historical device incident's cause. Likewise the reproduced stale-entry bug is
established; the exact cause of the user's original Debug observation remains
unconfirmed. Published Debug 0.1.2 changes release metadata/changelog, with unchanged
runtime behavior, so its appearance alone is not evidence of stale execution.

## Additional findings and changes

- Running-app detection used the available entry rather than the installed entry.
  Entry-changing updates could miss a live old process. Docker now verifies that
  Cancel leaves the old process running, while Close and update stops that exact
  process before installing and launching the new entry.
- Python can accept old bytecode after a same-size source replacement, including
  unchecked-hash caches and interpreter-wide cache locations. Generated launchers
  now use a commit-specific cache namespace and disable bytecode writes; updates
  transactionally remove caches for managed modules. An execution regression runs
  old code, creates stale bytecode, updates, and verifies new output on macOS and
  Docker Linux. This is an additional stale-code risk, not a proven cause on the
  original device.
- Pinned Git executable modes were discarded. Acquisition now retains those modes;
  a regression installs and executes a packaged helper in Linux.
- A final marker/journal cleanup failure could occur outside successful write
  handling. Final output bytes, modes and removals are now read back before
  completion, with finalization included in rollback. Injected write/finalization
  failures verify rollback; corruption, unsafe paths, local edits and unrelated
  data protection remain covered.
- Repository failures replaced useful state, and one malformed app rejected all
  apps in its source. Each source now has an independent validated disk snapshot;
  failed refreshes retain its last good entries. Individual malformed apps become
  diagnostics. Invalid overall structure or duplicate IDs reject the new snapshot.
- UI updates reset navigation and lost useful failure context. Selection, search,
  filter, list position and Details scroll now persist where applicable. Errors
  remain attached to the affected app. Worker disconnects preserve the catalog and
  offer reconnection by reopening App Center.
- Hidden selections could expose active app actions after filtering. Actions are
  disabled while the selected app is excluded; its selection remains available
  when the filter includes it again. Docker checks the empty Updates state after
  a successful update.

## State and lifecycle design

Remote catalogs, local installed status, operation progress/cancellation, search
and filter, selected app/Details, and errors have distinct state. Opening an
existing App Center sends a local Scan. Restarting loads validated saved catalogs
and reconstructs installed status. Neither path contacts repositories. Successful
and failed mutations refresh only the affected row and installed-app discovery.

Acquisition still uses the selected package's exact repository, commit, directory
and complete inventoried files, with bounds, TLS, SHA-256 and manifest agreement
checks. There is no mutable archive cache. Package bytes are acquired again for an
update and verified against its own pinned inventory. Local edits and publisher
switches fail closed. User-owned files and app-local virtual environments remain
outside package replacement; removals touch only validated owned files/support
metadata and matching managed shortcuts.

File writes and removals are journaled, staged, synced, individually atomically
renamed, and verified before success. A pending marker excludes incomplete apps
from discovery and launch. Recovery preserves later user edits. This is a
recoverable multi-file transaction, not an atomic directory swap. Edited custom
launchers are protected with an actionable failure rather than overwritten.

The live Apps menu refresh preserves selection by ID and retains native/device
entries. Open uses the existing shell launch/process path. Running applications
must be explicitly closed before replacement; updates do not silently resume the
old process.

## Changelog, UI and performance

The publisher already inventories `CHANGELOG.md`, so no catalog/manifest version
change or separate release-note endpoint was needed. Explicit Refresh fetches the
pinned, size/hash-verified file once and caches it by content hash. UTF-8 text is
bounded to 64 KiB, preserves multiline sections, and is shown newest first according
to the publisher's dated-heading convention. Details exposes **What's New** before
Update. Missing, invalid, oversized or control-filled notes show a readable empty
state. Opening Details/What's New performs no network requests. The exact format is
in [the App Center guide](app-center.md#changelog-and-icon-contract).

The list now gives each app a clear icon/name, short description and status/version
line. Search is visible beside All/Installed/Updates filters and matching counts.
Details separates description, versions/status, repository and requirements, with
primary Install/Update/Repair/Open, What's New, and a separate Remove action.
Long content scrolls; errors and no-results states explain the next useful action.
Repository management uses the same row/button presentation and shows source errors.
Progress stays associated with the operated app; browsing unrelated entries remains
available while conflicting mutations are disabled. Removal and process-close
confirmations default to Cancel. Keyboard/keypad, Tab, mouse and touch share visible
targets and activation behavior.

Presentation data is fetched only on explicit remote refresh and reused by hash.
Icons are decoded and reduced to 32×32 off the UI thread. Notes and icon pixels are
shared with `Arc`; UI rows omit the package file inventory instead of cloning it.
Rendering performs no file reads or PNG decoding. Local scans remain local, and a
single-app mutation does not download/reparse or rebuild the remote catalog.

Manual screenshot review covered the actual Linux SDL catalog, Details, current and
older release notes, source errors, search/editor, progress, confirmations, failures,
empty Updates, offline restart, live Apps menu and published Tk windows. Device-size
480×272 and scaled 800×480 screens were reviewed. Render tests additionally cover
320×200 and 1280×720; input tests exercise keyboard/keypad and mouse/touch targets.

## Docker scenario coverage

The lifecycle runner reports 20 fixture scenario groups; the published-app pass
reports three more. Grouped below are all 30 requested checks. Every row passed.

| Requested checks | Executed path |
| --- | --- |
| 1–3 | Fresh shell/App Center, explicit repository load, stable populated list during interaction. |
| 4–8 | Install Debug and Carousel; retain catalog/search/selection; immediately activate Carousel from the live menu; assert actual executed canonical script path. |
| 9–15 | Run intentionally old Debug 0.1.0, detect 0.2.0, view cached current/history notes, update, inspect receipt/files, remove obsolete source, launch and assert new version/entry output. |
| 16–18 | Cancel-default removal, confirmed removal, immediate App Center state and live menu removal. |
| 19–21 | Failed download preserves other installs; corrupt update preserves and launches the old version; unsafe removal leaves an unrelated app byte-for-byte intact. |
| 22 | Failed repository refresh retains known-good catalog entries. |
| 23 | Install A → Install B → Update A → Remove B → Install C, with menu and catalog checks throughout. |
| 24 | Search/selection survive installs, Updates remains active after update, and zero matches disables hidden app actions. |
| 25 | Request-log assertions show local scans, reopen, mutations and Details do not refresh remote repositories. |
| 26–28 | Current and historic multiline notes, missing notes, malformed UTF-8 notes, readable Details in each case. |
| 29 | One malformed app and one unavailable configured repository leave valid apps usable. |
| 30 | Reopen and restart offline, reconstruct installed status from canonical files/receipts and saved catalog without remote requests. |
| Additional | Cancel/confirm running old entry; broken device-menu configuration during Carousel installation; actual published Carousel installation/window/removal; published Debug 0.1.1 → 0.1.2 with fresh process and verified metadata/files. |

`baseline.py` separately reproduces the original catalog wipe, missing live menu
under device-config failure, and old launcher after an entry-changing Debug update.
Its two scenario groups pass by asserting the old failures. Current lifecycle
checks assert the corrected behavior.

## Automated regressions and exact checks

Seven new lifecycle tests cover entry-changing update and launch/removal; stale
bytecode execution; worker catalog persistence over sequential mutations and local
scans; per-source saved snapshot and refresh failure isolation; bad app/multiline
changelog validation; executable file modes; and presentation-cache request reuse.
Two browsing tests cover mutation-preserved query/filter/Details scroll and
search/filter/hidden-selection behavior. A transaction regression covers failure
at finalization and pending-marker cleanup. Existing navigation, confirmation,
customization and update/failure tests were adjusted to the new behavior.

| Command/check | Final result |
| --- | --- |
| `cargo fmt --all --check` | Passed. |
| `cargo clippy --workspace --all-targets --all-features -- -D warnings -D clippy::all -D clippy::pedantic -D clippy::nursery -D clippy::cargo` | Passed, with `--locked` in the validation script; no lint allowances added. |
| `cargo test --workspace --all-features` | Passed on macOS and Docker Linux: 187 unit tests plus 6 desktop integration tests; 0 failures, 1 explicit online test ignored, 0 doc tests. |
| `cargo test --workspace --all-features app_center` | Passed: 52 tests; 1 explicit online test ignored. |
| `sh scripts/validate.sh` | Passed: formatting, locked check/strict Clippy/full Rust suite, 77 Python tests, release build of all workspace binaries, SDL shell/native-app smokes, shell syntax, Python compilation, local documentation links and diff whitespace. |
| Docker `sh tests/simulator/run.sh`, with `VITRALLIS_TEST_PUBLISHED=1` | Passed: full Rust suite, 20 fixture and 3 actual-published scenario groups. |
| Docker `python3 tests/simulator/baseline.py` | Passed: 2 original-failure reproduction groups. |
| Docker `VITRALLIS_QA_DIR=/sim/artifacts/render cargo test --workspace --all-features system_panels_render_at_device_and_scaled_sizes` | Passed: 1 render test, screens generated at four sizes. |
| `cargo test --workspace --all-features published_catalog_packages_match_the_current_contract -- --ignored --nocapture` | Passed: 1 real online test verifying Bitcoin 1.2.4 (9 files), Debug 0.1.2 (10), Carousel 0.1.2 (18), pinned source `308a1ab569a86f2104d9f0675a42e963e7d7147e`. |
| Publisher `python3 tools/validate_catalog.py --catalog apps.json --package apps/vitrallis-debug --package apps/vitrallis-media-carousel --package apps/bitcoin-dashboard` | Passed at the reviewed publisher checkout. |
| `git diff --check` | Passed. |

Generated logs and screenshots are retained locally under
`target/app-center-audit/`: `validation.log`, `docker-validation.log`,
`baseline-validation.log`, `online-catalog.log`, and `docker/` including scenario
JSON, HTTPS request logs, process logs, screenshots and render images. Artifacts
are ignored by Git; the runner and fixtures are the reproducible evidence. Published
Carousel's test environment records Pillow 12.3.0 and packaging 26.3 in
`docker/published-runtime.txt`.

## Changed files

| Files | Purpose |
| --- | --- |
| `src/app_center/cache.rs` (new) | Per-source snapshots, isolated parsing, verified cached changelog/icons. |
| `src/app_center/mod.rs` | Remote/local/targeted worker operations and installed entry process checks. |
| `src/app_center/metadata.rs` | Description/presentation metadata, isolated catalog entries, lightweight UI copies. |
| `src/app_center/network.rs` | Catalog documents, pinned Git modes, contextual acquisition errors. |
| `src/app_center/install.rs` | Current launchers, obsolete files/cache removal, protected ownership and permissions. |
| `src/app_center/runtime.rs` | Exact release launcher and Python cache/environment handling. |
| `src/app_center/transaction.rs` | Final readback, rollback-aware finalization and its regression. |
| `src/app_center/uninstall.rs` | Shared owned-path validation, installed-entry lookup, verified commit. |
| `src/app_center/discovery.rs`, `src/ui.rs` | Independent live installed-app refresh and Open integration. |
| `src/app_center/screen.rs`, `src/renderer/app_center.rs` | Browsing/state/interaction redesign, Details/What's New, readable rows and render QA. |
| `src/app_center/tests.rs`, `src/app_center/lifecycle_tests.rs` (new) | Updated existing tests and lifecycle execution regressions. |
| `tests/simulator/Dockerfile`, `start.sh`, `run.sh`, `repositories.py`, `lifecycle.py`, `published.py`, `baseline.py`, `README.md` (all new) | Disposable Linux simulator, HTTPS fixtures, real UI/process scenarios and repeatable instructions. |
| `docs/app-center.md`, `docs/app-center-validation.md` | Current behavior/format guide and this audit report. |

## Remaining limits

No known failing software checks remain in this pass. Docker validates Linux
software paths, not physical the target device touch, ARMv7 responsiveness, battery/display
hardware, hardware media decoding or full Carousel media/server functionality.
Published apps were opened and their actual script processes verified; this is not
exhaustive testing of those apps. No ARM/device performance result is claimed.

App Center still requires a working Python/Tk runtime and declared dependencies;
it never installs those dependencies itself. Downloads may fail while cached entries
remain browsable. Acquisition cancellation can wait for the current bounded curl
request; an already-started filesystem commit finishes or rolls back. Journals and
content-addressed presentation files are retained and currently have no age-based
pruning. Same-user hostile filesystem/process races are outside isolation guarantees.

No obsolete pre-release layout or launcher migration was added. A launcher from a
superseded format may be treated as a local customization and block updating;
current-format installs and their updates are the verified lifecycle. Existing
obsolete installations are neither migrated nor silently deleted.
