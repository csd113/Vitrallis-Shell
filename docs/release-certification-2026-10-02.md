# Public-release certification — 2026-10-02

**NOT RELEASE READY. Certification is in progress.** This record distinguishes
published beta testing from development testing and will be updated as the
remaining gates are exercised. The owner authorized preparation of Shell 1.0.0,
Bitcoin Dashboard 1.3.1, Media Carousel 0.4.3 and PocketCHIP Places 0.11.2.

The Shell baseline is `08a204be025d4be7cad07f6fed443a8603441ac3` on
`upgrade/rust-1.99.0`, initially clean. Reviewed storage, timeout, native UI and
release-preparation changes are recorded in coherent local commits. The exact
candidate still requires its remaining physical certification and matching remote CI.
Raw logs, receipts, checksums and screenshots are retained locally under
`target/release-certification/2026-10-02/`; they are not published release assets.

## Device and installation

Physical PocketCHIP connected by USB SSH, `192.168.81.1`, normal desktop user
`chip` (uid/gid 1000), Debian 13.7, kernel `6.12.107+deb13-chip`, X11/Awesome,
480×272. Mesa 25.0.7 reports Mali400 with SDL's hardware `opengles2` renderer,
backbuffering and requested VSync. This proves the selected backend; visual
tearing and physical touchscreen acceptance remain separate checks.

The installed beta was inventoried, stopped and uninstalled. Remaining modified
pre-release runtime/configuration was preserved outside its active paths. The
exact public README command downloaded
`https://raw.githubusercontent.com/csd113/Vitrallis-Shell/main/integrations/pocketchip/bootstrap.sh`
and installed public `v1.0.0-beta-2` successfully. Its native bundle digest was
`0f3c5ab0f5ff11ed760d363543cfc5cf6eb219bab147015945bf4fc97f090d55`.
One real reboot reached the supervised Shell normally. A second exact public
install exited 0, preserved the current generation, desktop configuration and
sentinel file, and left no root-owned files in the checked user runtime/config.

**Clean-install limitation:** system packages, the root GPU provisioning and
Carousel media helper/policy were retained by the documented normal uninstaller.
The initial public test therefore certified a clean user installation, not the
required complete removal/reprovisioning of Vitrallis-owned system integration.
That stronger test remains pending. The stable candidate's public route must
also be tested after its exact bytes are available.

The 1.0.0 development bundles were separately installed with their matching local
installers and launched through the normal supervised desktop entry. The bundle
containing the direct Files-to-Notepad correction has digest
`71b2fc2781f31c5868ff7e12b86ccc60b0eccc1878dd4a5405baeaa956e74463`.
The latest complete bundle also includes the footer direction correction:
`a4a3c315c040d45920e89ec137a9e1456e4cf251ad1fb715c77e7acbb77aca4e`.
This is development evidence, not certification of a clean release commit.
A subsequent development reboot recovered the desktop after the first physical
Rust test run exhausted RAM-backed `/tmp`; that harness failure is detailed below.

## Repairs and reviewed evidence

- App Center payloads now use `Documents/Vitrallis/Apps/<id>` and persistent
  state uses private `Documents/Vitrallis/AppData/<id>`. Runtime paths, working
  directory, umask, discovery, uninstall and storage accounting follow the
  [storage contract](application-storage.md). Lifecycle tests retain saved data
  through update, uninstall and reinstall; unsafe AppData fails before mutation.
- Native document-directory creation validates every existing ancestor and
  creates new directories privately. Bitcoin and Carousel use the supplied data
  environment contract with path validation. Places initializes state outside
  its payload; settings/cache durability and safety regressions pass.
- The explicit one-time saved-data importer previews its plan, rejects unsafe
  sources and overlapping destinations, copies privately and publishes without
  overwriting existing AppData. Seven regression tests pass. On PocketCHIP,
  all 215 imported Carousel files (76,339,025 bytes) matched their original
  SHA-256 hashes; imported files were user-owned, single-link and mode 0600,
  with mode 0700 directories. After that check, the owner explicitly authorized
  deletion of Carousel data and Places preferences. Both old and imported data
  copies were removed, including Carousel's cache; app payloads were retained.
  The unrelated saved Notepad test note was verified unchanged.
