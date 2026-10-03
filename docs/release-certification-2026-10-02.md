# Public-release certification — 2026-10-02

**NOT RELEASE READY. Certification is in progress, continuing into 2026-10-03.** This record distinguishes
published beta testing from development testing and will be updated as the
remaining gates are exercised. The owner authorized preparation of Shell 1.0.0,
Bitcoin Dashboard 1.3.1, Media Carousel 0.4.3 and PocketCHIP Places 0.11.2.

The Shell baseline is `08a204be025d4be7cad07f6fed443a8603441ac3` on
`upgrade/rust-1.99.0`, initially clean. Storage, timeout, native UI,
release-preparation, fresh root provisioning and startup focus corrections are
reviewed in separate commits. The installed native bundle and helpers correspond
to `9f6f0d0eb7cea0dc49f5015105c66978daaac031`, including the startup guard at
`70540cb02e7f1268efc141b9f6e72dead32f0142` and runtime notices at
`290ce8836449345ea6df3e17d6875f2fe218681a`. Remote Rust 1.91.0, 1.99.0 and
stable checks pass for that exact head. Its offline diagnostic Details page
and cause-first cancelled-update message passed physical inspection. Complete host validation, ARMv7 packaging and
normal-user installation also passed. The final candidate is not yet established.
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
certification. Window-manager tracing identified the original PocketHome process's
delayed update dialog and subsequent direct X11 focus grab. The first contextual
activation filter at `72fb6821da1bfa479c5a74a2ce284a6fecd5c806` blocked its EWMH
request but failed the actual reboot because the direct focus grab bypassed it.
That failed observation is retained.

The scoped guard at `70540cb` identifies the original desktop using its existing
trusted client, PID and process start time. Temporary activation filters and a
focus callback keep that process from displacing the Shell or the current app;
unrelated windows are unaffected. Two actual reboots then passed 125.35 and
126.37 seconds of idle foreground observation without injected input. Screenshots
confirmed the normal 480×272 launcher. A direct desktop focus attempt while native
Notepad was active kept the editor foregrounded. Exit Vitrallis restored PocketHome,
stopped the user service and removed both owned activation filters (zero remaining
in each context). Final-candidate reboot and cold-power testing remain pending.

The Notepad header candidate's complete ARMv7 bundle digest is
`39d011de974babb7fedd6cc4697e2adec46f3eef260874d999c652ab6d0e0284`.
It includes a one-cell gap between Notepad's title and document name, correcting
the observed joined label without reducing editor rows. The root/focus corrections
change its companion installer/session helpers, whose
installed hashes and receipt were independently verified. Normal-user integrity
checks found no unsafe ownership, writable entries or unexpected links in the
installed runtime, canonical AppData and Shell configuration. Current/previous
are the two explicitly validated generation symlinks. The first integrity harness
incorrectly treated their symlink mode as file permissions; that failed harness
result is retained, and the corrected check validates their owner and bounded
relative generation targets. Debug 0.3.2 is now installed in the canonical
payload directory. The newer refresh-message bundle has digest
`e8a4db2166ade88f7b850972a2e1d8f4d1dcb0415ee3e3fa912030bb2f03c0eb`;
its normal-user installation, sustained software-launch focus, root integration
readability and runtime/AppData ownership checks passed. This is a prepared
candidate installation, not the pending clean public route.
The earlier diagnostic-details bundle has digest
`11b3199f34105ac8e50e8cde91754db955b97058c6e96954d61d2a759a2dd467`.
It passed normal-user installation, a 23.21-second software-launch focus
observation and runtime/AppData integrity checks. Debug's marker, both newer
Notepad saves and the Places source approval retained their exact bytes and
private ownership through the replacement.
The current download-error bundle has digest
`b9c7bf9367ec57bdaaf1fe1396ff29751638be5ebca248b8b3406c4ea65fd0de`.
Normal-user installation under `umask 077`, runtime/AppData ownership checks and
23.95 seconds of software-launch foreground observation passed. The original
saved note remains private and unchanged. These are prepared candidate tests;
the corrected public installation route and final-candidate reboot remain pending.

