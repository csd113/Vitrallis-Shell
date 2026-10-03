# Public-release certification — 2026-10-02

**NOT RELEASE READY. Certification is in progress.** This record distinguishes
published beta testing from development testing and will be updated as the
remaining gates are exercised. The owner authorized preparation of Shell 1.0.0,
Bitcoin Dashboard 1.3.1, Media Carousel 0.4.3 and PocketCHIP Places 0.11.2.

The Shell baseline is `08a204be025d4be7cad07f6fed443a8603441ac3` on
`upgrade/rust-1.99.0`, initially clean. Storage, timeout, native UI,
release-preparation, fresh root provisioning and startup focus corrections are
reviewed in separate commits. The installed native bundle and helpers correspond
to `fe5825bfd516f1f6f55a12ad71002babe91490e6`; the subsequent notice reconciliation
is `caf9293f701970daff0319937dc573a7a553761a`. Both revisions have passing remote
Rust 1.91.0, 1.99.0 and stable checks. The final candidate is not yet established.
[Shell review PR #5](https://github.com/csd113/Vitrallis-Shell/pull/5) remains a draft.
Raw logs, receipts, checksums and screenshots are retained locally under
`target/release-certification/2026-10-02/`; they are not published release assets.

## Device and installation

Physical PocketCHIP connected by USB SSH, `192.168.81.1`, normal desktop user
`chip` (uid/gid 1000), Debian 13.7, kernel `6.12.107+deb13-chip`, X11/Awesome,
480×272. Mesa 25.0.7 reports Mali400 with SDL's hardware `opengles2` renderer,
backbuffering and requested VSync. This proves the selected backend; visual
tearing acceptance remains separate from the owner's keyboard/touch report.

The initial public `v1.0.0-beta-2` user-runtime uninstall/reinstall succeeded,
including a second/idempotent install and reboot. Its bundle digest was
`0f3c5ab0f5ff11ed760d363543cfc5cf6eb219bab147015945bf4fc97f090d55`.
That test retained root GPU/media integration and cannot certify a fully fresh
installation. The stronger test below supersedes its clean-public-install claim.

The complete Vitrallis-owned root GPU/media integration was then inventoried,
verified against its installed bytes, backed up and removed. Both board DTBs were
restored to their verified original bytes. Separately identified obsolete
Vitrallis entries in the user's PocketHome configuration and Awesome home-routing
hook were backed up and removed without changing unrelated desktop entries.
Actual reboots reached the original PocketHome, with no Vitrallis runtime or root
integration remaining. Normal uninstall intentionally preserves user data; the
41-byte saved Notepad note remained private, user-owned and unchanged.

**Fully clean public README installation: FAILED.** The exact current public
README shell command was fetched from GitHub and executed unchanged. It downloads
`https://raw.githubusercontent.com/csd113/Vitrallis-Shell/main/integrations/pocketchip/bootstrap.sh`
and selected the published `v1.0.0-beta-2` release. Download verification and root
setup completed, but normal-user installation failed with permission denied on
`/var/lib/vitrallis-pocketchip/gpu-status.json`. The README command's private
`umask 077` made the root status directory 0700, preventing the desktop user from
reading its 0644 status file. No Shell generation, installation receipt or launch
entry was activated. An initial PTY harness attempt that could not forward sudo
input is separately retained and is not counted as an installer result.

The failed attempt's owned root setup was backed up and removed again. A matching
candidate installer at `93cc413104e64062382ca4b6cfcf2d10ce113f00`, using the same
private umask, then successfully provisioned root integration and activated Shell
1.0.0 from the verified clean state. Dedicated public root status/reader
directories and newly created media-helper directories are now searchable by the
normal user; private root account files remain 0600. Reboot confirmed readable,
healthy Lima/devfreq/trace status. This candidate test used prepared artifacts;
it does not turn the failed published README test into a pass.

That fresh candidate boot exposed a second defect: Shell reached its ready state
but remained behind PocketHome. The corrected `fe5825b` supervisor claims the
actual launcher window through reversible Awesome manage/name events. Its first
real reboot automatically foregrounded the launcher without input; the 480×272
screenshot was inspected. Native Notepad launch/exit and a real launcher title
signal while Notepad was focused passed. Exit Vitrallis restored PocketHome and
removed the owned route. The second reboot reproduced a remaining defect with
no injected input: about 15 seconds after Shell came forward, PocketHome regained
focus. The first observation is an initial-focus pass, not sustained startup
certification. Window-manager event tracing is in progress; startup remains a
release blocker.

The latest complete ARMv7 bundle digest is
`a4a3c315c040d45920e89ec137a9e1456e4cf251ad1fb715c77e7acbb77aca4e`.
The root/focus corrections change its companion installer/session helpers, whose
installed hashes and receipt were independently verified. Normal-user integrity
checks found no unsafe ownership, writable entries or unexpected links in the
installed runtime, canonical AppData and Shell configuration. Current/previous
are the two explicitly validated generation symlinks. The first integrity harness
incorrectly treated their symlink mode as file permissions; that failed harness
result is retained, and the corrected check validates their owner and bounded
relative generation targets. No canonical catalog app payload is installed yet.

Seven actual reboots have completed so far: the initial published-beta boot,
recovery from the development test harness's RAM-backed `/tmp` exhaustion, two
returns to the original desktop during complete cleanup, the corrected root
provisioning boot and two focus-candidate boots. These are software reboots;
no physical cold power cycle has been performed.

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
- Bitcoin and Carousel's scoped source changes are published for review at
  `b1460ed1d7b373719e0edfc1d4fb5fbc9e29e504`, with the separate catalog/history
  commit `2c66c4dfd7741429ef0aebffaf804b2e992a5e59`, in
  [Apps draft PR #18](https://github.com/csd113/Vitrallis-Apps/pull/18). Catalog
  pins match the published payload bytes; every other catalog entry and all
  installable flags are preserved. Changelog policy and both Python CI jobs pass.
  Public main is unchanged. Coordination preserves the other Apps writer's work;
  that writer also recorded the owner's Codex code/artwork confirmation.
- PocketCHIP Places 0.11.2 is committed locally as
  `aac179bc02b1cf09711de698617171d900022173` on
  `codex/pocketchip-appdata-certification` in the isolated pinned-source checkout.
  Modern Places work is untouched. The source is published for review in
  [Places draft PR #2](https://github.com/csd113/Places/pull/2); public main is
  unchanged. Its build cache is excluded from the package and commit. Exact
  source/package/device checks remain separate from the modern renderer work.
- The selected Arti ARMv7 normal/build graph contains 428 distinct package
  versions. Fifteen missing inventory entries were reconciled against cached
  originals and immutable upstream source revisions; all selected graph entries
  now reference retained notices. Four new verbatim texts preserve their
  copyright holders, and the MPL component includes its unmodified source URL.
  The existing 1,294,896 notice bytes were preserved. This conservative dependency
  graph does not establish which build-only crates are linked. The compiled Rust
  runtime notice inventory is still being reviewed.

## Gate ledger

| Gate | Current result and remaining work |
| --- | --- |
| Repository/device baseline | Passed baseline inventory; complete component review continues |
| App filesystem contract | Implemented and host-tested; complete physical app lifecycle pending |
| Filesystem/permissions | Fresh root directory modes repaired and normal-user runtime integrity passed; app and physical fault matrix pending |
| Complete clean public installation | Fully clean published Beta2 FAILED on root status permissions; matching corrected candidate installed; public corrected route pending |
| Installer failure cases | Canonical fixtures pass; physical fault matrix pending |
| Uninstall/reinstall with real app data | Initial removal/reinstall passed; complete persistence sequence pending |
| 480×272 UI | Core Settings/launcher screenshots reviewed; remaining utilities, App Center and failure states pending |
| Keyboard/touch | Physical touch and keyboard accepted by owner; synthetic routes exercised; repeat/stress coverage pending |
| Every catalog app lifecycle | Coherent review source/catalog pins published; public flags preserved; physical runs pending |
| Real data persistence | Host lifecycle passed; physical update/reboot/reinstall sequence pending |
| Python runtime | Host tests passed; current device dependency/lifecycle checks pending |
| Process lifecycle stress | Development sampler found no zombies; utility/app repetition pending |
| Repeated startup | Initial focus repaired; second reboot FAILED sustained foreground focus with no injected input; event tracing in progress |
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
| Public owner documentation | Exact fully clean Beta2 entry FAILED; corrected public candidate entry must be retested |
| License/repository consistency | Artwork and selected Arti dependency notice gaps resolved; compiled runtime and final artifact notices pending |
| Canonical release builds | Shell full host gate passes; ARM development bundle builds/packages; remaining exact-state checks pending |
| Exact clean candidate | Final revision not established; reviewed focus/native state has matching CI and physical evidence; no final tag/release |
| Final physical smoke | Not run |

## Validation recorded so far

Shell `sh scripts/validate.sh` passes on macOS ARM64 and native Linux ARM64 with
the approved 1.0.0 version and reviewed references: formatting, locked
all-target/all-feature check, strict Clippy, workspace tests, Python tests,
release binaries, SDL/native smokes and doc links. The latest runs include the
editor/importer, private-root-umask and real-Lua focus regressions: 361 Rust
tests on macOS, 362 on Linux, and 172 Python tests (nine existing skips on macOS, eight on Linux), plus the native
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