- A real Files-to-Notepad defect queued the editor request until Files returned
  to the launcher. The UI's Ready-only guard has been removed: the broker
  already defers requests during dialogs and pending actions. A regression
  exercises the Running state and verifies dialog deferral and the exact Unicode
  filename argument. On the physical device, the corrected complete bundle opened
  the saved note directly while Files remained running. Shell owned both utility
  processes; closing Notepad returned to the launcher, and Files resumed its
  previous selection. The 41-byte note survived Shell update and reboot unchanged.
  A second defect made both footer arrow keys move right. Left now moves to the
  previous button, with wraparound. On the latest physical build, F6 selected
  Open, Left wrapped to the visible Close selection, Right wrapped back to Open,
  and Left/Enter closed Files. Its regression and host build also pass.
- Screen-timeout settings reject unsafe paths, create private directories and
  sync after rename. The injected post-commit sync failure keeps the actual timer
  and saved value consistent and reports uncertain reboot persistence.
- The owner's ChatGPT artwork provenance and MIT redistribution confirmation
  are recorded with per-file hashes. This resolves the supplied artwork issue;
  it does not substitute for the remaining dependency/license inventory review.
- The authorized version changes affect exactly four Software Updates frames
  per reviewed OS/CPU profile. All 337 frames were generated for macOS ARM64,
  Linux ARM64, ARMv7 and x86-64; the four changed frames in each were visually
  inspected at 320×200, 480×272, 800×480 and 1280×720. The other 333 hashes per
  profile matched. Only those 16 reviewed hash entries were updated.
- Physical screenshots cover the launcher, Desktop actions, all eight Settings
  categories, Tor and timezone pages, and a power-off confirmation with Cancel
  selected. Early scripted subpage captures that lost foreground tracking are
  excluded. Subsequent automation checks the screen before sending input.
- The owner tried the actual PocketCHIP and reported that touch and keyboard
  work on the development build. A separate automated hardware workflow created
  and saved a 41-byte Notepad note with spaces and punctuation in its filename,
  exercised keyboard Save, and read it back after closing Files. Key-repeat
  stress and explicit visual-tearing acceptance remain separate checks.
- The clock changed to 12-hour time and screen timeout to 1800 seconds on the
  device; both preference files are `chip:chip` mode 0600. After the development
  reboot, the display still used 12-hour time and `xset` reported 1800-second
  screensaver/DPMS timeouts. The remaining exposed settings still need certification.
- A 30-minute development soak collected 61 samples over 1805 seconds with no
  desktop-user zombies. It included an intentional software exit/restart, so it
  does not certify uninterrupted candidate uptime. The restarted Shell used
  37,908–38,164 KiB RSS and averaged 0.77% CPU; Picom used 3612 KiB and 0.11% CPU.
  The session log grew by 18,853 bytes during active testing. Exact-candidate
  stability and repeated application stress remain pending.
- Bitcoin and Carousel's scoped changes were reviewed and committed locally as
  `b1460ed1d7b373719e0edfc1d4fb5fbc9e29e504` in an isolated Apps worktree. Their
  source and catalog updates are unpublished. Coordination preserves the other
  Apps writer's changes; that writer also recorded the owner's confirmation
  that Codex created all app code and assets. Dependency notices still need
  review against the artifacts actually shipped.
- PocketCHIP Places 0.11.2 is committed locally as
  `aac179bc02b1cf09711de698617171d900022173` on
  `codex/pocketchip-appdata-certification` in the isolated pinned-source checkout.
  Modern Places work is untouched. The source is unpublished; its build cache
  is excluded from the package and commit.

## Gate ledger