Eleven actual reboots have completed so far: the initial published-beta boot,
recovery from the development test harness's RAM-backed `/tmp` exhaustion, two
returns to the original desktop during complete cleanup, the corrected root
provisioning boot and six startup-diagnostic/candidate boots. These are software reboots;
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
  final released artifact notices still need verification.
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
- The Notepad header gap was inspected with both a dirty unnamed document and
  a saved filename. Keyboard Save and the Open browser saved and reopened notes
  with spaces, punctuation and `café` in the path; the original note remained
  unchanged. UTF-8 file contents and filenames were preserved. The current bitmap
  font displays an unsupported em dash as `?`; glyph coverage remains limited.
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
  commit `2c66c4dfd7741429ef0aebffaf804b2e992a5e59`, followed by the exact Places
  mirror/catalog publication at `8c0a247d304524552d5cc66d906089214cdefa28`, in
  [Apps draft PR #18](https://github.com/csd113/Vitrallis-Apps/pull/18). Catalog
  pins match the published payload bytes; every other catalog entry and all
  installable flags are preserved. The earlier submission's remote CI passed;
  the published draft tree passed pinned-source validation, changelog policy and
  74 tooling tests locally. Its remote runtime checks then failed on a relative
  Places asset root; that failure is retained. The corrected five-file mirror
  and catalog pin are prepared locally and pass source/package validation and
  the changelog policy against public main. They are not yet published.
  Public main is unchanged. Coordination preserves the other Apps writer's work;
  that writer also recorded the owner's Codex code/artwork confirmation.
- PocketCHIP Places 0.11.2 is committed locally as
  `768abe2015c52940f3c082b94c931c0bf030d718` on
  `codex/pocketchip-appdata-certification` in the isolated pinned-source checkout.
  Modern Places work is untouched. The source is published for review in
  [Places draft PR #2](https://github.com/csd113/Places/pull/2); public main is
  unchanged. Its build cache is excluded from the package and commit. The exact
  source canonicalizes its asset root before comparing the persistent-state
  boundary. This fixes the real CI failure with external Cargo target directories
  without permitting in-package state. The failing case, expanded boundary test,
  full macOS/Linux suites and mirrored CI-context retest pass. Its ARMv7 executable
  SHA-256 is `97f6607e95a63bc26aee5a2b3feb122010038d2ddd20893ecff6f776b168866d`.
  Device checks remain separate from the modern renderer work.
- Debug 0.3.2 installed from the public catalog, launched as `chip` with AppData
  as its working directory, returned to Home and resumed the same PID, then
  exited with its running badge cleared. With Wi-Fi disabled through Settings,
  normal keyboard-confirmed uninstall removed the receipt-owned payload while
  preserving a private Unicode-named data marker. Reconnecting through Settings,
  refreshing, reinstalling and relaunching preserved the exact marker hash.
  All 14 installed payload files match the pinned receipt, with no unreceipted
  files or Vitrallis-service zombies. Its dependency file declares no pip packages;
  the actual runtime is system Python 3.13.5 with Tk.
- Physical offline refresh retained cached entries but exposed misleading
  `Refresh complete` wording. The correction reports an incomplete refresh at
  480×272; successful online retry removes the source-error entry. The regression
  reproduces the old failure with and without a cache and verifies recovery and
  unchanged snapshot bytes. Error Details also hid the cause behind placeholder
  app metadata; the separate correction now exposes the reason and recovery step,
  and passed physical inspection with the actual network failure wrapped inside
  the content area. Source trust for Places was explicitly
  confirmed through the UI, with Cancel as the default and a private 0600 settings
  file. Cancelling the published Places 0.11.1 download left no receipt, payload
  files or Vitrallis-owned curl process and preserved Debug's data. The UI offered
  Install again. A subsequent retry failed with curl exit 28 while fetching a
  pool-wall PNG; no receipt activated. A bounded direct probe of that exact
  immutable file then passed with its expected size and SHA-256, and another
  full public retry installed 0.11.1 successfully. All 198 payload files match
  its receipt. Menu launch, Home/background, same-process resume and normal menu
  Exit passed. The published baseline recreated settings and lightmap cache in
  its payload; those known test-generated files were removed before testing the
  prepared storage correction, honoring the owner's preference deletion request.
- Download failures previously put the filename before the cause and did not
  retain the full operation error in the private session log. The correction
  puts the cause first, explains curl's transfer timeout and logs the complete
  quoted error. The regression fails against the old ordering, verifies no
  activation after failure/cancellation and verifies a successful retry. On the
  physical corrected Shell, a cancelled Places update visibly reported its
  cause and logged the full failed-file context. The previous 0.11.1 receipt,
  all 198 payload files and a private Unicode-named AppData marker were unchanged.
  This used a backed-up development catalog fixture pinned to reviewed Places
  source `768abe2`; it does not certify a public catalog refresh or an abrupt
  interruption during commit. The complete retry installed 0.11.2: all 201
  device payload files match the new receipt, including binary SHA-256
  `97f6607e95a63bc26aee5a2b3feb122010038d2ddd20893ecff6f776b168866d`.
  The AppData marker is unchanged and there are no unreceipted payload files.
  Updated-app launch and application-created state checks remain pending.
- Firefly Field 0.3.1 installed from the public catalog, launched with canonical
  AppData as its working directory, resumed the same process after Home and
  exited normally through Escape. Its renderer reports hardware GLES2/Mali400,
  480×272 and requested VSync. HUD snapshots showed 4 FPS at startup and 1 FPS
  after resume, with a 43-second process sample at 40.5% average CPU and 45,100 KiB
  RSS. These are observations, not a sustained benchmark; measurement and
  investigation remain pending before performance acceptance. Update and
  uninstall/reinstall coverage also remain pending.
- A controlled physical-device Notepad save on an isolated 512 KiB tmpfs with
  only 16 KiB free failed with ENOSPC. The original 64 KiB document, ownership
  and private mode survived, and the staging file was cleaned. Removing the
  owned filler allowed a successful retry through the same editor. This tests
  real kernel I/O and the native UI, not NAND pressure or NAND durability. The
  generic error dialog lacks save-specific context; its wording remains to be
  improved.
- Debug's actual GPU Pulse produced a rise and return to idle in the private
  Mali utilization trace. This verifies activity reporting; it does not certify
  physical tear-free presentation. Process checks distinguish the original
  PocketHome's outside-service zombies from Vitrallis-owned processes. An initial
  overbroad all-user zombie assertion is retained as a failed harness result.
- The selected Arti ARMv7 normal/build graph contains 428 distinct package
  versions. Fifteen missing inventory entries were reconciled against cached
  originals and immutable upstream source revisions; all selected graph entries
  now reference retained notices. Four new verbatim texts preserve their
  copyright holders, and the MPL component includes its unmodified source URL.
  The existing 1,294,896 notice bytes were preserved. This conservative dependency
  graph does not establish which build-only crates are linked.
- The matching Rust 1.99.0 source/runtime inventory is reconciled for the three
  distributed Linux targets. Shell and PocketCHIP Places retain 37 distinct exact
  texts/annotations/excerpts, including compiler-builtins' complete AND terms,
  the LLVM exception, Unicode data, and nested musl/Sun/BSD/CORE-MATH notices.
  Existing collected bytes were preserved. The conservative source inventory is
  documented in [Rust runtime licenses](rust-runtime-licenses.md); it does not
  claim every build dependency or math routine is linked, or certify an OS image.

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
| Every catalog app lifecycle | Debug public lifecycle and offline uninstall/reinstall passed; Places and Firefly public install/launch/resume/exit passed; prepared Places update cancellation preserves installed bytes/data; complete updates and remaining app runs pending |
| Real data persistence | Host lifecycle passed; physical update/reboot/reinstall sequence pending |
| Python runtime | Host tests passed; current device dependency/lifecycle checks pending |
| Process lifecycle stress | Debug exits left no app processes or Vitrallis-service zombies; stock PocketHome zombies distinguished; sustained repetition pending |
| Repeated startup | Delayed original-desktop focus grab repaired; two reboots passed sustained idle focus; exact final build and cold-power checks pending |
| Hardware features | Display/GPU backend observed; radio/audio/backlight/battery/power acceptance incomplete |
| Every setting persistence | Clock format and timeout survived development reboot; remaining matrix pending |
| Offline/network failures | Physical Wi-Fi off/on, cached App Center, offline uninstall and refresh recovery passed; remaining fault matrix pending |
| Low NAND / ENOSPC | Controlled physical Notepad ENOSPC/recovery passed on isolated 512 KiB tmpfs; actual NAND pressure remains pending |
| Shell update/rollback | Host recovery tests pass; actual candidate device update/rollback pending |
| Security/trust boundaries | Concrete path guards repaired; complete audit/fault matrix pending |
| Failure UX | Safe uninstall/trust confirmations, corrected offline refresh/Details and cause-first cancelled update inspected; full operation error retained in private log; other failures pending |
| Files/Terminal/Notepad | Real note save/read, direct editor and footer wraparound passed on device; remaining manual utility cases pending |
| Performance | Development measurements recorded; Firefly HUD reports low FPS across startup/resume and needs sustained measurement/investigation; exact-candidate measurements pending |
| Extended soak | 30-minute development session completed with intentional restart; exact-candidate soak pending |
| Logs | Initial renderer/startup logs inspected; final audit pending |
| Code/documentation hygiene | Storage/provenance docs updated; final sweep pending |
| Public owner documentation | Exact fully clean Beta2 entry FAILED; corrected public candidate entry must be retested |
| License/repository consistency | Artwork, selected Arti graph and matching Rust runtime notices reconciled; final published artifact verification pending |
| Canonical release builds | Shell full host gate passes; ARM development bundle builds/packages; remaining exact-state checks pending |
| Exact clean candidate | Final revision not established; reviewed focus/native state has matching CI and physical evidence; no final tag/release |
| Final physical smoke | Not run |

## Validation recorded so far

Shell `sh scripts/validate.sh` passes on macOS ARM64 and native Linux ARM64 with
the approved 1.0.0 version and reviewed references: formatting, locked
all-target/all-feature check, strict Clippy, workspace tests, Python tests,
release binaries, SDL/native smokes and doc links. The latest runs include the
editor/importer, private-root-umask, real-Lua focus and offline diagnostic
regressions: 364 Rust tests on macOS, 365 on Linux, and 173 Python tests
(nine existing skips on macOS, eight on Linux), plus the native
renderer suite (two tests, one existing skip).
The ARMv7 release build and complete five-executable packaging/version checks
pass against glibc 2.36/SDL2 2.26.5. Bitcoin 1.3.1 has 71 passing tests; Carousel
0.4.3 has 271 passing tests and one existing skip. The isolated Places package's
latest full suite has 843 passing tests and three existing ignored tests; its
settings/ENOSPC regression and cache round trip pass. Its final strict Clippy
includes all, pedantic, nursery and cargo groups, with documented exceptions for
two unavoidable upstream duplicate-crate pairs. Formatting and ARMv7 rebuild
pass. Its 212-file staged package passes the Apps manifest validator, and its
notices record all 39 locked ARMv7 dependencies and the matched Rust runtime.
The development-fixture 0.11.2 update verified all 201 installed device payload
files and retained the private marker; application-created state checks remain
pending.

The startup hook's two Lua regression scenarios also passed inside the physical
Awesome Lua 5.3 runtime, in an isolated mock environment. The device Python session
suite passed 12 tests with two standalone-Lua skips; those two scenarios were
therefore exercised separately inside Awesome rather than silently omitted.
An Arti QEMU probe initially crashed when the harness mixed the cross-toolchain
loader with the multiarch runtime libraries. With the matching system loader/libc
prefix, all five ARMv7 version probes and exact `c17280d`, `a4a3e8f` and
`fbf2ec7` and `9f6f0d0` packaging passed. The
same Arti bytes also passed their native device version probe. The failed mixed
sysroot attempts remain in the evidence.

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
