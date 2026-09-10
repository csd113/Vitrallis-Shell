# Step 2 compatibility report

Implementation and host validation: 2026-09-10. The user explicitly deferred actual-device access during this task. No SSH, device probing, deployment, Marshmallow writes or session/startup changes were performed. Device behavior is **not** claimed verified.

## Reference

The inspected source is [Marshmallow dccbd38](https://github.com/o-marshmallow/PocketCHIP-pocket-home/tree/dccbd38dc233f3f45ebd6ea130d6a787268e23f5), with pinned [JUCE 630ab88](https://github.com/juce-framework/JUCE/tree/630ab88f8bf11c8389f99a9ffb6daa3a0e891f14). Local reference checkout was read outside this repository. The earlier [Step 1 audit](marshmallow-step1.md) is historical; this report supersedes its discovery/pagination/icon limitations.

Source evidence: `Source/Main.cpp` selects the user config and creates it from defaults when missing; `Utils.cpp` establishes asset search order; `LauncherComponent.cpp` consumes `pages` named `Apps`; `AppsPageComponent.cpp` accepts string `name`, `shell`, `icon` and preserves insertion order. `Grid.cpp` crosses pages during keyboard navigation. JUCE `StringArray::fromTokens(command,true)` groups double-quoted strings without shell expansion; `juce_posix_SharedCode.h` unquotes only the executable and calls `execvp`.

## Behavior matrix

| Behavior | Step 2 result | Evidence / limitation |
| --- | --- | --- |
| App discovery | Reads user config, then defaults only if absent; gathers all Apps items | Automated precedence, absent/invalid config, no-mutation tests. Does not copy defaults into Marshmallow's directory. |
| Ordering | Preserves page/item order, including repeated commands | Unsorted names and duplicate-entry tests; stable content-derived IDs with duplicate occurrence suffixes. |
| Existing applications | No conversion; string shell commands resolved in PATH order | Pinned reference's six names, executable basenames, arguments and icon paths compared exactly by host script. Installed device set deferred. |
| Commands | Absolute resolved executable; JUCE argument grouping/quote behavior; no implicit shell expansion | Literal metacharacter, quote, missing command and PATH tests. Unmatched double quotes are deliberately rejected. |
| Working directory/environment | Inherits session defaults; optional cwd/env/args extensions; no global mutation | Actual child verifies cwd, env and literal arguments. Future runtime + entry also exercised with a shell script. |
| Missing/broken apps | Keeps missing commands visible; invalid metadata skipped with diagnostics; malformed catalog gives empty usable screen | Parser and launch-failure tests. Repair requires launcher restart; no live reload. |
| Artwork/labels | Actual PNG assets and labels; BMP, bounded decoding, aspect ratio; default/placeholder fallback | Actual reference icons rendered and visually inspected at 480×272 and 800×480. PNG decoding/corruption tests. SVG/JPEG, Lato and full Unicode remain later. |
| Pagination | Six entries per page, partial final page, page count and previous/next controls | Catalogs with 0, 1, 5, 6, 7, 11, 12, 13 and 61 apps tested at three resolutions. |
| Keyboard | Flat left/right and stride-three up/down across pages; stopped outer edges; Page Up/Down added | Automated navigation and partial-page tests. No physical keyboard/device claims. |
| Touch | Shared tile hitboxes, matching press/release, pointer identity, duplicate event filtering | Automated mouse/finger events, page-button and invalid-coordinate tests. Real touchscreen calibration deferred. |
| Focus | Selected app/page retained during launch/return; SDL raise requested on exit | State and SDL smoke tests. Across-restart selection and Awesome running-window focus remain later. |
| Launch lifecycle | Ready → Launching → Running → Ready, dismissible spawn failures, duplicate guard and cooldown | Mock failure, real spawn/nonzero exit, shutdown and 40-cycle tests. One managed app at a time. |
| Cleanup | Reaps direct child; Unix process group cleanup helper; direct child killed/waited on orderly shutdown | Real child tests and shutdown PID absence. Not a subreaper; descendants escaping groups and external single-instance apps remain later. |
| Coexistence | Reads metadata/assets only; no Marshmallow installation/session changes | No device access or deployment. Test source configs checked byte-for-byte unchanged. Normal Marshmallow operation on hardware remains untested. |

## Validation

Required commands (rerun after final code edits):

- `cargo fmt --all --check`
- `cargo clippy --workspace --all-targets --all-features -- -D warnings -D clippy::all -D clippy::pedantic -D clippy::nursery -D clippy::cargo`
- `cargo test --workspace --all-features`

Tests cover discovery/parsing/order, invalid metadata and assets, layout/navigation, keyboard and pointer translation, launch-state guards, runtime/entry/environment/cwd, process ownership and reaping. The desktop integration suite exercises actual SDL rendering, screenshot overwrite protection and a launched demo child. Forty managed launch/exit cycles complete without accumulating owned child handles; this is host evidence, not a measurement of device RSS, CPU or long-term degradation.

The offline reference comparison returned, in order: Terminal, Play PICO-8, Make Music, Get Help, Write, Browse Files. Each imported argument vector and icon path matched the shipped configuration. Their Linux executables are absent on this macOS host, so they were correctly flagged unavailable and were not launched. Representative host children include direct execution, explicit shell runtime, immediate/nonzero exit and long-running shutdown cleanup.

## Deferred scope

Per the user's follow-up, compare the **installed** app set on the actual device later, exercise every physical keyboard/touch page, launch PICO-8/SunVox/terminal/editor/file manager as available, measure repeated-cycle RSS/CPU/processes and confirm Marshmallow still operates normally. ABI/SDL runtime compatibility and hardware Home/WM behavior also need device validation. This task does not implement Store, package installation, native package scanning, system settings or status features.

Final host result: all three required validation commands **passed**; 25 unit tests and 3 desktop integration tests passed. `git diff --check` passed. No commits were created.

## Files changed

- `Cargo.toml`, `Cargo.lock`: JSON/PNG decoding dependencies and resolved versions.
- `src/app.rs`, `src/config.rs`: normalized app model and centralized paths/CLI.
- `src/discovery/mod.rs`, `src/discovery/marshmallow.rs`: separable catalog backend, import and diagnostics.
- `src/platform/mod.rs`, `src/platform/generic.rs`, `src/platform/pocketchip.rs`: platform policy separated from discovery; demos explicit.
- `src/launcher.rs`, `src/navigation.rs`, `src/layout.rs`, `src/input.rs`: pagination, focus, shared hitboxes and guarded interaction.
- `src/process.rs`, `src/ui.rs`, `src/renderer.rs`, `src/lib.rs`: launch lifecycle, cleanup, icons, error panel and application integration.
- `src/test_support.rs`, `tests/desktop.rs`: isolated fixtures and integration coverage; behavior tests also live beside their modules.
- `README.md`, `docs/devices/pocketchip/validation.md`, `docs/validation.md`, `docs/compatibility-step2.md`: usage, evidence and compatibility/deferred-device report.