| Gate | Current result and remaining work |
| --- | --- |
| Repository/device baseline | Passed baseline inventory; complete component review continues |
| App filesystem contract | Implemented and host-tested; complete physical app lifecycle pending |
| Filesystem/permissions | Several guards repaired; full path/owner audit and device fault tests pending |
| Complete clean public installation | User runtime passed for published beta; root integration and stable candidate pending |
| Installer failure cases | Canonical fixtures pass; physical fault matrix pending |
| Uninstall/reinstall with real app data | Initial removal/reinstall passed; complete persistence sequence pending |
| 480×272 UI | Core Settings/launcher screenshots reviewed; remaining utilities, App Center and failure states pending |
| Keyboard/touch | Physical touch and keyboard accepted by owner; synthetic routes exercised; repeat/stress coverage pending |
| Every catalog app lifecycle | Pending coherent published source/catalog pins and physical runs |
| Real data persistence | Host lifecycle passed; physical update/reboot/reinstall sequence pending |
| Python runtime | Host tests passed; current device dependency/lifecycle checks pending |
| Process lifecycle stress | Development sampler found no zombies; utility/app repetition pending |
| Repeated startup | One published-beta and one development recovery reboot passed; repeated exact-candidate boots pending |
| Hardware features | Display/GPU backend observed; radio/audio/backlight/battery/power acceptance incomplete |
| Every setting persistence | Clock format and timeout survived development reboot; remaining matrix pending |
| Offline/network failures | Host fixtures pass; physical offline and recovery checks pending |
| Low NAND / ENOSPC | Host fault tests present; safe physical storage pressure pending |
| Shell update/rollback | Host recovery tests pass; actual candidate device update/rollback pending |
| Security/trust boundaries | Concrete path guards repaired; complete audit/fault matrix pending |
| Failure UX | Core confirmations inspected; deliberately induced device errors pending |
| Files/Terminal/Notepad | Real note save/read, direct editor and footer wraparound passed on device; remaining manual utility cases pending |
| Performance | Development measurements recorded; exact-candidate measurements pending |
| Extended soak | 30-minute development session completed with intentional restart; exact-candidate soak pending |
| Logs | Initial renderer/startup logs inspected; final audit pending |
| Code/documentation hygiene | Storage/provenance docs updated; final sweep pending |
| Public owner documentation | Exact beta entry tested; stable installation claim pending |
| License/repository consistency | Artwork resolved; app/dependency inventory and final pins pending |
| Canonical release builds | Shell full host gate passes; ARM development bundle builds/packages; remaining exact-state checks pending |
| Exact clean candidate | Not established; no tag, public release or matching remote CI |
| Final physical smoke | Not run |

## Validation recorded so far

Shell `sh scripts/validate.sh` passes on macOS ARM64 and native Linux ARM64 with
the approved 1.0.0 version and reviewed references: formatting, locked
all-target/all-feature check, strict Clippy, workspace tests, Python tests,
release binaries, SDL/native smokes and doc links. The latest runs include the
editor regression and importer: 361 Rust tests on macOS, 362 on Linux, and 167
Python tests (nine existing skips on macOS, eight on Linux), plus the native
renderer suite (two tests, one existing skip).
The ARMv7 release build and complete five-executable packaging/version checks
pass against glibc 2.36/SDL2 2.26.5. Bitcoin 1.3.1 has 71 passing tests; Carousel
0.4.3 has 271 passing tests and one existing skip. The isolated Places package's
latest full suite has 843 passing tests and three existing ignored tests; its
settings/ENOSPC regression and cache round trip pass. Its final strict Clippy
includes all, pedantic, nursery and cargo groups, with documented exceptions for
two unavoidable upstream duplicate-crate pairs. Formatting and ARMv7 rebuild
pass. Its 212-file staged package passes the Apps manifest validator, and its
notices record all 39 locked ARMv7 dependencies. Physical package validation and
the compiled-runtime license inventory remain pending.

The physical ARMv7 Shell library suite passes unchanged: 298 tests passed and
nine existing opt-in tests were excluded. The controlled run used a private
NAND-backed temporary directory, one test thread and the stock executable PATH.
All four process tests that fail under QEMU pass on the PocketCHIP. All 41 native
library tests also pass with the device's normal umask 002. Two explicit opt-in
accelerated renderer tests pass against the physical X11/Mesa backend: scene
comparison against software, and atlas preservation after artwork refresh.
Their existing error bounds were preserved.
The physical Files, Notepad and Terminal library suites also pass: 2, 3 and 11
tests respectively, including the corrected footer selection, safe unsaved
defaults and real PTY output/resize/reaping behavior.

The first physical Shell suite failed three tests. Its renderer captures filled
the 227 MiB RAM-backed `/tmp`, and the kernel killed the desktop window manager.
An unrelated custom `/usr/local/bin/vala-terminal` also contradicted a stock
discovery fixture; a short fake-Arti startup deadline failed under resource
pressure. The desktop was recovered by the authorized reboot. The tests passed
unchanged with controlled PATH and NAND-backed temporary storage. The failed run
is retained as failed; production discovery continues to preserve custom entries.

The Linux x86-64 full run under Docker/QEMU passed formatting/check/Clippy but
failed four process tests: emulated spawn error reporting, guest executable
identification through `/proc`, and interrupted socket reception differ from
native execution. All four tests pass unchanged on native Linux ARM64. This
emulated run is retained as failed; it does not certify native x86-64 hardware.

Failures and their causes are retained: artwork-classification fixture glob,
old Places lighting-vector location and scaled artwork expectations, invalid
cross-build tooling setup, and intentionally stale version-label reference
hashes. Assertions were repaired against actual contracts; none of these checks
were suppressed or marked ignored to obtain a pass.
