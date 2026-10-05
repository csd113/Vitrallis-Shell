# Public-release certification — 2026-10-02

**RELEASE READY. Exact-public 1.0.3 automated/device checks and final owner
physical cold-power/input/display/audible-audio acceptance pass.**
The [14-item readiness report](release-readiness-2026-10-02.md) is the current
handoff. Earlier records below retain their original revision-qualified failures,
repairs and qualifications; their pending/unauthorized statements are historical.

The goal baseline is `08a204be025d4be7cad07f6fed443a8603441ac3` on
`upgrade/rust-1.99.0`. The final reviewed/tagged source is
`ed10bf0d827da1b200528fab8e07905979eb7de6` (Shell 1.0.3), merged through
[PR #9](https://github.com/csd113/Vitrallis-Shell/pull/9) with an identical tree.
The owner authorized this release and subsequent prerelease version changes until
the actual full release. No additional version authorization is outstanding.
Raw logs, receipts, checksums and screenshots remain under
`target/release-certification/2026-10-02/`; private raw evidence is not a public
release asset. Earlier published release binaries remain unchanged.

## Final public candidate

[The exact tagged workflow](https://github.com/csd113/Vitrallis-Shell/actions/runs/37250311088)
and all four exact-source Rust 1.99.0/stable review checks pass. All 19 assets,
eight sidecars, six tagged helpers and three legal files verify. Both targets
contain five hash-verified ELF members; publication leaves all asset IDs, sizes
and digests unchanged. The certification prerelease is published at
2026-10-05T01:24:30Z. ARMv7 bundle SHA-256:
`41daa4cdc9aafe0e7ffebafd58c79ebd08970fd347604ded28358329a000cc2f`.

Normal uninstall and exact owned root-integration cleanup restore both original
board trees and Awesome configuration. Stock boot
`c101061b-01ad-45f7-b855-ec0c161dc4ea` reaches PocketHome in 177.709 seconds with
no Vitrallis core, process or live GPU OPP. The anonymously fetched public README
command is unchanged (SHA-256 `cd8e4d9be5f8ad70c7aee90ac056cc76d1569f379e957f01d4a78659f11e48ee`).
Its completed first and repeat runs select public 1.0.3 and pass all five native
versions/hashes/modes, five installed helpers, private runtime/receipt/lock modes,
exact startup replacement and saved configuration. No previous pointer is
invented by repetition. Retained App Center/Tor state is restored without
replacing any public artifact; all 93 saved entries retain exact bytes/owners/modes.

Public installed boot `227f90e4-23b1-4a22-ac93-f5b506ec8653` reaches Ready in
186.194 seconds including OS/USB/SSH startup. The same fullscreen 480×272 Shell
owns actual X focus at 0, 15 and 30 seconds, with no PocketHome or owned zombie.
Mali400 graphics self-test and three utility hardware smokes pass with VSync and
no fallback. Terminal PWD/normal keyboard exit, a private 26-byte Notepad save,
Files folder/parent/keyboard Close, and Settings/Wi-Fi/storage/time/About pass.
Calculator's Cancel-default uninstall declines safely; confirmed removal,
public 0.1.1 reinstall, focused 480×272 launch, Home and normal WM exit pass.
The normal background exit restores deliberately cleared actual X focus while
Settings remains selected; keyboard Enter opens Display & Sound afterward.
The PID-specific child-reaped event records normal status zero. All 93 baseline
entries and three private test notes remain unchanged. All 15 Tor cases pass
against the exact public Arti 2.7.0 SHA `a8553d5afa3387a729def3c46773e25e79a8621d48f01936f30fa87712c43dc6`,
including real bootstrap and clean stop (77.960 seconds, no exclusions).

The intervening stock session resets brightness again. Restoring 100% through
actual Settings returns all eight original settings to baseline. The final
Vitrallis-only cold-power review below verifies live/saved brightness and stored
settings, while qualifying the live volume after manual audio use. The stable
release endpoint reports no stable release while all releases are certification
prereleases; this clear response is expected until the first full release is published. Older
prereleases expecting Arti 2.6.0 require the README setup route for the current
2.7.0 bundle, and obsolete generations are not supported rollback targets.

Final pre-owner log review has no unexpected panic/error/warning, fallback or
owned zombie. The missing optional surf warning and the deliberately requested
no-stable-release check are recorded separately. Before owner acceptance, battery
readback is Good/Charging, 98%, 4.196 V; USB is online at 4.812 V, Wi-Fi connected,
and free space is 1,161,629,696 bytes. UI automation is paused for the final owner
physical test. This pre-owner snapshot is retained separately from the final
acceptance below.

The first private draft-asset observer uses the public tag endpoint and receives
404; authenticated draft-ID lookup resolves it without changing artifacts. Old
launcher/modal header guards stop on current selection/context, and corrected
current screenshots are inspected. Native utilities share renderer log events;
anchoring to the current supervised-session marker retains the complete session
and Ready checks. The first app-exit observer assumes PID/plain-status fields
on app_exited; actual PID-specific child_reaped and the source logger's debug
status establish normal exit. Original failed observers are retained. Repository
assertions and visual goldens remain unchanged; no public recovery latency is
claimed from the failed timing observer.

### Final owner cold-power acceptance

The owner reports: “all testing looks good, audio is working, touch and keyboard”.
This answers the final physical cold-power/input/display test request. Boot ID
changes from `227f90e4-23b1-4a22-ac93-f5b506ec8653` to
`3f18fc84-ab4b-4880-b1ac-0a05892bcae6`. Independent post-test inspection verifies
the exact public 1.0.3 generation and five native versions/hashes/modes, five
helpers, private configuration and startup replacement. Native PID 883 owns
actual keyboard focus in the fullscreen 480×272 Shell; PocketHome and owned
zombies are absent. Current-session logs contain no unexpected error, panic,
warning or graphics fallback. Wi-Fi is connected, timezone is Vancouver, battery
is Good at 99%, USB is online, and free space is 1,160,278,016 bytes.

The first strict settings snapshot observes an idle DPMS-off screen with live
brightness zero. A harmless Shift wake restores 10/10 without setting brightness;
the OS-saved brightness is also 10/10. Private preferences, timeout, timezone and
saved ALSA volume (57/63) match their original values. The live mixer after manual
audio testing reads zero; the owner confirms “Yes, I changed system volume”.
That chosen value is preserved. The owner confirms audible audio. Current mixer state and user edits
are retained. Earlier controlled reboot/update/Restore volume evidence remains
revision-qualified.

An additional controlled software reboot of the same public artifacts verifies
all eight original settings, including live volume 57/63. Boot is
`c365234c-a1c2-46a3-a582-a4d8f509428e`, native PID 1072; SSH/native readiness
takes 186.899 seconds, and actual fullscreen focus remains correct at 0, 15 and
30 seconds.
The OS-saved ALSA file retains identical bytes. The pre-test live mixer value
zero is restored afterward, and all 93 validated post-owner entries—including
the two app setting edits—remain unchanged. No source or release version changes
are needed. This independently verifies persistence while preserving the
owner-confirmed volume change.

The strict unchanged-byte observer also stops on two settings files following
manual app use. Review of the original archived JSON and current hashes shows
only Music volume/selected track and Sketch color changed; both remain valid
private settings. All 93 tracked paths retain their kinds, owners and modes;
91 retain identical contents, and all three saved Notepad test documents remain
unchanged and private. These app edits are preserved. Original failed observers
are retained; no repository assertion or visual baseline is weakened.

The candidate is ready for the owner's full release decision with the known
qualifications in the readiness report. The published 1.0.3 assets remain the
same certification prerelease; this documentation does not promote it to stable.

Evidence: `v103-candidate-identity.json`, `v103-artifacts-manifest.json`,
`v103-publication-proof.json`, `v103-stock-reboot.json`,
`v103-public-readme-proof.json`, `v103-public-first-verification.json`,
`v103-public-repeat-verification.json`, `v103-public-installed-reboot.json`,
`v103-public-real-arti-device-proof.json`, `v103-calculator-exit-focus-proof.json`,
`v103-final-health-before-owner.json`, `settings-persistence-v103-before-owner.json`
and `retained-state-v103-before-owner.json`. Final evidence:
`v103-owner-physical-acceptance.json`, `v103-final-health-after-owner.json`,
`v103-public-after-owner-verification.json`,
`settings-persistence-v103-after-owner-awake-review.json`,
`retained-state-v103-after-owner-review.json`,
`v103-after-owner-audio-state.json`, `v103-volume-controlled-reboot.json`,
`v103-volume-controlled-persistence-proof.json`,
`settings-persistence-v103-volume-controlled-boot.json`,
`v103-final-health-after-controlled.json` and
`retained-state-v103-after-controlled.json` and
`v103-owner-volume-clarification.json`.

## Authorized 1.0.3 and dependency refresh

The owner approved Shell 1.0.3 and gave standing authorization for prerelease
version changes until the actual full release. This supersedes the named-version
approval constraint for the ongoing certification work. The owner also requested
Rust 1.99 and the newest dependencies. The workspace now declares Rust 1.99 as
its minimum; host and release pins remain exact 1.99.0, and the official Debian
12 Rust 1.99.0 container is available. CI validates 1.99.0 and latest stable.

The live crates.io review checks all ten direct dependencies and 51 locked
registry entries. libc updates to 0.2.190 and lazy_static to 1.5.1; remaining
direct crates are current. Three older transitive versions remain constrained
by upstream SDL2/PNG requirements, as documented in the dependency review.
Arti updates to latest stable 2.7.0 using its publisher's locked graph and the
existing feature set. Every current version consumer and fixture is updated.
The selected ARMv7 normal/build graph contains 426 packages; 106 new or updated
license rows are reconciled against cached source texts and exact registry VCS
revisions. Workspace RustSec audit reports no vulnerabilities or warnings.

Private evidence is recorded in `v103-authorization.json`,
`v103-registry-index-audit.json`, `v103-dependency-upgrade-final.log`,
`v103-cargo-update.log`, `v103-cargo-audit.json`,
`v103-dependency-license-proof.json` and
`v103-arti-license-reconciliation.json`. Full `sh scripts/validate.sh` passes
on macOS and Linux: formatting, locked all-target/all-feature checks, unchanged
strict Clippy policy, 380/382 Rust tests, 181 Python cases with nine/eight platform
exclusions, release builds, renderer goldens, SDL smokes and documentation links.
The real Awesome/X11 suite also passes on the new build, including actual-focus
recovery after a background exit and preservation of another app's focus. ARMv7
build and five-executable packaging pass with Arti 2.7.0.

The first Linux Python pass failed because Docker's 95 GiB filesystem was full;
free-space preflights stopped seven tests before their intended paths. The failed
log is retained. Clearing only Vitrallis compiler incremental caches restored
3.9 GiB free; the complete unchanged canonical sequence and X11 suite then pass.
No unrelated volume, image or test evidence was removed. Both Arti target graphs
have complete retained notices: 426 ARMv7 and 427 x86-64 packages, zero unresolved
selected rows. Exact tag assets, the renewed clean public installer and final
physical acceptance remain open.

The first exact-source push check failed the existing live process-identity test
with Linux ESRCH (`No such process`) while scanning procfs. The two PR jobs pass
on the same source; this is a process-exit race, not grounds to discard the
failed check. The scanner now ignores only ENOENT/ESRCH for disappeared entries
and preserves permission and other I/O errors. A targeted regression checks
both outcomes; complete renewed Mac/Linux canonical validation passes with
380/383 Rust tests. The Shell reuses the existing workspace libc crate for the
errno constant; no registry crate is added.

Prepared 1.0.3 installation and repeat installation pass on PocketCHIP with the
new dependencies, retaining all 93 tracked entries and all eight settings. The
480×272 launcher owns actual X focus, with no PocketHome or owned zombie. All 15
Tor cases pass on the device, including real Arti 2.7.0 bootstrap and clean stop.
An initial test packet omitted sandbox.py; that fixture error and its corrected
complete rerun are retained. These prepared checks do not replace the exact
tagged/public candidate checks.

## Authorized 1.0.1 recovery correction

The owner approved Shell 1.0.1 after the published 1.0.0 reinstall recovery defect
was reproduced. Reinstalling an active bundle now retains the distinct previous
generation. The regression test fails against the published installer and passes
with the correction; all 42 installer tests pass. No Rust production behavior,
package format or dependency changes accompany this correction.
[PR #6](https://github.com/csd113/Vitrallis-Shell/pull/6) is merged at
`24a646f2461780b2e9ad45b3797973e48afb46fc`. The annotated `v1.0.1` tag identifies
reviewed source `b2ff6d48be33b0a70c373d56d1a8c2f354501db2`; its tree equals the
merge tree. All six Rust 1.91.0/1.99.0/stable review checks pass.
[The exact tagged workflow](https://github.com/csd113/Vitrallis-Shell/actions/runs/37184116931)
passes canonical validation, ARMv7 and x86-64 builds, utility probes, packaging
and legal notice checks. The exact tagged device checks pass; 1.0.1 is published
as a certification prerelease at 2026-10-04T07:19:36Z. All 19 asset IDs, sizes and digests are
unchanged by publication. Stable readiness is not declared.

All 19 draft assets match GitHub sizes/digests; eight checksum sidecars and all
six helper/three legal files verify against the tagged source. Both bundles have
five verified ELF members and no trailing data. ARMv7 bundle SHA-256 is
`1ee6c8e5e4cd7184a27ce8668f252ab40ac8a339f30ba9e96e55f746910dc0e6`;
x86-64 bundle SHA-256 is
`3f4a8c74c40e61d998cf832ead89a3c2794f53d96e1d0f32d45aa4221134a804`.
Prepared installation of the exact ARMv7 bytes and all matching helpers on the
physical device passes. Repeating the installation retains published 1.0.0
bundle `5ad2944b…` as the distinct previous generation. All five executables
have the expected version, hashes and `chip:chip` ownership with mode 0755;
installed helpers are mode 0644. All 93 tracked saved entries and all eight
original settings remain unchanged. These prepared checks do not substitute
for the public README installation route after publication.

The exact tagged Shell graphics self-test and three utility hardware smokes
pass with Mali400, VSync and no fallback. At 480×272, strict software/hardware
pixel equality is **FAIL** on 73 Terminal, 41 Notepad and 90 Files pixels, each
by at most one colour level. Hardware and automatic readbacks match exactly;
no assertion or visual baseline is weakened. A software reboot reaches
current-session Ready in 187.112 seconds, boot
`8d15742a-e23a-4530-aaf2-55600aa096c0`, native PID 1114. At 0, 15 and 30 seconds,
actual X11 focus agrees with the same fullscreen 480×272 Shell window, with no
PocketHome process/window or owned zombie. All 93 retained entries and original
eight settings match afterward. This is software reboot evidence; physical cold
acceptance of these bytes is still required.

Full canonical validation of version 1.0.1 passes on macOS and native Linux:
formatting, strict Clippy, workspace check/tests, release builds and software
SDL smokes. macOS runs 378 Rust tests; native Linux runs 380. Python discovery
runs 178 cases, with nine macOS and eight Linux explicit environment exclusions.
The two actual Awesome/X11 fixtures also pass in a separate session-bus run.
Initial disposable Linux harness failures (hidden Cargo binary, then missing Git
metadata) and the fixture run without its required session bus are retained;
corrected harness runs pass without weakening assertions.

Before installing 1.0.1, four real-device configuration fault cases passed on
the prior 1.0.0 native bundle: missing, malformed, partial and mode-000 files for
preferences, screen timeout and Tor policy. Each reaches the focused 480×272
launcher with no PocketHome process/window or owned zombie. Nonmissing invalid
bytes/modes remain unchanged; missing Tor policy creates the documented private
default. Each case restores the original files and relaunches successfully.
All 93 tracked saved entries and eight original settings match afterward.
This is prior-native-build fault evidence, not a claim that the 1.0.1 executable
was used for these cases.

Evidence: `recovery-v1.0.1-artifacts-manifest.json`,
`recovery-v1.0.1-device-install-proof.json`,
`settings-persistence-after-v101-install.json`,
`recovery-v1.0.1-canonical-summary.json`,
`recovery-v1.0.1-tag-workflow-final.json`,
`recovery-v1.0.1-pr-merged.json`, `configuration-faults-device-proof.json`,
`retained-state-after-configuration-faults.json` and
`settings-persistence-after-configuration-faults.json`.

## Fully clean public 1.0.1 installation and cancellation UX

The normal 1.0.1 uninstaller removes all three installed generations and restores
the exact original Awesome configuration (`507e5205…`). All 93 tracked saved
entries remain unchanged. A certification-only guarded cleanup then archives
and removes the exact Vitrallis-owned root GPU/media integration, restores both
original board trees (`0132f7fa…`) and retains saved App Center/Tor state outside
the absent core root. Reboot `fdf7515b-865e-4c56-bfd4-4e434a7722a4` reaches the
stock PocketHome desktop with no Vitrallis-owned process, no installed core root,
matching actual X11 focus and no live Vitrallis GPU OPP. Stock readiness takes
176.711 seconds. The first cleanup observer incorrectly required a rotated log
that was absent; it stopped after a successful normal uninstall. The corrected
continuation verifies the exact four retained children before further mutation.
The original failed observer and all cleanup/backup logs remain retained.

The exact public README command is fetched anonymously on this clean device,
SHA-256 `cd8e4d9be5f8ad70c7aee90ac056cc76d1569f379e957f01d4a78659f11e48ee`.
Anonymous 1.0.1 release metadata verifies all 19 asset IDs/sizes/digests. The
unmodified command selects published 1.0.1 and succeeds, including fresh root
GPU/media provisioning and the expected reboot request while PocketHome is
active. All five native versions/hashes/0755 modes and five installed helpers,
0700 core/generation directories, 0600 lock/receipt, one exact startup replacement
and retained configuration verify. Saved App Center/Tor state is restored without
replacing any public core artifact. All 93 saved entries match again.
A completed repeat of the same public command also succeeds and preserves an
absent previous pointer, all public core checks and all 93 saved entries.
Public-install reboot `60a5ecb5-1a66-478e-b2df-72ffc0762d38` reaches Ready in
186.179 seconds with native PID 1071. At 0, 15 and 30 seconds, the same fullscreen
480×272 window owns actual X11 focus, with no PocketHome process/window or owned
zombie. All 93 tracked retained entries still match. Final critical-function
smoke is continuing.

The original-eight-settings verifier fails afterward on brightness only: live
brightness is 1/10 instead of 10/10. The OS backlight service reports loading a
saved zero and clamping it to one. This clean sequence includes an intervening
stock PocketHome session; the reset is not yet attributed to Shell. The failed
capture is retained. Restoring 100% through the actual Settings slider returns
all eight original settings to their exact baseline. A controlled Shell-only
reboot passes without that stock interval: boot
`2fa3e6d0-30d2-46b1-885e-e817413e034f`, Ready in 187.458 seconds, PID 1086,
the same actual X11 focus at three samples, no PocketHome or owned zombie,
brightness and OS saved brightness both 10/10, all eight original settings and
all 93 tracked saved entries unchanged. The failed clean-sequence observation
remains qualified; this controlled result does not prove what wrote its zero.
The launcher has all
17 expected entries across three pages; the initially suspected missing entries
are visible on the middle page, including the working System Settings tile.

Public 1.0.1 native utility smoke also passes: Terminal accepts keyboard input,
prints `/home/chip` and reaps its interactive shell; its visible Close control
then exits the window. Notepad saves `public 1.0.1 smoke.txt` with the expected
26 bytes, mode 0600 and `chip:chip` ownership. Files enters Documents with arrow
keys/Enter and returns to the home directory with Escape. Both Close controls
return to the original focused Shell; all four identified utility/PTY PIDs are
gone, with no owned zombie. All eight original settings and all 93 original
retained entries still match. The new note is additional real update-test data.
The genuine Settings update check reports that no stable release is published
yet, matching the existing stable-only policy of native version 1.0.1. Official
OTA failure tests will use a genuine published beta native baseline, which
accepts the published certification prerelease, with current integration helpers.

The first repeat was started before the verification/retained-state restoration
observer finished and was cancelled during download to end overlapping work.
No installed pointer, receipt or saved entry changes, and the subsequent retry
passes. This exposes a real failure-UX defect: published bootstrap cancellation
prints a Python `KeyboardInterrupt` traceback. A focused prepared correction
catches that interruption, prints a short instruction to rerun setup and exits
130. Its CLI regression fails against published code and passes with the fix;
all 15 bootstrap cases and full Mac/Linux canonical validation pass (179 Python
cases, plus unchanged Rust/build gates). An actual prepared-helper public bundle
download is interrupted at 110,592 bytes on the PocketCHIP: exit 130, short
message, no traceback, no curl process or temporary payload, unchanged installed
pointers/receipt and all 93 saved entries. The correction is not in published
1.0.1. Separate authorization for a fresh version is pending; published assets
remain immutable.

Evidence: `recovery-v1.0.1-public-clean-v2.log`,
`recovery-v1.0.1-stock-reboot.json`, `recovery-v1.0.1-public-readme-proof.json`,
`recovery-v1.0.1-public-first-verification.json`,
`recovery-v1.0.1-public-repeat-verification.json`,
`recovery-v1.0.1-public-repeat-interrupted-download.log`,
`recovery-v1.0.1-public-repeat-retry-install.log`,
`retained-state-after-v101-public-repeat-retry.json`,
`recovery-v1.0.1-public-installed-reboot.json`,
`retained-state-after-v101-public-reboot.json`,
`settings-persistence-after-v101-public-reboot.json`,
`settings-persistence-after-v101-public-brightness-restore.json`,
`brightness-controlled-v101-reboot.json`, `brightness-controlled-v101-os-state.json`,
`settings-persistence-after-v101-controlled-reboot.json`,
`v101-public-utility-smoke-state.json`,
`retained-state-after-v101-public-utility-smoke.json`,
`settings-persistence-after-v101-public-utility-smoke.json`,
`bootstrap-cancellation-regression-red.log`,
`bootstrap-cancellation-device-proof.json` and
`bootstrap-cancellation-canonical-summary.json`.

## Official update and interruption evidence

Published native version 1.0.1 retains its stable-only release policy: its real
Settings check reports that no stable release is published yet. To exercise
official prerelease downloads without changing metadata or executable versions,
the device uses the genuine published five-member `v1.0.0-beta-2` ARM bundle
`0f3c5ab0…` with current 1.0.1 integration helpers. All five baseline member
hashes and version probes match their public assets. This is a prepared update
baseline, not an installation through the obsolete beta installer or an app
lifecycle test of that older native build.

The actual Settings Check action offers public 1.0.1, and its Install
confirmation visibly defaults to Cancel. Three targeted failures pass:

- Interrupt the verified owned curl at 77,824 downloaded bytes: readable
  interruption error, no activation, complete staging cleanup and curl exit.
- Pause that verified curl, corrupt byte zero of its private staging payload
  and resume: actual SHA-256 rejection before activation, complete cleanup.
- Kill only the verified owned Shell while Arti extraction is partial at
  147,520 bytes: current and previous remain unchanged, and actual session
  relaunch reaches the healthy beta with sustained real X11 focus.

After each failure, the installed receipt, new public-build note and all 93
original tracked saved entries remain unchanged. The extraction interruption
leaves staging as expected after process death. The beta's subsequent Check
cleans it through that release's older completion probe; current 1.0.1 instead
opens and cleans staging when installation begins. This source difference is
explicitly qualified. No completion bridge is added to current code.

The ensuing genuine public retry completes in 112.062 seconds, verifies all five
1.0.1 member hashes/versions/0755 modes and unchanged matching helpers, activates
`1ee6c8e…`, retains beta `0f3c5ab0…` as previous and removes staging. The already
present verified target generation is reused after comparing all staged bytes.
Actual Relaunch executes public 1.0.1 in PID 9510 and reaches the focused 480×272
launcher with no PocketHome or owned zombie. Saved data remains unchanged.
Settings Restore then activates the beta, retains 1.0.1 as previous and relaunches
successfully with the same data/focus checks. The reverse Restore/relaunch also
passes: public 1.0.1 is active again, with beta retained as previous. Both
confirmations visibly default to Cancel. All 93 original saved entries, the new
note and all eight original settings match after this round trip. The device is
left at the focused public 1.0.1 launcher in PID 9510, with no PocketHome or owned
zombie.
These observations precede the additional relaunch failure below.
Activation-window interruption and final candidate hardware acceptance remain
separate gates.

Two observer failures are retained: a placeholder substitution initially corrupts
a Python identifier and stops before device execution; the corrected observer
compiles locally before dispatch. A later observer incorrectly expects staging
still to exist after the beta Check; its failure is retained, the source explains
the earlier cleanup, and the retry observer instead requires a clean initial
stage, observes a new private download and verifies the completed public result.

Evidence: `official-update-beta-2-baseline-proof.json`,
`official-update-interrupt-download-proof.json`,
`official-update-corrupt-download-proof.json`,
`official-update-interrupt-extraction-proof.json`,
`official-update-after-extraction-recovery-state.json`,
`official-update-success-proof.json`,
`official-update-relaunched-v101-state.json`,
`official-update-restore-to-beta2-pointers.json`,
`official-update-relaunched-beta2-state.json`,
`official-update-restore-to-v101-pointers.json`,
`official-update-final-v101-state.json`,
`retained-state-after-official-public-update.json`,
`retained-state-after-official-rollback-to-beta2.json`,
`retained-state-after-official-rollback-round-trip.json` and
`settings-persistence-after-official-rollback-round-trip.json`.

### Subsequent relaunch failure and pressure-fixture cleanup

A later actual Restore/relaunch from public 1.0.1 to the genuine beta baseline,
while the owned NAND pressure fixture is being prepared, leaves `ip` PID 18270
as a persistent zombie under Shell PID 9510. Direct PID inspection confirms
state `Z`, parent 9510 and process start ticks 306165, rather than relying on a
filtered process-name listing. The failure remains present more than eleven
minutes later. Earlier passing samples remain valid observations, but do not
clear this later process-lifecycle failure.

Published 1.0.1 replaces the multithreaded Shell through `Command::exec` without
first joining its background system workers. `Worker.pending` covers control
requests only; the separate periodic status worker can still own an `ip` child.
This supplies the cause corrected in the prepared source below. The OTA
activation observer has not been started; the subsequent installer cancellation
test does not substitute for that separate update path.

The live UBIFS fixture is stopped through its owned stop marker, and its guarded
cleanup removes the filler, readiness file and stop marker. It reports
1,232,613,376 free bytes afterward, versus 1,244,553,216 before preparation.
The interrupted preparation had not reached its 112 MiB target and is not a
low-space Shell-update pass. Its intentional `InterruptedError` is in the
certification harness log, not the product. At that cleanup, the native baseline is
the beta, with public 1.0.1 retained as previous and current integration helpers.

Evidence: `activation-relaunched-beta2-state.json`,
`relaunch-persistent-zombie-before-pressure-cleanup.json` and
`shell-update-pressure-fixture.log`.

### Prepared relaunch and cancellation corrections

The worker-lifetime defect is corrected in `f20cc01`: before replacing Shell,
a dedicated cleanup thread disconnects and joins the system workers and stops
and joins Tor supervision. The launcher keeps presenting frames while cleanup
runs. Cleanup or exec failure refuses replacement and restores services for an
explicit retry. The transient Updates view displays the operation without
inactive navigation/action buttons or key hints. Existing app/control blocking
and safe Install/Restore confirmation defaults remain in effect.

The regression holds a real status subprocess while no control is pending,
checks that preparation returns promptly without exec, and releases and reaps
the child before allowing replacement. Updater coverage checks pending cleanup,
refusal on failure and explicit retry. The waiting view is inspected at 480×272;
automated rendering verifies that the action/footer area is empty at all four
supported fixture sizes. Existing visual references are unchanged.

A prepared device trial holds the exact owned `nmcli radio wifi` process through
a verified pidfd. The cleanup thread appears while the child is stopped and
Shell still executes the prior generation. Exec occurs only after the child's
PID is gone, preserving Shell PID 8905 and start ticks 680053. This first trial
starts from a private diagnostic build differing only in Restore-availability
logging, and executes the uninstrumented prepared bundle `da11916e…`; it is not
an exact public-candidate pass. Subsequent inspection confirms the focused
480×272 launcher, no PocketHome, no owned zombie, all 93 original saved entries
and all eight original settings unchanged. The first readiness observer lost
Ready from its 4 KiB log tail after later query messages grew; the corrected
observer uses the complete latest renderer-initialization segment and passes.

An initial visual interpretation incorrectly reports a missing Restore footer.
Enlarging the retained 480×272 capture confirms the Restore label was present.
The exact 120×22 footer crop from the normal prepared and diagnostic captures
has identical RGB SHA-256 `7cf722a5302da96378dfa16d8303844f197fc040aa0f38f08c4b97974abe4abd`.
The read-only ARM structural probe and private runtime logging also validate the
retained beta and the visible Restore control. This is a corrected observer
mistake, not a product defect or a new compatibility requirement. Original
captures and diagnostic evidence remain retained. A clean uninstrumented source
archive rebuild produces exactly the same bundle `656c0850…`. Diagnostic logging
exists only in a private fixture. The following relaunch starts from those normal
prepared bytes, with no diagnostic logging.

The production-source trial starts with bundle `656c0850…` and Shell PID 31813,
start ticks 1133893, then uses the actual Settings Restore confirmation and
Relaunch action to select the genuine published beta. The verified owned
`nmcli radio wifi` child, PID 903/start ticks 1159943, is held through its pidfd.
The cleanup thread appears and exec remains deferred while that child is stopped;
its PID is gone before exec completes. Shell retains its PID/start ticks across
replacement, and the receipt and new note remain unchanged. The observed trial
takes 14.519 seconds including the held query. The focused 480×272 beta launcher
then has no PocketHome or owned zombie; all 93 original saved entries and eight
original settings still match. This verifies production-source cleanup, with the
genuine beta serving only as a replacement baseline, not a newly certified app
runtime or an exact public-candidate pass.

A normal session stop and unmodified prepared installation return the device to
`656c0850…`, with the genuine beta retained as previous. All five native bytes
and 0755 modes verify, the new note is unchanged and no repair marker remains.
Native PID 2894 reaches the focused 480×272 launcher on the same boot, with no
PocketHome or owned zombie. All 93 original saved entries and eight original
settings match again. The old beta's unfixed in-place relaunch is not used for
this return.

`e905401` catches setup cancellation in both public bootstrap and local installer
CLIs, exits 130 and gives a short rerun instruction. All 44 installer regressions
pass, including rollback of both pointers and the original receipt after an
interruption following current publication, then successful repair/retry with
private user data unchanged. Published 1.0.1 remains immutable and still contains
the earlier cancellation UX and relaunch defect.

On the real PocketCHIP's UBIFS filesystem, the unmodified prepared installer is
interrupted by a real SIGINT immediately after publishing current and before
removing its repair marker. A deterministic trace checkpoint selects that narrow
window. The genuine public beta is used as the replacement payload. Exit 130 and
the short cancellation instruction pass with no traceback; both original
pointers, original receipt and helper bytes/owners/modes are restored, the repair
marker remains, and the public-build note is unchanged. An ordinary retry
successfully activates the beta and removes the repair
marker; normal prepared installation then restores `656c0850…` with the genuine
beta retained as previous. All five native hashes/versions/0755 modes, all five
helper hashes/0644 modes and the retained beta inventory pass. Native PID 22453
reaches the focused 480×272 launcher with no PocketHome or owned zombie. All 93
original saved entries and eight original settings still match.

The first trace harness omitted its local callback return, missed the checkpoint
and completed an ordinary installation; it is not counted as interruption
coverage. A subsequent SSH harness inherited a consumed heredoc as stdin and
could not deliver its sudo password. It is cancelled before installer execution
and replaced with a file-backed SSH driver. Both harness failures are retained;
neither is hidden as a product pass.

The complete final prepared-source `sh scripts/validate.sh` passes on macOS and
native Linux: formatting, workspace check, strict Clippy, 380/382 passing Rust
tests with 9/12 explicit opt-ins/platform exclusions, 181 Python cases with 9/8
exclusions, release builds, renderer fixtures, software smokes, script checks and
59 Markdown files. ARMv7 release packaging also passes. The current prepared
bundle including the waiting view is `656c0850…`, still internally version 1.0.1;
it is an unpublished fixture. The reviewed changes are in
[draft PR #8](https://github.com/csd113/Vitrallis-Shell/pull/8). All six push/review
Rust 1.91.0, 1.99.0 and stable jobs pass for exact source
`e90540123b37c9ed4376576121ae7b8fe4755004`. Separate release version authorization,
exact tagged candidate/public-route checks and final physical acceptance remain
required.

Evidence: `relaunch-held-probe-to-prepared-proof.json`,
`relaunch-held-returned-prepared-v2-state.json`,
`retained-state-after-held-relaunch-prepared.json`,
`settings-persistence-after-held-relaunch-prepared.json`,
`relaunch-held-production-to-beta-proof.json`,
`relaunch-production-retest-beta-ready-state.json`,
`retained-state-after-production-held-relaunch.json`,
`settings-persistence-after-production-held-relaunch.json`,
`relaunch-production-held-return-install-proof.json`,
`relaunch-production-held-returned-prepared-state.json`,
`retained-state-after-production-held-return.json`,
`settings-persistence-after-production-held-return.json`,
`relaunch-final-canonical-summary.json`, `relaunch-final-artifacts-proof.json`,
`installer-cancellation-after-activation-device-v2-proof.json`,
`installer-cancellation-after-activation-device-v2-retry-proof.json`,
`relaunch-final-install-v2-proof.json`, `relaunch-final-prepared-installed-state.json`,
`retained-state-after-final-prepared-cancellation-retry.json`,
`settings-persistence-after-final-prepared-cancellation-retry.json`,
`relaunch-review-ci-e905401.json`,
`installer-cancellation-after-activation-device-original-hook-missed.log`,
`relaunch-final-canonical-macos-v4.log`, `relaunch-final-canonical-linux-v4.log`,
`relaunch-final-prepared-arm-build-v3.log`,
`relaunch-final-ui-qa/relaunch-wait/updates-480x272.png`,
`relaunch-final-updates-beta-previous.png`,
`relaunch-final-updates-beta-previous-restore-footer-zoom.png`,
`restore-footer-visibility-correction-proof.json`,
`restore-availability-probe.log` and `restore-beta-runtime-ui-renderer.log`.

## OTA activation interruption and normal public retry

The actual beta Settings updater downloads genuine public 1.0.1. An inotify
observer catches publication of `previous`; a verified pidfd stops only native
Shell PID 5468/start ticks 1250435 before `current` switches, then sends SIGKILL.
All five new destination files have already passed their public hashes, sizes,
ownership and executable modes. The active beta stays complete and unchanged,
while `previous` temporarily names that same beta; the original prepared
rollback generation remains on disk. The private download and staged generation
remain after process death. The installed receipt and real note are unchanged.
This is process-interruption coverage on actual UBIFS, not physical power-loss
coverage.

Normal session launch reaches the focused 480×272 beta launcher in PID 9327 on
boot `2fa3e6d0-30d2-46b1-885e-e817413e034f`, with no PocketHome or owned zombie.
All 93 original saved entries and eight original settings still match. The
subsequent genuine Check removes the stale stage through the beta's older
completion probe. An ordinary public retry observes a new private download,
reuses the already verified 1.0.1 destination, completes in 112.586 seconds,
retains beta as previous and removes all staging. All five public hashes,
versions, owners and 0755 modes pass; prepared helpers and receipt stay unchanged.
A normal session stop/launch runs public 1.0.1 in PID 11058 with focused readiness,
no PocketHome or owned zombie, and all saved entries/settings still unchanged.
The unfixed beta in-place relaunch is not used for this recovery.

The complete-generation activation and pointer-publication functions, and
bundle extraction source, are byte-identical between this published baseline
and current source. The beta's obsolete inventory bridge and prerelease selection
are explicitly qualified; no bridge or test-only update path is added to current
production. This is genuine public OTA fault/retry evidence, not a fresh exact
public-candidate certification.

The first observer times out without any update activation. Its 15-second
confirmation expires during separate UI/screenshot operations; that attempt is
not a product failure or an interruption pass. Batched input confirms the next
actual update, and the corrected observer proves the narrow activation window.
A readiness observer initially names a nonexistent private manifest; the normal
session launch has already succeeded. The corrected manifest observation passes.
Both observer failures remain retained.

The published 1.0.1 notes now disclose the persistent relaunch zombie and setup
cancellation traceback, with a link to prepared PR #8. All 19 numeric REST asset
IDs, sizes and digests still match the original release manifest. An initial
asset observer compares opaque GraphQL IDs with numeric REST IDs and fails;
using the matching REST representation confirms immutability.

Evidence: `official-update-activation-v2.log`,
`official-update-activation-v3-proof.json`,
`ota-activation-source-comparison.json`,
`ota-activation-v3-recovered-state.json`,
`retained-state-after-ota-activation-interruption.json`,
`settings-persistence-after-ota-activation-interruption.json`,
`official-update-activation-retry-proof.json`,
`ota-activation-retry-public-ready-state.json`,
`retained-state-after-ota-activation-public-retry.json`,
`settings-persistence-after-ota-activation-public-retry.json`,
`relaunch-review-ci-bc84d36.json` and
`published-v101-relaunch-disclosure-proof.json`.

## Exact tagged 1.0.2 certification prerelease

[PR #8](https://github.com/csd113/Vitrallis-Shell/pull/8) is merged at
`eaa11aa52d2c6858d4c79977c3fa595ec42b7f19`. Its tree equals reviewed/tagged
source `6f0a155b53d84c68423e2240f57127e463cf0980`, which has all six successful
Rust 1.91.0/1.99.0/stable push/review jobs. The
[exact tagged workflow](https://github.com/csd113/Vitrallis-Shell/actions/runs/37230948676)
passes full validation, x86-64/ARMv7 builds, utility probes, packaging and legal
notice checks. All 19 assets match GitHub IDs/sizes/digests; eight checksum
sidecars, six helpers and three legal files match tagged source. Both bundles
have five verified ELF members and no trailing data. ARM bundle SHA-256 is
`1052c7a2c6617c6d4c1ed8131b662f6457b5289dc12eac5cc955d5f3c2d07a24`.

Actual installation and repeat installation of the exact tagged ARM bytes and
helpers pass on PocketCHIP, retaining prepared `656c0850…` as the distinct
previous generation and all 93 tracked saved entries. All five executable hashes,
versions, owners and 0755 modes pass; installed helpers match with 0644 modes.
All eight original settings match. Native Shell hardware graphics self-test and
Terminal/Notepad/Files hardware smokes pass on Mali400 at 480×272, with VSync
and no fallback. PID 28070 owns actual X11 focus on the fullscreen launcher,
with no PocketHome or owned zombie, before and after the graphics probes.

The release is published as a certification prerelease at 2026-10-04T20:26:52Z.
All 19 asset IDs/sizes/digests remain unchanged by publication. The normal
uninstaller and guarded removal of owned root integration leave the core root
and all eight owned root paths absent. Stock reboot
`8a0973f7-1162-4296-bec1-3dcaa4a2f448` reaches PocketHome in 176.914 seconds,
with no Vitrallis process or core integration. Anonymous device metadata verifies
all 19 public assets. The exact unmodified README command succeeds from this
fully clean state and on completed repeat execution, including fresh GPU/media
provisioning and its required reboot. All five executable versions/hashes/modes,
five installed helpers, private core directories/receipts and saved config match.
Restoring only retained App Center/Tor state preserves the freshly installed
public core; all 93 original saved entries still match.

Public-install reboot `6427f918-0efa-4099-aff1-b1b833a57cc5` reaches Ready in
185.640 seconds, PID 1083. Three observations at 0, 15 and 30 seconds confirm
480×272 fullscreen, sustained actual X11 focus, no PocketHome and no owned zombie.
Terminal accepts `pwd`, reports `/home/chip`, reaps its PTY and closes normally.
Notepad saves the new 26-byte `public 1.0.2 smoke.txt` with mode 0600 and
`chip:chip` ownership; the earlier note's hash is unchanged. Files enters `bin`
and returns to `/home/chip` with Escape, then closes through its keyboard footer.
The intervening stock session again leaves OS-saved backlight brightness zero,
clamped to 1/10 at startup. This repeats the separately qualified stock-interval
observation; Shell config bytes and other controls are preserved.

Final navigation automation accidentally launches Bitcoin while changing pages.
After its verified owned Python process receives normal SIGTERM, Settings remains
visible and Awesome considers Shell selected, but actual X11 input focus is
**zero**. Keyboard activation fails while touch still works. A 60-second readiness
observer fails; this is a real app-exit input blocker in public 1.0.2. The remaining
owner cold-power check is postponed until that correction is released. Stable
readiness is not declared. The first asset observer uses a draft tag REST lookup
which returns 404; verification through the authenticated numeric release ID
succeeds. This is an observer correction, not a product failure.

Evidence: `v102-pr-merged.json`, `v102-candidate-identity.json`,
`v102-review-ci-ledger-head-v4.json`, `v102-tag-workflow-final.json`,
`v102-artifacts-manifest.json`, `v102-publication-proof.json`,
`v102-device-install-proof.json`, `v102-tagged-installed-ready-state.json`,
`settings-persistence-v102-tagged-installed.json`,
`v102-native-graphics-proof.json`, `v102-after-native-graphics-state.json` and
`v102-draft-tag-lookup-failure.json`.

## App-exit keyboard focus correction

Public 1.0.2 reaches the focused launcher after its clean public reboot, but its
final Python app-exit test exposes another input transition defect. A FocusGained
transition can mark Shell ready and clear foreground process ownership before
the completed app is reaped. The subsequent exit skips the existing guarded focus
repair, leaving Awesome's selected Shell client different from actual X input
focus. Touch remains usable; keyboard activation does not.

The correction keeps window raising conditional on the existing active-app policy,
but runs the existing guarded X focus repair on every app exit. The guard preserves
another selected app. No dependency, version, installation helper or data-format
change is made. The real Awesome/X11 regression reproduces cached Shell selection
with actual X focus zero after Home/background return: the old build times out;
the corrected build restores keyboard focus and preserves another app's focus.
Existing native launch, Home, resume, close and crash-recovery cases also pass.

Full `sh scripts/validate.sh` passes again on macOS/Linux: formatting, checks,
strict Clippy, 380/382 Rust tests, 181 Python cases (nine/eight exclusions),
release builds, renderer/SDL smokes and documentation checks. ARMv7 build and
five-member packaging pass. Initial disposable fixture runs lack Pillow; installing
its test runtime allows the unchanged regression to run. One packaging probe uses
an incompatible QEMU loader and segfaults on the unchanged published Arti executable;
using the matching system loader and Cortex-A7 model passes all five probes.
These failed harness attempts remain retained.

Prepared bundle `f9db32028969f4a18e770a7270f23231d676e564fbfb43767321d88cdd1354cb`
is installed through the normal installer, retaining immutable public 1.0.2 as
previous. This bundle still reports workspace version 1.0.2 and is explicitly
**unpublished correction evidence**, not the published asset. Its five binary
hashes/versions/owners/modes pass. On device, a real Calculator Tk window is
launched, returned Home and backgrounded while Settings is open. X focus is then
set to None while Awesome still selects Shell; a normal WM close of that owned
Calculator window restores actual Shell keyboard focus automatically in 1.132
seconds. Calculator is reaped, keyboard Enter opens Display & Sound, and Escape
returns to the launcher. All eight settings and 93 saved entries match; no
PocketHome or owned zombie remains. Final public corrected candidate and owner
cold-power acceptance remain required. No fresh release version is authorized yet.

Evidence: `exit-focus-validation-summary.json`,
`exit-focus-regression-baseline-v3.log`,
`exit-focus-final-x11-artifacts/stock-session.json`,
`exit-focus-prepared-manifest.json`, `exit-focus-device-install-proof.json`,
`exit-focus-device-regression-proof.json`, `exit-focus-post-regression-state.json`,
`settings-persistence-exit-focus-post-regression.json`,
`retained-state-exit-focus-post-regression.json`,
`v102-public-clean-proof.json`, `v102-stock-reboot.json`,
`v102-public-readme-proof.json`, `v102-public-first-verification.json`,
`v102-public-repeat-verification.json`, `v102-public-installed-reboot.json`,
`v102-public-notes-proof.json`, `v102-accidental-app-focus-state.json` and
`v102-stock-interval-brightness.log`.

## Actual low-NAND Shell update and fixture cleanup

An owned, private incompressible filler on the real `ubi0:rootfs` UBIFS reaches
112 MiB free without crossing its 48 MiB reserve. Filling takes 1609.900 seconds;
the kernel reports one writeback stall over 120 seconds for the fixture Python
process. The fixture continues making progress and finishes. This warning is
retained as hardware storage behavior, not suppressed or counted as a Shell
crash.

From the genuine published beta with prepared helpers, actual Settings installs
public 1.0.1 in 96.977 seconds. The observer verifies the filler inode, size,
owner and mode throughout 4053 samples: initial free space is 117,432,320 bytes,
minimum is 85,315,584 bytes, and final is 115,740,672 bytes. A new private download
is observed; all five public executable hashes, sizes, versions and 0755 modes
pass. Beta becomes previous, staging is removed, prepared helpers and receipt
stay unchanged, and the real note is unchanged. The already verified final target
generation is reused after mandatory complete new staging extraction, sync and
five version probes. The ready function is byte-identical to current tagged
source, as recorded in `low-nand-staging-source-proof.json`. The absent-final-
destination rename branch under pressure and physical power-loss durability
are not inferred. Activation/extraction code
matches current source as recorded above; the beta selection/bridge differences
remain qualified.

A guarded stop marker lets the same fixture clean up normally. The filler,
readiness record and stop marker disappear, its process exits, and free space
returns to 1,118,363,648 bytes. Normal launch of public 1.0.1 reaches actual X11
focus at 480×272 in PID 20239 with no PocketHome or owned zombie. All 93 tracked
entries and eight original settings match. The ordinary prepared installer
then restores corrected bundle `656c0850…`, retaining public 1.0.1 as previous.
PID 21103 reaches focused readiness; all saved entries/settings still match.

Earlier UI harness attempts either expired the 15-second confirmation or sent
input from the wrong screen. The latter queued Tor startup; stopping the verified
session cleared it before retry. The successful Restore baseline uses three
separate input calls within the confirmation window and verifies actual pointers.
Two commands initially treated the host UI helper as a remote file and failed
without performing input. These attempts do not count as update passes.

Evidence: `official-update-low-nand-proof.json`,
`shell-update-pressure-detached.log`, `low-nand-fill-kernel-authenticated.log`,
`low-nand-pressure-cleanup-proof.json`, `low-space-three-call-restore-proof.json`,
`low-nand-public-ready-state.json`, `retained-state-after-low-nand-public-update.json`,
`settings-persistence-after-low-nand-public-update.json`,
`low-nand-final-prepared-return-install-proof.json`,
`low-nand-final-prepared-return-state.json`,
`retained-state-after-low-nand-final-prepared-return.json` and
`settings-persistence-after-low-nand-final-prepared-return.json`.

## Authorized 1.0.2 prepared validation

The owner explicitly authorized Shell 1.0.2 on 2026-10-04. Source
`656abc2cb2f48617fc938d14a99659c5c8a7ecfe` includes the relaunch and cancellation
corrections and the authorized workspace version. Complete `sh scripts/validate.sh`
passes on macOS and native Linux: formatting, locked workspace checks, strict
Clippy, 380/382 Rust tests, 181 Python discovery cases, release builds, renderer
and SDL smokes, script checks and all 59 local Markdown files. Mac Python has
nine explicit exclusions; this Linux container has eleven, including three
Lua execution tests because its interpreter is absent. Mac and exact-source CI
cover those Lua tests. The Linux harness initially hides Cargo with a cache
mount, then lacks rustfmt/Clippy; correcting the mount and installing the standard
toolchain components allows the unchanged source to pass. Failed harness logs
are retained. No assertion is weakened.

All six exact-source Rust 1.91.0/1.99.0/stable push/review jobs pass. The ARMv7
release build and five-member packaging/version checks pass, producing prepared
bundle `7cc156ce150a58f012080de775c33b5aba0eedd4ee49ebe8d24b1d12e5a61ce2`.
This is prepared validation; exact tagged release assets, clean public installation
and final owner physical acceptance remain required.
Evidence: `v102-authorization.json`, `v102-canonical-summary.json`,
`v102-review-ci-final.json`, `v102-prepared-artifacts-proof.json` and
`v102-prepared-arm-build.log`.

## Shell certification scope

On 2026-10-03 the owner clarified that Shell release readiness takes priority.
App Center certification covers Shell installation, update, launch, process
lifecycle, recovery and saved-data preservation. Further app-internal functional,
GPU-accuracy and performance work belongs to the separately authorized Apps audit.
Music rapid switching, Monitor's Tk shutdown correction and Firefly frame-rate
investigation are retained follow-ups; none is evidence of an unresolved Shell
defect. The owner authorized Monitor 0.4.2; that app release is deferred while
Shell owns the device.

The remaining Shell critical path is the exact tagged/public 1.0.2 candidate
and final physical smoke. Actual UBIFS activation interruption, normal public
retry and the low-NAND update pass with the qualifications above. The relaunch
correction passes a production-source held-query trial, and exact tagged 1.0.2
installation, repeat and hardware smokes pass. Clean public-candidate
installation and final owner acceptance remain required.
Controlled brightness persistence passes with the qualification above. The clean
public 1.0.1 README install, repeat execution, reboot and published artifact
verification pass as recorded above. Earlier final-production
activity-soak checks pass. The replacement startup is installed, with renewed
owner cold-power/input/display acceptance and a software reboot of the newly
rebuilt candidate passing as recorded below.
Prepared local installation is not a substitute for the public route. No final
stable release or readiness declaration is authorized by this scope correction.

## Cold-boot keyboard failure and PocketHome replacement

The owner performed a normal physical shutdown and power-on, then reported that
touch navigation worked but keyboard navigation did not. Boot
`ec128189-6679-45c6-9411-8490119e4101` reached the 480×272 Vitrallis launcher
(native PID 909, window 10485773), while PocketHome PID 845/window 6291466 also
ran. Awesome reported the Vitrallis client focused, but `XGetInputFocus` returned
the hidden PocketHome window. The previous foreground checks therefore do not
certify keyboard delivery. Manual focus resets restored delivery temporarily;
both immediate and deferred focus-hook experiments failed three out of three
reproductions and were removed. Original physical cold-boot keyboard acceptance
is **FAIL**; renewed acceptance of the replacement startup is recorded below.

The owner explicitly requires PocketHome not to run when Vitrallis is installed.
The repair replaces the recognized PocketHome launch in the existing Awesome
configuration with Vitrallis, removes the competing-desktop focus guards and
restores the exact original command on uninstall. First installation defers
launch until reboot if PocketHome is still running in the current login;
the runtime refuses to launch alongside that desktop. No PocketHome binary or
unrelated OS configuration is removed. The prior installed helper successfully
removed its old managed integration, and the original Awesome configuration was
verified restored before testing the new installer. The replacement installer
then passed on the device and installed helpers byte-identical to the staged
checkout, retaining all saved/config paths, bytes, owners and modes, checked app
files/integration and seven unrelated receipts.

Two new software reboots, `cf8113b3-705c-416c-bdd1-0579bfa0172b` and
`d5004f81-2a37-4671-bfcf-11c26d9569dd`, each retain one native Shell identity and
no service-owned zombies. At 0, 15 and 30 seconds after the first native-window
observation, no PocketHome process/window exists and `XGetInputFocus` agrees with
the focused 480×272 Shell window. Current-session Ready is established at the
15- and 30-second samples. The first observer could match an earlier retained
Ready line at its initial sample; its timings measure native-window observation,
not current-session readiness. The original observer is retained and its future
readiness check now isolates the current compositor/session log segment.
Synthetic arrows, Enter and Escape visibly navigate the launcher, open/close
Notepad and Settings, and Home returns to Shell while retaining Notepad's same
PID. Resuming Notepad and normal clean close pass. Notepad owns actual X11 focus
while foreground. Its first proof expected an incorrect window title; the
retained data and corrected proof use actual title, PID, executable and class.
The owner then performed another full power-off/on and reported: keyboard and
touch work, the display looks good, and the device boots directly into Vitrallis.
Read-only inspection of boot `2d609fb9-3c9f-4d65-a5f9-d24aa0b4c61e` confirms
current-session Ready, one Shell PID 912, no PocketHome process/window or
service-owned zombies, and actual X11 focus on the 480×272 Shell window.
Renewed physical cold-start keyboard/touch/obvious-display acceptance is **PASS**
for bundle `ea6fc653…` and startup helpers from `a57b107830b802372afe72fcb584386f1c3f6abd`.
The old draft assets are superseded by the repaired candidate recorded below.
Keyboard selection and activation of Exit Vitrallis remove every owned process
and window without starting PocketHome. The restored original Home binding
then starts a new ready Shell (PID 4004), with matching actual X11 focus and no
PocketHome process/window. UI automation was paused for the owner's
cold-power/input/display test and resumed after the successful report.

Full `sh scripts/validate.sh` passes on macOS and native Linux AArch64, including
required formatting, strict Clippy, workspace tests, release builds, smokes,
Python/shell syntax and 59 Markdown files. Python discovery runs 177 cases with
nine macOS and six Linux explicit environment exclusions. Both real Awesome/X11
fixtures run in Linux, including absence of the fake PocketHome launch and one
Vitrallis startup. The first Linux attempt lacked rustfmt and stopped before
source validation; the corrected disposable image includes rustfmt/Clippy and
the required GUI tools. The initial Mac run failed three stale first-launch
expectations, which were updated to test the new reboot deferral and exact query
and launch behavior. No test failures are hidden.

Evidence: `keyboard-cold-boot-x-input-before.json`,
`keyboard-cold-boot-direct-focus-experiment.json`,
`keyboard-cold-boot-live-nil-hook-experiment.json`,
`keyboard-cold-boot-live-deferred-hook-experiment.json`,
`pockethome-replacement-old-uninstall-v2.log` and
`pockethome-replacement-device-preflight.json`,
`pockethome-replacement-install-proof.json`,
`pockethome-replacement-two-reboots.json`,
`pockethome-replacement-reboot-ready-proof.json`,
`pockethome-replacement-persistence-proof.json`,
`pockethome-replacement-notepad-focus-proof.json`,
`pockethome-replacement-after-exit.json`,
`pockethome-replacement-after-home-relaunch.json` and the
`pockethome-replacement-canonical-*-v2.log` records.
The renewed physical report and read-only observations are retained in
`pockethome-replacement-owner-cold-acceptance.json` and
`pockethome-replacement-owner-cold-boot-inspection.json`.

## Rebuilt startup-repair candidate

Source `a57b107830b802372afe72fcb584386f1c3f6abd` has all six Rust
1.91.0/1.99.0/stable jobs passing in the
[PR workflow](https://github.com/csd113/Vitrallis-Shell/actions/runs/37176307013)
and [push workflow](https://github.com/csd113/Vitrallis-Shell/actions/runs/37176304262).
Locked all-feature release builds and five executable version probes per
architecture pass for ARMv7 and x86-64. ARMv7 bundle SHA-256 is
`07f299b4f3f35198f1ac556aedb476a6c5801983b4eff8253a944ef7d0f843ea`;
x86-64 is `7f1a3e4bdb9246a9b054daaa3afe558471c26cac0a05bc735abe88ac29dcc535`.
The highest referenced glibc symbol is 2.34 in both architectures, within the
declared 2.36 floor. Exact ARM emulation and native x86-64 Shell/utility smokes
pass at 480×272 and 800×480. Native Rust source, assets and dependency inputs
are unchanged from the prior candidate, but the four freshly built native
ELFs have different bytes; those new bytes were installed and tested rather
than assumed equivalent. Arti remains byte-identical.

The repeated installer retains the replacement Awesome configuration exactly.
Software reboot `49f85831-6020-428f-bd2b-b99de95115a1` reaches current-session
Ready with Shell PID 1062. At 0, 15 and 30 seconds, the exact new generation
retains one Shell identity, no PocketHome process/window or service-owned
zombies, and actual X11 focus on its fullscreen 480×272 window. The 185.68-second
request-to-observation interval includes OS boot and USB/SSH availability; it
is not Shell-only launch latency. All five installed executable hashes,
lengths, chip ownership and 0755 modes match the new bundle. All 57 recorded
AppData entries, five config entries, 21 checked Firefly payload entries, three
app-integration entries and seven unrelated receipts remain unchanged.
The earlier manual physical cold test applies to the same repaired integration
with bundle `ea6fc653…`; the fresh bundle's automated software reboot is
recorded separately.

Keyboard navigation opens and closes Notepad and Settings on the fresh build.
Notepad's actual X11 focus matches its native window. Terminal executes `pwd`
and `id -un` through the physical device's PTY, reporting `/home/chip` and
`chip`; HISTFILE is unset before the diagnostic commands. Its normal exit/Close
flow returns to Shell. Files opens the normal home directory, keyboard selection
reaches its Close button, and activation returns to the same Shell PID. Final
inspection finds only the supervisor, compositor and Shell in the owned unit,
no utility processes or zombies, no PocketHome process/window, and actual X11
focus on the launcher. All 93 baseline entries again match bytes, owners and
modes. The current-session log has no unexpected failure or hardware fallback;
four warnings concern the same missing stock `surf` command.
Initial Terminal and Files captures were still the launcher during startup.
Two header comparisons and a foreground guard rejected unready/changed screens
before sending input; subsequent painted controls and window identity were
checked. Those retained observer failures are not completed UI checks.

Shell's exact native `--graphics-test` passes on Mali400 with acceleration,
backbuffering and VSync. All three utilities pass their supported hardware
`--smoke-test` and 480×272 screenshot readbacks; automatic and hardware images
match exactly. Strict software-versus-hardware screenshot equality is **FAIL**:
73 Terminal, 46 Notepad and 92 Files pixels differ by at most one colour level
out of 255, with total absolute channel errors 116, 83 and 171 respectively.
The screenshots were inspected and retain the same visible layout and text.
This is recorded as a small backend colour-quantization limitation, not exact
software parity. No repository test assertion or production rendering was
changed. Initial observer attempts expected the wrong Shell diagnostic format
and passed the unsupported `--graphics-test` option to Terminal; those failures
are retained separately from the valid hardware probes.

Evidence: `pockethome-replacement-a57-ci-jobs-proof.json`, the
`pockethome-replacement-*-abi-proof.json` and exact-architecture smoke logs,
`pockethome-replacement-final-build-install.log`,
`pockethome-replacement-final-build-reboot.json`,
`pockethome-replacement-final-executable-proof.json`,
`pockethome-replacement-final-persistence-proof.json`,
`pockethome-replacement-final-notepad-focus.json`,
`pockethome-replacement-final-terminal-focus.json`,
`pockethome-replacement-final-native-ui-proof.json`,
`pockethome-replacement-final-ui-data-log-proof.json`,
`pockethome-replacement-final-native-graphics-v2.log`,
`pockethome-replacement-final-native-utility-graphics-v2.log` and
`pockethome-replacement-final-native-pixel-comparison.json`.

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
The earlier download-error bundle has digest
`b9c7bf9367ec57bdaaf1fe1396ff29751638be5ebca248b8b3406c4ea65fd0de`.
Normal-user installation under `umask 077`, runtime/AppData ownership checks and
23.95 seconds of software-launch foreground observation passed. The original
saved note remains private and unchanged. These are prepared candidate tests;
the corrected public installation route and final-candidate reboot remain pending.
The earlier Notepad save-error bundle has digest
`69a363acef44157eabef4f7266f2b856d18707fe9fece5fdc8d3243d80cd4139`.
Its normal-user installation and helper/runtime/AppData integrity checks pass.
Places settings, both cache files and its private Unicode marker retained their
exact hashes through this Shell replacement and the next offline reboot.
The Settings Restart action, confirmed with the keyboard after its safe Cancel
default, produced a new boot ID with Wi-Fi disabled. Automatic launcher focus
then remained uninterrupted for 126.56 seconds without injected input. The
observer first saw the launcher at 186.03 seconds after observation began; this
is a measured startup observation, not a claim of fast boot. Root status, private
ownership, 12-hour time and the 1800-second display timeout survived. An earlier
confirmation expired during harness inspection and did not reboot; its
observation is excluded from reboot certification.

The earlier radio-readback bundle, built from exact code `ae343ad`, has digest
`d5dfe623f631b8a48b28c9304b3f5261006f7d872e415018adb6ba68c71325a4`.
All seven artifact sidecars, normal-user installation under `umask 077`, helper
hashes, runtime ownership, root-status readability and persistent data pass.
The installer first correctly refused replacement while Shell was open, leaving
its generation unchanged; normal UI Exit completed service cleanup before retry.
An SSH launch missing XAUTHORITY was a harness-context failure; launch with the
existing desktop's verified display/authentication environment succeeded. Wi-Fi
off/on reports Saved with matching authoritative readback. This is a prepared
software launch on the same boot, not an additional reboot or public-route pass.

The current Settings-utility return bundle, built from exact code `5c56067`, has
digest `e5bdb419b999e08519ed0cc8de232b3f48fad869d9917be5a11ca0f3340e3a7c`.
All seven artifact sidecars, normal-user installation under `umask 077`, helper
hashes, root-status access, runtime ownership and persistent data pass. The
prepared complete ARMv7 bundle contains the five release executables and passed
three native smoke scenarios. Its timezone authentication utility exited with
status zero after changing Vancouver to Whitehorse and after restoring Vancouver;
both returns reopened Date & Time with the actual zone and “Time zone refreshed.”
This is a prepared same-boot installation and hardware retest, not a final public
installation or cold-power pass.

Twelve actual reboots have completed so far: the initial published-beta boot,
recovery from the development test harness's RAM-backed `/tmp` exhaustion, two
returns to the original desktop during complete cleanup, the corrected root
provisioning boot and seven startup-diagnostic/candidate boots. These are software reboots;
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
- A separate continuous 30-minute run of exact Shell code `bce0d86` collected
  61 samples over 1800.25 seconds after the clean offline reboot. The Shell PID,
  process start time, generation and boot ID stayed fixed, with no service-owned
  zombies in the samples. It included Places launch/settings/exit, offline
  uninstall/cancel, Wi-Fi recovery, background reinstall, Notepad permission
  failure/Save as recovery and Firefly launch. Shell CPU averaged 2.57%; RSS
  ranged from 37,032 to 69,208 KiB and ended at 65,420 KiB after catalog/install
  activity. Awesome stayed within 36,336–36,896 KiB, available RAM remained at
  least 215,512 KiB and the private log grew 20,807 bytes. This single activity
  sequence does not establish bounded memory over repeated install cycles or
  certify a future final revision; repeated-use investigation remains pending.
- A further continuous run of exact `ae343ad` collected 61 samples over
  1800.16 seconds during NAND writes, Notepad fault/recovery and control tests.
  The Shell PID/start time, generation and boot ID remained fixed; no
  service-owned zombie appeared in the samples. Shell RSS stayed at 68,020 KiB
  and average CPU was 0.93%; Awesome RSS was 36,388–37,540 KiB, available RAM
  stayed at least 188,508 KiB and the private log grew 22,819 bytes. Actual focus
  query timeouts and the timezone-return failure below are retained. Process
  stability does not make those failures pass, establish repeated-install
  memory behavior or certify the newer revision.
- Exact `5c56067` then collected 61 samples over 1800.09 seconds during Settings
  navigation and managed Carousel launch, upload, settings and Home/resume tests.
  The Shell PID/start time, generation and boot ID remained fixed, with no
  service-owned zombies in the samples. Shell RSS stayed at 38,580 KiB and CPU
  averaged 0.69%; Awesome RSS was 36,500–37,332 KiB, available RAM remained at
  least 215,420 KiB and the private log grew 18,956 bytes. Carousel's retained
  failures below qualify this activity run. This does not establish bounded
  memory over repeated installations or certify a future final revision.
- Bitcoin and Carousel's scoped source changes are published for review at
  `b1460ed1d7b373719e0edfc1d4fb5fbc9e29e504`, with the separate catalog/history
  commit `2c66c4dfd7741429ef0aebffaf804b2e992a5e59`, followed by the exact Places
  mirror/catalog publication at `8c0a247d304524552d5cc66d906089214cdefa28`, in
  [earlier Apps PR #18](https://github.com/csd113/Vitrallis-Apps/pull/18). Catalog
  pins match the published payload bytes; every other catalog entry and all
  installable flags are preserved. The earlier submission's remote CI passed;
  the published draft tree passed pinned-source validation, changelog policy and
  74 tooling tests locally. Its remote runtime checks then failed on a relative
  Places asset root; that failure is retained. A separate candidate branch starts
  from `2c66c4d`, before the earlier Places preparation, preserving the original
  branch, draft and all seven working files without rewriting history. Its
  corrected package/catalog commit is `33b8447a4190fd3ce6b2cb899646b57a2c8a9df0`,
  reviewed in [Apps draft PR #19](https://github.com/csd113/Vitrallis-Apps/pull/19).
  The superseded PR #18 was closed after the replacement passed; its branch and
  prior failed checks are retained.
  Pinned-source validation, 74 tooling tests and changelog policy against both
  the branch base and current main pass. All six exact-head remote Python
  3.11/3.13/runtime and policy jobs pass. Public main is unchanged. Coordination
  preserves the other Apps writer's work; that writer also recorded the owner's
  Codex code/artwork confirmation.
- The earlier PocketCHIP Places 0.11.2 correction was committed as
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
  A later focused startup-wording correction distinguishes the asset working
  directory from persistent settings/cache. Its source is
  `368dd1f0c45ca81a0e7f29041eb4bbfe33846b70`, with rebuilt ARMv7 binary
  `39c7431b4775c84d2c8f5ccab03fcedbace11c54c0bf9532a01029428fd986a6`.
  All 212 mirrored package files match that source, and full macOS/Linux gates
  and ARMv7 rebuilding pass. The new development fixture preserves the original
  public-cache backup and changes only this package pin/inventory. The older
  installed payload remains intact under the changed-inventory fixture; Open
  and receipt-scoped Remove remain available. Keyboard-confirmed removal retains
  AppData, and reinstall verifies all 201 new payload files, sizes, hashes and
  modes with no extras. Debug and Firefly retain their pinned payloads. All
  persistent data hashes remain unchanged. The latest build restores Nearest
  and VSync, resumes the same PID/start time after Home, then exits normally
  with no app process and unchanged saved-data hashes.
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
  Updated-app launch, keyboard settings change and normal menu Exit passed.
  Settings and both generated lightmap-cache files are private, user-owned and
  stored in canonical AppData; no settings/cache remains in the payload. The
  saved nearest-filter preference, cache and marker survived the next Shell
  replacement and offline reboot byte for byte. Relaunch restored Nearest and
  VSync on the actual Graphics screen, and normal menu Exit left no app process.
  Offline uninstall first defaulted to Cancel, and Enter left the app installed.
  Keyboard confirmation then removed every receipt-owned payload file, receipt
  and launcher while retaining private AppData and unrelated apps. Reinstall
  from the same reviewed source completed in the background while Notepad was
  usable. All 201 payload files again match pinned sizes, hashes and modes;
  generated settings/cache and the marker remain byte-identical. Post-reinstall
  launch restored Nearest and VSync, Home/resume retained the same app process,
  and normal menu Exit reaped it. All persistent data hashes remained unchanged.
  A 60-second menu-background
  render sample used 20.65% process CPU and a stable 58,384 KiB RSS; HUD snapshots
  showed 9–11 FPS. This does not establish sustained gameplay performance.
- Firefly Field 0.3.1 installed from the public catalog, launched with canonical
  AppData as its working directory, resumed the same process after Home and
  exited normally through Escape. Its renderer reports hardware GLES2/Mali400,
  480×272 and requested VSync. HUD snapshots showed 4 FPS at startup and 1 FPS
  after resume, with a 43-second process sample at 40.5% average CPU and 45,100 KiB
  RSS. These observations were taken before repairing the screenshot harness
  leak described below, so they do not establish clean-device performance.
  Clean-boot managed HUD snapshots reproduced 3–4 FPS. A 60-second active-field
  sample used 61.41% process CPU with stable 45,104 KiB RSS. A separate instrumented
  diagnostic attributed about 63% of render CPU to Python sprite packing; SDL
  presentation itself was inexpensive in that diagnostic. A developer packing
  correction preserves all positions, UVs, colors and indices across 5,126
  randomized quads and 250 frames with reused slots. Isolated PocketCHIP packing
  CPU improves 2.28×. Matched seeded standalone runs improve from 7.67 to 12.15
  FPS and from 65.74% to 55.24% process CPU over a minute each. These standalone
  measurements do not replace managed-package acceptance. The reviewed source
  and three added geometry/cache/bounds regressions pass all 21 app tests.
  Firefly release-version preparation is awaiting the owner's explicit approval;
  its public 0.3.1 payload and catalog are unchanged. Final managed lifecycle,
  update and uninstall/reinstall coverage remain pending.
- Prepared Carousel 0.4.3 installed through the actual App Center from immutable
  source `b1460ed1d7b373719e0edfc1d4fb5fbc9e29e504`. Completion with its committed
  receipt was observed while the owned NAND-pressure filler remained present,
  with 118,513,664 bytes free. All 29 payload files (323,461 bytes) were verified
  after pressure cleanup. Its private app-local runtime contains 1061 entries
  totaling 24,915,555 bytes, with qrcode 8.2, Pillow 11.1.0 and packaging 25.0;
  system Python still has no qrcode. No unsafe entries or pending runtime remain.
  Installation creates an empty private AppData directory; the owner's deleted
  media/preferences were not restored. An initial harness incorrectly expected
  that directory to remain absent; its failure is retained separately.
  Actual managed launch uses the app-local Python and canonical AppData as its
  working directory. The real LAN browser created a Unicode-named test collection,
  saved eight disposable PNG/JPEG/GIF/animated-WebP/VP8/VP9 files and rejected a
  corrupt upload. Reordering and shared settings saved privately; settings,
  metadata and media survived normal exit/relaunch. Home/resume preserved the
  exact PID/start time and data hashes. Normal Escape exit returned status zero
  and left no app or service-owned decoder process. An authenticated device API
  download verified all eight ZIP entries and original hashes. The browser said
  “Folder download ready,” but its automation download event timed out; that
  harness observation is retained separately. After relaunch, the previous code
  and unauthenticated requests returned 401, while wrong Host/Origin returned 403.
  Paused native still/GIF frames and all five playback controls were inspected;
  the renderer reports Mali400 hardware presentation and software video decode.
  Physical motion/tearing acceptance, managed update and uninstall/reinstall
  certification remain pending. These are newly generated test media, not the
  owner's deleted files.
- Carousel's first managed run reported VP8 unavailable and later surfaced
  “WebM decoder stalled.” Settled exact probes and a direct three-frame native
  decoder test pass; a second managed launch reports VP8/VP9/WebP ready. The
  first failure's cause was not logged and remains unexplained. Some unoverlaid
  GPU screenshots are black, while paused captures show images; physical display
  observation is requested before attributing this to capture or presentation.
  The second launch's focused native window was first observed at 51.019 seconds
  from the click, including SSH-query overhead. A read-only, bracketed import
  profile took 12.696/13.016 seconds and 9.423/9.455 CPU seconds with the managed
  cache prefix, versus 6.274 seconds and 2.834 CPU seconds with ordinary caches.
  This explains part of the cost, not the whole launch delay. The namespace's
  stale-bytecode protections remain intact pending further investigation.
  A subsequent actual managed launch was measured with reversible, read-only
  Awesome window-event observers and a normal-user process sampler. The process
  started 0.260 seconds after the click; focus arrived at 15.817 seconds. The
  last pre-focus sample had used 9.88 CPU seconds and 27,088 KiB RSS. The observer
  was removed afterwards. This demonstrates variable startup latency; it does
  not explain or erase the earlier 51-second observation.
  The longer playback run also surfaced “File missing, corrupt or no longer
  readable.” All eight fixture files remained byte-identical. Two complete
  direct passes through the installed production decoder succeeded for every
  fixture, including 60 frames per moving clip, 12 frames per animation and
  three frames per one-frame WebM. That diagnostic bypasses Tk/GPU presentation
  and playback pacing, so the actual playback failure remains unexplained.
  The temporary repeat setting was restored from ten to three, and two further
  managed Escape exits returned status zero with unchanged private test data.
  The stale web footer also displays 0.4.1; its two-file correction passes all
  271 tests with the existing skip and is kept uncommitted. Because replacing
  the pinned 0.4.3 bytes requires a new immutable package version, 0.4.4
  preparation is awaiting the owner's explicit authorization.
- Carousel's actual offline receipt-owned removal passed after inspecting the
  visible Cancel default, cancelling with Enter, then explicitly selecting
  Uninstall by keyboard. NetworkManager reported Wi-Fi disabled and an external
  TCP connection failed with “Network is unreachable.” The manifest, receipt,
  pending marker and generated launcher were removed; all 15 private AppData
  entries and 236 other installed-app file hashes were unchanged. The current
  uninstall retains 934 app-local Python runtime files totaling 24,915,555 bytes
  because those generated files are outside its package receipt. Full dependency
  cleanup acceptance remains open. A checker that required the entire app
  directory to disappear failed; it is retained separately from the qualified
  receipt/payload result. Settings restored Wi-Fi and a GitHub TCP connection
  succeeded on the same boot. Actual App Center reinstall of the same immutable
  prepared 0.4.3 verified all 29 payload hashes, recreated its launcher and kept
  those 15 data entries and 236 other-app hashes unchanged. It reused the
  retained runtime; this is not fresh dependency provisioning or a public-main
  catalog installation. Post-reinstall Open launched a new, focused normal-user
  process in canonical AppData. Native Home showed all eight items; native
  Settings and the authenticated device API agreed on five seconds, three
  repeats and the saved order/loop values. VP8/VP9/WebP decode checks were ready.
  Normal Escape exit returned status zero, left no process-group member and
  retained every fixture hash; Wi-Fi and the current Shell generation stayed
  restored.
  Shell RSS after completion was 39,584 KiB, compared
  with 38,580 KiB in the preceding soak. One lifecycle is not evidence that
  repeated installs have bounded memory use.
- A subsequent host-only correction includes Shell-generated dependency files
  in the uninstall journal while retaining AppData, custom `.venv` environments
  and unmanaged files. Runtime backups are reclaimed only after durable
  completion; interrupted-operation backups remain recoverable. Empty runtime
  directories remain for recovery and can be removed before reprovisioning.
  Five new Rust regressions cover interruption/rollback, data retention,
  oversized files, links, empty-directory reprovisioning and completed-backup
  reclamation. The exact offline pip fixture also checks private staging under
  shared umask 002. It exposed a group-writable copied activation template;
  staging now normalizes those copied modes before publication.
  Complete macOS and native Linux/aarch64 validation pass with 373/374 Rust
  tests (9/12 existing ignores), 173 Python tests (9/10 environment skips),
  two renderer tests (one existing skip), release builds and native smokes.
  ARMv7 rebuilding, three native smokes and all seven package sidecars pass;
  the prepared 26,826,084-byte bundle hash is
  `4c4fa2aca954d2c4acb9a887b30e17b0df743b39c9805861147f01bdff9a0c50`.
  An emulated amd64 container attempt failed three existing process launch/
  identity tests. A standalone probe reproduces Rosetta reporting successful
  spawn followed by exit 127 for a nonexistent executable and exposing Rosetta
  as the process executable; native Linux reports ENOENT and the actual
  executable. Those unchanged tests pass in native Linux. All six native amd64
  Rust 1.91.0/1.99.0/stable remote jobs pass for exact `c9f70fb` (PR run
  37143031683, push run 37143028507); the emulated failures remain recorded.
  This correction has not been installed or tested on PocketCHIP: the device
  was handed to the existing Apps worker for its hardware audit. Dependency
  cleanup, fresh reprovisioning, low-space/fault behavior and repeated-install
  memory acceptance remain open until handback and physical retesting.
- Host fault testing then reproduced two resource leaks: failure while copying
  a later journal image retained an earlier backup outside a pending journal,
  and successful rollback retained disposable runtime backups. Staging now
  publishes its bounded path/hash/mode manifest before copying images, becomes
  pending only after all images are durable, and recovers interrupted staging
  without writing application files. Cleanup validates the complete scope and
  published image integrity, preserves later edits and unrelated files, and
  removes only its declared temporary scratch names. Exclusive temporary-file
  creation failure also preserves the pre-existing file. Runtime backups are
  reclaimed after durable rollback as well as commit; pending backups and
  ordinary payload backups retain their existing policies.
  A Linux raw-byte runtime filename separately reproduced a JSON serialization
  panic. Journal paths now require UTF-8 and return an error while retaining
  the installed app, receipt and offending file. The regression runs on Linux,
  whose filesystem supports the fixture; macOS rejected fixture creation with
  EILSEQ before reaching uninstall, and that initial attempt remains recorded.
  All new regressions pass without new ignores. An actual 4 MiB native-Linux
  tmpfs reproduced ENOSPC after staging began: both original 1 MiB files and
  modes remained intact, the incomplete marker and owned partial snapshots
  were removed, and free space returned from 4,194,304 bytes to the same value.
  The transient test mount was removed. This is host filesystem evidence,
  separate from the earlier physical NAND tests.
  Final complete `sh scripts/validate.sh` passes on macOS and native
  Linux/aarch64 with 376/378 Rust tests (9/12 existing ignores), 173 Python
  tests (9/10 environment skips), two renderer tests (one existing skip),
  strict Clippy, release builds, native smokes, script checks and document links.
  The final ARMv7 build, three native utility smokes and all seven package
  sidecars pass. The 14-file prepared package includes the 26,842,468-byte
  bundle with SHA-256
  `7910151b0ff16833acb1746ac982ddeb9a8bd9f0eb2a911d824a33608d5b005f`.
  All six native amd64 remote Rust 1.91.0/1.99.0/stable jobs pass for exact
  `391744e7faab5840a3cd5584fe277eef205ed34b` (PR run 37147384379, push run
  37147382585). These follow-up fixes have not been installed on PocketCHIP
  while the Apps worker owns the device. Physical interruption, runtime cleanup
  and fresh reprovisioning acceptance remain open.
- Wireless once reported “Wi-Fi change denied or unavailable” while the same
  screen and independent NetworkManager readback showed Wi-Fi enabled and
  connected. A clean UI off/on retry and six bounded direct radio changes pass;
  the original command failure cause was not logged, so a timeout remains an
  inference. The source correction lets authoritative readback establish success
  after a command error and retains the complete command error in the private
  log. Denied, unconfirmed and disappeared adapters still fail. All four radio
  tests, complete macOS/Linux gates and exact-head remote CI pass. The matching
  ARMv7 bundle is installed; keyboard off/on shows Saved and NetworkManager
  confirms disabled, then enabled/connected on the actual 480×272 screen.
- Actual timezone authentication saved the zone and exited normally but returned
  to the launcher. A focus-gained event made the launcher Ready and cleared the
  active process identity before the exit callback; the callback consequently
  missed the pending Settings utility. The fix retains the reaped child's exact
  identity and matches only the pending internal utility. Timezone completion
  returns to Date & Time, calibration to Device and network management to Home;
  unrelated background exits do not reopen Settings. Two regressions cover event
  ordering, matching and unrelated exits. Exact `5c56067` passes complete host
  gates and all six remote jobs. Both physical timezone changes described above
  returned correctly with visible keyboard selection and normal child reaping.
- Keyboard brightness changed the actual backlight from 1 to 2; touch selected
  maximum 10 and restored the lit minimum 1. A held synthetic Left changed 10
  to 9 once, consistent with Shell's deliberate repeat filtering; an initial
  harness expectation of repeated changes is excluded. Keyboard volume changed
  mixer readback from 57/63 (90%) to 50/63 (79%, displayed as 80%) and restored
  57/63. A mid-test probe that raced restoration is excluded; a separate settled
  read passed. Audible output, the physical Fn-key matrix and battery/power
  transitions remain pending. Clock format, brightness, volume and Vancouver
  timezone were restored after these tests.
- A controlled physical-device Notepad save on an isolated 512 KiB tmpfs with
  only 16 KiB free failed with ENOSPC. The original 64 KiB document, ownership
  and private mode survived, and the staging file was cleaned. Removing the
  owned filler allowed a successful retry through the same editor. This tests
  real kernel I/O and the native UI, not NAND pressure or NAND durability. The
  original generic error dialog lacked save-specific context. The correction at
  `bce0d86` prefixes the cause with “Could not save”, gives a space-recovery or
  writable-folder action and retains the complete quoted I/O error in the
  private session log. The exact ARMv7 build repeated the physical fault: both
  lines fit at 480×272, the log records `StorageFull`, the original note stays
  unchanged and no staging file remains. Freeing the owned filler permits a
  successful retry. Normal exit and complete temporary-volume cleanup passed.
- A disposable NAND-backed folder with mode 0500 produced a real Notepad
  permission-denied save after a one-character edit. Its original file stayed
  unchanged and no staging file remained. The save-specific dialog and writable
  folder advice fit at 480×272; the private log retains `PermissionDenied`.
  Ctrl+Shift+S saved the edited document into a writable folder with user
  ownership and mode 0600. Normal exit and removal of both disposable test
  files/folder passed. Original AppData notes are unchanged.
- The actual UBIFS root was reduced to 67,100,672 bytes free using one private,
  normal-user, incompressible filler, with a 48 MiB hard floor and write headroom.
  Storage visibly reported 67.1 MB and LOW STORAGE at 480×272. The exact prepared
  bootstrap's real preflight refused a 26,805,604-byte bundle requiring
  70,388,424 free bytes, preserving current/previous generations, receipt,
  preferences and the original note. This exercises the actual NAND preflight,
  not a complete public installer or Shell update. Clock 12→24→12 writes passed
  and restored identical preference bytes. Native Notepad saved a one-character
  edit to a disposable 64 KiB document at this free-space level, privately and
  without leftover staging files. The filler was reduced before Carousel's
  app-local runtime installation. Two earlier fill attempts timed out or were
  deliberately stopped before reaching the threshold; both cleaned up and are
  excluded from low-space success. The successful fill's 1200-second hold ended
  with automatic cleanup; the same bootstrap preflight then passed. No fillers,
  markers, disposable notes or pressure mounts remain. A brief SSH read stall
  near cleanup recovered; its cause is not established.
- A separate 4 MiB ext2 loop image on UBIFS NAND, mounted nosuid/nodev/noexec,
  provided isolated real ENOSPC without filling the operating system volume.
  Native Notepad's failed save preserved the original 64 KiB file and cleaned
  its stage; the recovery message fit at 480×272. Removing only the validated
  filler let the same editor save successfully. Find selected actual text;
  a new disposable dirty note exercised safe Cancel and explicit Discard.
  Normal exit, owned-file removal, unmount, loop detach and image/mount cleanup
  passed. Original AppData hashes remained unchanged. This covers NAND-backed
  ext2 I/O, not global UBIFS ENOSPC or power-loss durability.
- The certification screenshot harness leaked one GDK display connection per
  capture, retaining 135 connections in Awesome. This is a harness defect, not
  a measured production leak. Closing the verified temporary connections and
  repairing capture cleanup leaves zero connections open after a screenshot.
  Awesome RSS fell from 161,544 to 94,344 KiB immediately, retaining some
  allocations; available RAM was only 35,500 KiB before cleanup. Earlier
  performance and soak observations require qualification and repetition after
  a clean reboot. This finding does not establish that every UI delay or low FPS
  was caused by the harness.
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
  Linux targets supported by the bundle format. Shell and PocketCHIP Places
  retain 37 distinct exact texts/annotations/excerpts, including compiler-builtins'
  complete AND terms, the LLVM exception, Unicode data, and nested
  musl/Sun/BSD/CORE-MATH notices.
  Existing collected bytes were preserved. The conservative source inventory is
  documented in [Rust runtime licenses](rust-runtime-licenses.md); it does not
  claim every build dependency or math routine is linked, or certify an OS image.
- The prepared x86-64 package was then rebuilt from native code `391744e` with
  Rust 1.99.0 and the canonical locked Arti feature set. All five version probes,
  a reviewed 480×272 demo frame, three native utility software smokes and the
  actual-bundle updater probe pass in the emulated Linux environment. These
  checks do not certify native x86-64 hardware or physical display synchronization.
  The x86-64 bundle is 31,050,416 bytes, SHA-256
  `4c16f74507385f2477c6098e76f2a256096b4044c7e85a61c375ff6e8f081c7f`.
  Together with the matching ARMv7 package recorded above, its prepared release
  inventory has exactly 19 distinct assets, eight verified checksum sidecars,
  five hash-verified executable members per bundle, six helpers matching the
  committed source, and one exact copy each of LICENSE and both notice files.
  ELF inspection confirms both targets' expected machine type, ARM hard-float
  ABI and maximum GLIBC requirements of 2.34, within the 2.36 release contract.
  The selected x86-64 Arti normal/build graph contains 429 packages, including
  three packages absent from the ARMv7 selection; every selected package has
  retained notice Text IDs. Fifteen resolved items were removed from the stale
  missing-evidence list without changing the collected notice bytes. The
  runtime inventory now distinguishes the bundle format's AArch64 support from
  the current workflow's two published targets. This is prepared local artifact
  evidence; final tagged builds, published-byte verification and exact final
  physical certification remain open. PocketCHIP remains with the Apps worker.
- The obsolete beta3.9/beta4 four-executable bridge is removed from the updater,
  installation inventory, release packager and tag workflow. Current generations
  require all five companions and equal semantic versions remain current. The
  historical release record is retained and explicitly separated from the current
  update contract. The retired CLI option now fails during argument parsing,
  before creating output; its initial failing regression is retained. A new
  filesystem regression removes each companion in turn and verifies refusal
  before staging, with the active pointer unchanged. Existing checksum, unsafe
  file, concurrency, rollback and semantic-version guards remain exercised.
  Targeted updater and filesystem suites pass 18 and 23 tests respectively.
  The four update-available frames per OS/CPU profile now say “New version
  available” instead of offering same-version completion. All 333 other frames
  per profile remain byte-identical. The new four-size macOS, Linux AArch64 and
  emulated ARMv7 pages were manually reviewed; Linux x86-64 captures match
  AArch64 exactly. Only these 16 reviewed hashes changed. At 480×272 the macOS
  pixel difference is confined to the status text rectangle (12,60)–(170,68).
  Initial reference failures and harness failures from a missing or reused
  capture directory remain in the evidence; successful reruns use fresh output.
  Full `sh scripts/validate.sh` passes on macOS and native Linux AArch64:
  376/378 Rust tests pass with nine/twelve existing opt-in exclusions, 172 Python
  tests pass with nine/ten existing environment skips, and the native renderer
  suite passes with one existing accelerated-backend skip. The lower Python
  count replaces two obsolete bridge packaging tests with the current CLI
  refusal regression. The rebuilt ARMv7 and x86-64 bundles pass all version
  probes, software utility smokes and their complete renderer reference gates;
  the actual x86-64 bundle updater probe passes with one exercised test and no
  exclusions. The combined prepared inventory verifies 19 distinct assets,
  eight sidecars, five hash/ELF-verified members per bundle and exact helper and
  legal companion bytes. ARMv7 is 26,838,372 bytes, SHA-256
  `5aa1042317fee561de5e172441bb02471177628303a2f92417b2934a182feb53`;
  x86-64 is 31,046,048 bytes, SHA-256
  `633586de1ab624c3ce0f142e0028e97354c87b424d00b3732354aeac9b3f4b55`.
  No version, dependency or notice bytes changed. These are host and emulated
  checks; device verification of this source remains pending while the Apps
  worker owns PocketCHIP.
- The Apps worker's native Tk/Pillow import probe takes 3.665 seconds with the
  generated per-commit bytecode prefix versus 0.793 seconds with the interpreter's
  default caches; CPU time is 3.023 versus 0.557 seconds. The Shell worker inspected the
  successful tool output without accessing the held device. Sketch disables
  bytecode writes before imports, so the private namespace cannot become warm.
  A five-run host import-only reproduction of unchanged Sketch source has median
  times of 0.187 seconds with the namespace and 0.059 seconds with default caches
  plus unconditional hash checks; source compilations fall from 91 to 28. No
  cache files are created. These host figures are not device launch estimates,
  and the native control did not exercise the final hash-checking option.
  Current generated launchers clear inherited prefixes, reuse installed library
  caches, keep bytecode writes disabled and always verify hash-based caches,
  including producer-marked unchecked caches. Managed module cache invalidation
  and unsafe-cache refusal remain in the update path. The obsolete namespace,
  its commit-only launcher argument and its accounting/reserved-path references
  are removed; current consumers use the single new policy. No old-launcher
  recognition or compatibility shim is added. Obsolete pre-release app installs
  must be recreated; separate AppData and custom-launcher protections remain.
  A failing regression reproduces repeated compilation of valid cached library
  source. New regressions execute real generated launchers and verify cache reuse,
  stale unchecked-hash rejection, inherited-prefix exclusion and unchanged cache
  bytes. All six targeted runtime tests and 13 lifecycle tests pass. A macOS
  fixture initially assumed isolated-mode and normal-mode cache defaults were
  identical; the corrected fixture observes the selected interpreter's normal
  default and cleans only its own unique cache subtree. The initial failures and
  an unsupported Serde-derive test attempt are retained. No dependency or version
  changes are made. Full `sh scripts/validate.sh` passes on macOS and native
  Linux AArch64: 378/380 Rust tests, with nine/twelve existing opt-in exclusions;
  172 Python tests, with nine/ten environment skips; and the native renderer
  suite, with its existing accelerated-backend skip. Formatting, locked checks,
  strict Clippy, release/smoke/script/doc gates pass without changing renderer
  baselines. Both five-executable release bundles build and package successfully;
  their combined 19 assets include eight verified checksum sidecars and exact
  current helper/license bytes. The ARM bundle is 26,838,372 bytes, SHA-256
  `ea6fc653952fc591ff9602ee2b740a873e1c8e6d6aec94a799b862a2081013bd`;
  x86 is 31,044,368 bytes, SHA-256
  `635497dbb46b188fab812b7f1bbb8787b9c314e3f3b755ceed142de162260c86`.
  The actual x86 release-bundle update probe passes with no ignored test, and
  all three utility smokes pass for both targets; ARM runs under Cortex-A8
  emulation. Complete native launch-latency tracing remains pending until the
  candidate is deployed.

- The Apps hardware worker released PocketCHIP at 2026-10-03 22:04:02 UTC
  after restoring the original catalog and connected Wi-Fi. The installed Shell
  remains `5c56067`; the private native-test staging area does not replace it.
  All ten cross-built workspace test executables from `dba70da` were verified
  and run as `chip`, one test thread at a time, with temporary files on NAND.
  The first run recorded 378 passes, two failures and twelve existing opt-in
  exclusions. The FIFO catalog regression inherited eight real app installs,
  producing twelve launcher entries instead of four. Its child now uses its
  scratch HOME and clears XDG data/config overrides; the count, nonblocking
  deadline and regular-file refusal assertions remain intact. The corrected
  FIFO test passes on PocketCHIP in 0.57 seconds and the complete desktop group
  passes seven tests with its three existing graphical opt-ins excluded.
  The native package lifecycle fixture initially tried to invoke unavailable
  device `rustc`. Supplying the documented host-built ARM `v1`/`v2` fixtures
  makes the unchanged install/launch/process-detection/update/uninstall test
  pass in 3.81 seconds. Both initial failures are retained. The full library
  rerun passes 316 tests with nine existing opt-in exclusions in 604.34 seconds.
  Together with the corrected desktop group and other workspace executables,
  all 380 default tests pass on native ARMv7; twelve existing opt-ins remain
  excluded. No assertion was weakened and no new exclusion was added.
  Full `sh scripts/validate.sh`
  passes for the three-line test isolation repair on macOS and native Linux
  AArch64: 378/380 Rust tests, nine/twelve existing opt-in exclusions, 172 Python
  tests with nine/ten environment skips, release builds, smokes, strict Clippy
  and 59-file Markdown-link validation. Production source, dependencies,
  versions and renderer references are unchanged. Before the planned candidate
  deployment, hashes, ownership and modes were recorded for all 32 current
  AppData files and four configuration files. A private local archive verifies
  all 36 files and 26 directory modes against that baseline. This includes the
  Apps audit's intentionally corrupt Music fixture; its 0664 mode is preserved.

- The reviewed `dba70da` ARM bundle was installed through the normal-user
  prepared helper route after the native suite passed. Its five executable
  hashes/modes and all helper receipt hashes match the prepared assets. This
  is a candidate installation, not a clean public README installation. The
  test-repair commit `4878c9e` changes only the FIFO test and this ledger;
  production code, helpers, dependency locks and renderer references match
  `dba70da`. All six Rust 1.91.0/1.99.0/stable jobs pass for `4878c9e` in
  PR run 37159837660 and push run 37159834592.
  Four existing graphical opt-ins were explicitly run on real PocketCHIP X11:
  accelerated readback/presentation, fullscreen dimensions, accelerated atlas
  refresh and hardware/software scene comparison. All four pass; this does not
  certify physical scanout/tearing acceptance. The started Shell uses native
  480×272 GLES2, Mesa Mali400 hardware, requested SDL VSync and the supervised
  Present compositor without rendering fallback.
  Actual Settings Restore was exercised from the candidate to the retained
  `5c56067` build and back. Default Cancel kept both pointers unchanged. An
  initial screenshot inspection exceeded the 15-second confirmation deadline;
  that safe expiry is retained separately. A batch of repeated Enter events
  left the confirmation open, so the completed Cancel check used separate
  keyboard events within 3.62 seconds. Each confirmed Restore exchanged the
  two pointers, reported success and required explicit Relaunch. Both relaunches
  executed the expected hash-verified generation and returned to one focused
  480×272 launcher. The same supervised native PID and Linux boot remained;
  these are two process relaunches, not two device reboots. Final verification
  checks both complete five-executable generations and unchanged contents,
  ownership and modes for all 36 saved/config files and 26 data directories.
  The candidate is active again, the earlier build is retained, and no pending
  installation remains. Official download/update, interruption faults and the
  exact clean public route remain open. A native 30.002-second endpoint sample
  of the returned candidate records 0.70% main-process CPU and unchanged
  38,752 KiB RSS. The lighter observer costs 0.032% while collecting endpoints,
  1.16% for the complete sampler child including interpreter startup, and
  2.74% including the Python driver through its result. SSH CPU and Shell
  descendants are excluded; both interpreter startups are included in the
  latter observer figure. No whole-process scan or UI operation occurs during
  the sample. This is observer calibration and a short idle sample, not an app
  benchmark or final stability soak.

- On the installed candidate, Carousel 0.4.3 was removed and reinstalled through
  the real App Center using its unchanged published source pin. Default Cancel
  left its receipt and launcher present. Confirmed uninstall removed all payload
  files, launchers and desktop entries, including 934 derived-runtime files
  totaling 24,915,555 bytes. Its completed journal reclaimed all 934 runtime
  backups and retained hash/mode-verified payload backups. The UI remained
  navigable during removal. Reinstall independently verifies all 29 published
  files and a private normal-user launcher with the new cache policy. All 36
  saved/config files, their modes, and seven unrelated app receipts are unchanged
  across both operations. The regenerated runtime contains the same 934 files
  and total bytes; Python source hashes match, while 445 bytecode files and ten
  generated environment files differ.
  Quiet warm startup observations use two temporary Awesome focus/name events,
  with no process scan or polling during startup. The old launcher reaches a
  focused 480×272 window in 22.31 seconds; the freshly installed launcher takes
  7.15 seconds. Later captures show the usable home and preserved eight-item
  Unicode-named collection. These observations include regenerated bytecode and
  differing filesystem cache state, so they do not isolate causality or certify
  cold startup. Thirty-second main-process idle samples record 1.37% CPU and
  stable 28,584 KiB RSS before reinstall, versus 1.40% and stable 27,724 KiB
  afterward. Descendants and interpreter startup are excluded; endpoint observer
  costs are 0.030% and 0.027%. Both launches exit normally. The separately
  authorized Apps publication now enables all eight public entries and publishes
  Music 0.1.1, Monitor 0.4.1, Firefly 0.3.2 and Carousel 0.4.4. Their integration
  with this Shell candidate still requires native managed-package retesting.

- A normal App Center refresh retrieves all eight enabled public entries. The
  private device catalog is byte-identical to `apps.json` at public Apps main
  `2cdec69dd12843eca05e77c9367b0adeeb92ca58`. Carousel's managed 0.4.3→0.4.4
  update succeeds; its receipt and all 29 files match the pinned published source
  `21774cfa5ce0670f2b2884d0060f238cd6af0b4d`, and the current launcher remains
  unchanged. Warm focus takes 6.59 seconds, followed by a usable native home.
  Actual collection playback reports Mali400 composition and software VP8/VP9
  decoding. Two full video captures differ by 30,332 pixels for the verified
  moving VP8 input and 32,986 pixels for the verified moving VP9 input. Both
  image pairs are manually inspected without a controls overlay. Keyboard pause,
  previous/next and Back, settings editing/save and normal exit are exercised.
  The repeat count was temporarily raised from three to twenty for inspection,
  then restored through the UI; original settings bytes match their baseline.
  Initial identical VP9 captures and subsequent captures of other/Home surfaces
  fail verification and are retained. The successful VP9 capture uses separate
  pointer positioning, press and release calls to activate the collection and
  a timed image pair, checked against its live decoder input. The requested
  537 ms interval takes 1.627 seconds under load; this is motion evidence,
  not an FPS or first-frame latency measurement. Both Carousel process identities
  and all identified decoders disappear after normal exit. Final inventory
  preserves all 36 saved/config files, 26 directory modes and seven unrelated
  receipts. Cold startup, physical motion/tearing/input acceptance and the other
  newly published app fixes remain open.

- Music 0.1.0 was removed through App Center and current public 0.1.1 installed
  from `ad32a6755ced427e6206c6e2b13b88cb891799da`. All eleven published files,
  receipt hashes and current launcher flags verify. The exact source and tests
  were independently staged into private normal-user NAND scratch directories;
  all eleven production files match the public inventory. All 26 native tests
  pass without skips in 100.658 seconds, including both Linux parent-death tests,
  the empty-command-line startup-pause guard, real format decoding, GUI, metadata
  and storage checks. These tests use dummy or null audio where specified; they
  do not establish audible output. Quiet managed warm focus takes 5.28 seconds,
  followed by a usable five-track library. This measures window focus/title,
  not first usable frame or cold startup.
  Actual managed FLAC, MP3, OGG and WAV selections show their correct metadata
  and owned FFplay inputs; each verified fixture opens the device's ALSA playback
  node. Corrupt media shows a readable error. Keyboard pause/resume, forward and
  backward seek, volume 65→60→65, playlist advance and Home/return are exercised.
  Returning to Shell preserves the paused OGG decoder and reopening Music focuses
  the same app and decoder identities. Some earlier filenames say MP3, but the
  captured process/input and screen prove automatic playlist advance had reached
  OGG before that Home check. Original repeat/volume/selected-track preferences
  are restored through the UI. Normal Exit leaves none of thirteen identified
  app/probe/decoder identities or matching Music processes. All 36 saved/config
  files, 26 directory modes and seven unrelated receipts remain unchanged after
  removal, installation, native tests and managed playback.
  A rapid Next sequence shows Working and a media error; later independent probes
  and individual format openings pass, but the earlier error's cause remains
  unresolved and the rapid-switch gate is not cleared. Two readiness observations
  fail on disappearing or temporarily inaccessible child FDs during replacement;
  the private observer now tolerates these transitions while checking start ticks.
  These failed observations are retained separately. Audible physical output,
  remaining input acceptance and cold startup remain open.

- Current Monitor 0.4.1 integration first exercises refusal of an obsolete
  pre-release launcher. The update reports that the launcher differs; complete
  payload/integration, saved/config and unrelated receipt inventories remain
  unchanged. The old generated cache-prefix launcher is retained as evidence.
  Normal App Center removal/reinstallation then removes all owned files and
  verifies all eighteen files at public source `21774cfa`, a current launcher
  and unchanged saved data. This is recreation, not a successful 0.4.0→0.4.1
  update. Native execution of the exact published tests reaches the real EGL
  check successfully, but fails a requested 640-pixel resize under Awesome's
  480-pixel tiling rule and later aborts with `Tcl_AsyncDelete: async handler
  deleted by the wrong thread`. Both failures are retained.
  An isolated Apps fix removes the dashboard/Tk reference from the background
  metrics worker, passing only the collector, result queue and acceptance event.
  A deterministic delayed-collection regression proves the closed dashboard is
  retained by the published worker, then released by the corrected worker while
  collection remains active. Its old-code run fails and corrected run passes.
  The resize fixture uses an unmanaged Tk window; all original geometry and text
  assertions remain. All 77 corrected native tests pass without exclusions in
  47.085 seconds, including real EGL and Linux FIFO checks; the Mac GUI suite
  runs 77 tests with two existing platform exclusions. The first host Linux GUI
  command cannot run because its container lacks `xvfb-run`; this is retained as
  an environment limitation, not counted as a test pass. All 25 tested source/test
  files match the isolated Apps worktree, and all 36 saved/config files, 26
  directory modes and seven unrelated receipts remain unchanged. The device
  still has published Monitor 0.4.1. The owner authorized correction release
  0.4.2; preparation/publication and managed correction/Pulse/GPU-accuracy checks
  are deferred to the Apps follow-up.

- Firefly's generated launcher was moved into a private verified backup to
  inject a missing-launcher fault without changing payload or saved data. Actual
  App Center Update recovered the launcher and installed current public 0.3.2;
  all 18 published package files, receipt and current launch flags verify.
  Managed launch reached a focused 480×272 window, and normal Escape exit was
  reaped by Shell with no remaining identified app process. All 36 saved/config
  files, 26 directory modes and seven unrelated receipts remained unchanged.
  The separate Firefly frame-rate/CPU evidence is retained for the Apps audit.

## Final production startup and soak

Three recorded software reboots of bundle `ea6fc653…` reach the automatic native
launcher, retain one PID/start-time/executable identity during each observation,
and keep the focused fullscreen 480×272 window at 0, 15 and 30 seconds. No
service-owned zombies appear in those observations. The boot IDs are
`6cf3cbc2-9468-46fc-b8e9-882618a7f9de`,
`2c34efb0-bb6d-474a-be08-f1668f14e526` and
`3ac1fab8-f720-4e9c-a0c4-3c496116959e`. All five executable hashes, lengths,
ownership and modes still match the reviewed bundle; saved/config bytes and
modes, Firefly payload/integration and unrelated receipts match the baseline.
These are software reboots, not physical cold-power acceptance. SSH reachability
and readiness observations include OS/USB transport time and are not Shell-only
startup latency.

A malformed initial sudo argument never requests a reboot and is retained as a
harness failure. Another reboot observation stops at a check comparing complete
process rows, including changing scheduler state; its exact failed row was not
recorded, so the immediate cause is not established. The corrected observer
records each row and compares PID/start-time/executable while preserving its
focus and no-zombie assertions. Two additional real reboots then pass, producing
the three complete records above. No production change or exclusion is involved.

The final activity soak completes in 1800.09 seconds with 61 samples on the last
boot. The first-to-last counter interval is 1799.93 seconds; sampler setup/final
sync accounts for the remaining interval. Boot, generation and Shell identity
remain unchanged, with no service-owned zombies. Main-process CPU averages 1.02%
of one core. RSS rises from 36,872 KiB as screens first load to 38,492 KiB, then
stays exactly 38,492 KiB in every sample of the final twelve minutes. Minimum
available memory is 259,380 KiB. The sampler itself uses 0.265% of one core;
interpreter startup, SSH, UI automation and other observers are excluded from
that observer figure. Session log size grows from 17,495 to 49,561 bytes within
its bound. The current-boot log has no panic/fatal/error, hardware fallback,
compositor exit or present failure. Ten discovery warnings identify the same
stock Get Help entry whose `surf` command is absent; that existing external-app
limitation remains visible and is not hidden. Terminal executes `pwd` as chip
and exits with status zero; its test session unsets HISTFILE before exit.
Notepad's clean document and close behavior pass. Files' Tab/Left selection
visibly reaches Close and normal activation returns to Shell. An earlier
synthetic F6 sequence did not select the
footer and is retained; no physical F6 acceptance is inferred. Subsequent native
utility cycles require matching first-painted controls before sending input.
The synthetic Power sequence also reaches App Center rather than the intended
Settings page; those screenshots are excluded as Settings evidence. Actual
Settings opens correctly through its launcher tile. An incomplete utility cycle
then checks only the shared window title while App Center remains open; a later
harness rejects a valid launcher because selection changes its text colour.
Both are retained harness failures. Subsequent utility cycles validate the
painted launcher glyphs independently of selection colour before acting. Two
complete utility cycles record Terminal, Notepad and Files at 480×272, each
returning to the same Shell PID. App Center Details also opens normally and
Escape returns through the list to the launcher.
Actual Date & Time changes 12→24→12 by keyboard, with restored preferences
byte-identical and private chip-owned mode 0600. App Center finishes loading all
eight public entries and displays their installed versions; transient
loading/cancellation captures are retained as such, rather than mistaken for
completed catalog screens.
Final inventory again matches all saved/config bytes and modes, Firefly payload/
integration and unrelated receipts. All nine identified utility PIDs are gone.
The owner reported a normal physical shutdown and power-on, then a cold-boot
keyboard failure with touch still working. The failure and replacement startup
repair are recorded above; the earlier input acceptance does not clear this gate.
Read-only SSH confirms the new boot `ec128189-6679-45c6-9411-8490119e4101`,
automatic native ready at 480×272, one Shell process and no service-owned zombies.
All five executable hashes/sizes/ownership/modes and saved/config bytes/modes,
Firefly payload/integration and unrelated receipts remain unchanged. The new
boot log confirms accelerated Mali400 without fallback and has no panic/error,
compositor exit or present failure; its sole warning is the existing missing
`surf` entry. The first log matcher incorrectly classified `fallback=false` as
a failure; its retained result is corrected to check `fallback=true`. These
software observations do not establish keyboard delivery or physical display acceptance.

## Published certification prerelease

The owner authorized merging PR #5 and publishing source
`a57b107830b802372afe72fcb584386f1c3f6abd` as a certification prerelease after
its tagged workflow and exact device checks passed. PR #5 merged at
`246e99056a4159e76c1197ee788230d5c82a16d4`; annotated tag `v1.0.0` points to
that reviewed source. [Tagged workflow 37178822016](https://github.com/csd113/Vitrallis-Shell/actions/runs/37178822016)
passes canonical validation, both release builds, ARMv7 probes and packaging.
[The certification prerelease](https://github.com/csd113/Vitrallis-Shell/releases/tag/v1.0.0)
was published on 2026-10-04 at 05:45:30 UTC. This is not a stable release or a
release-readiness declaration.

The tagged workflow rebuilt both native bundles; those bytes supersede the
prepared bundles recorded above. ARMv7 SHA-256 is
`5ad2944b47f3152032c87ab87e2f4bb3466bc2bd841815a4176fdecd6938b615` and
x86-64 SHA-256 is
`0f7d214d265b47cf6408c329412663cd5fcc5d2a799dd2353fb5c64bbbd3e878`.
All 19 freshly downloaded artifacts have unique names, eight matching checksum
sidecars, matching committed helper/legal files, and matching GitHub sizes and
SHA-256 digests. The physical PocketCHIP anonymously reads the published release
metadata and verifies that same inventory. `tagged-release-artifacts-manifest.json`,
`tagged-release-workflow-proof.json`, `tagged-release-published-api.json` and
`public-device-release-api.json` retain these checks.

All five tagged ARM executables pass actual device version, length, SHA-256,
chip-owned mode 0755 checks, including the rebuilt Arti 2.6.0. The Shell graphics
self-test and the three native utility hardware smokes/readbacks pass on Mali400
with VSync and no fallback. Strict software/hardware pixel equality remains
FAILED: 73 Terminal, 47 Notepad and 93 Files pixels differ, each by at most one
channel level. Hardware and automatic output match exactly. The native images
were inspected; production rendering and repository assertions remain unchanged.
Evidence is in `tagged-native-executables-graphics.log`,
`tagged-native-utility-graphics.log` and `tagged-native-pixel-comparison.json`.

The exact tagged bundle passes software reboot
`3c2b5ab2-19e1-4986-a37c-2a3ffefa5617`, reaching current-session native readiness
in 186.431 seconds. At 0/15/30 seconds after readiness, actual X11 input focus
matches Shell PID 1077, the same 480×272 fullscreen window remains focused,
PocketHome has no process/window and no service-owned zombie exists. The owner
physical cold-start acceptance above used this same repaired startup integration
with the prior native bundle; it is not represented as a physical cold-start
observation of the rebuilt binaries. See `tagged-native-reboot.json`.

Brightness 100→90%, volume 90→80% (amixer 57→50), 12→24-hour clock,
America/Vancouver→America/Whitehorse, background timeout Never→five minutes,
Notepad keep-running Off→On, screen timeout 30→10 minutes and Tor
On-demand→Disabled all survive leaving/re-entering Settings, Shell restart,
tagged-bundle installation and device reboot. Backlight and volume use the
existing OS restoration services. Original values are restored through the UI;
configuration bytes, private chip-owned modes, hardware readbacks and timezone
match the saved baseline. All 93 tracked data/config/app/integration entries
remain unchanged. `settings-persistence-*.json`,
`settings-restoration-v2-*.png` and `retained-state-restored-after-tagged-reboot.json`
retain the observations.

An unguarded restoration sequence reaches unrelated launcher tiles; its images
are excluded as Settings evidence. The corrected sequence checks each painted
page before acting. An immediate timezone read precedes asynchronous completion,
and a later readiness assertion inspects only the last 4096 log bytes after
activity has pushed the ready line out of that tail. Both retained harness
failures are corrected with observed completion and the complete current-session
log segment; actual timezone save/return and exact restored values pass.

The public `main` bootstrap now matches the repaired helpers and no longer passes
the removed `--make-default` flag. The PocketCHIP anonymously fetches the public
README; its exact command SHA-256 is
`cd8e4d9be5f8ad70c7aee90ac056cc76d1569f379e957f01d4a78659f11e48ee`.
Normal offline uninstall removes all three retained generations, core helpers,
launch shortcut and managed startup, restoring the original Awesome file hash
`507e520527681703f3747831d8c428bbdf49c12890ffe78ba45fc3960920200e`.
All 93 tracked retained entries still match. Retained runtime state is temporarily
isolated in the private certification directory to make the complete core root
absent before the public installer, while saved preferences, Apps and AppData
remain in place. The original PocketHome desktop is started for this clean test.
The exact README command succeeds from that absent runtime root and selects
the public `v1.0.0` assets. Repeating it also succeeds with the same native
hashes, matching source helpers, unchanged configuration and a single startup
block. Both runs pass five native version probes and private chip-owned runtime
directory checks; all 90 tracked entries outside the temporarily isolated App
Center integration remain unchanged. The public command's umask 077 creates
user-owned runtime/generation directories as 0700, rather than the 0755 assumed
by the initial private inspector; corrected checks use the actual secure
public-command contract. The unrelated root-owned platform status permissions
remain separately checked by provisioning. Retained App Center and Tor state
are restored after these install checks, without replacing any public core
artifact. `public-clean-readme-install.log`, `public-repeat-readme-install.log`,
`public-install-first-v2.json`, `public-install-repeat.json` and
`public-retained-state-restore-proof.json` retain the evidence.

The fresh install has no previous generation. Repeating the published installer
creates a previous pointer equal to current; the distinct retained-generation
case fails the regression recorded below.

The publicly installed native bundle passes software reboot
`7b88c948-e00a-4585-8b99-dafb39158dd2`, reaching current-session readiness in
185.085 seconds including OS/USB/SSH time. Shell PID 1072 retains actual X11
focus and its fullscreen 480×272 window at 0, 15 and 30 seconds. PocketHome has
no process/window and no service-owned zombie appears. All 93 tracked retained
entries and original eight settings still match. The first observer fails before
requesting a reboot because the fresh installation has not yet created its first
session log. The corrected observer reads the pre-reboot boot ID directly;
post-reboot readiness still requires the new session log and actual native
window. Both records are retained.

On that exact public installation, Terminal accepts `exit` and its keyboard
Close returns to Shell. Notepad accepts a short note, defaults to Cancel on dirty
exit, retains the text after Cancel, then discards only on selected Discard.
Files opens the real home directory; Tab/Left visibly selects Close and Enter
returns normally. All three utilities exit, leaving the original Shell PID,
three supervised processes, no zombies/PocketHome and actual X11 focus on the
launcher. All 93 tracked retained entries remain unchanged. This clears the
public native-utility smoke, not the complete final device smoke or official
update/interruption gates. Evidence: `public-installed-reboot.json`,
`public-installed-reboot-v2.log`, `settings-persistence-after-public-reboot.json`,
`public-installed-native-smoke-state.json`, `public-installed-*.png` and
`retained-state-after-public-native-smoke.json`.

## Reinstall rollback-pointer regression

The published installer sets `previous` to the active generation even when
reinstalling that same bundle. After A→B→reinstall B, both pointers select B,
so the normal previous-generation recovery choice A is lost although its files
remain. This is a release blocker. A new regression installs two distinct valid
current-format bundles and checks that reinstalling B preserves A. It fails on
the published source and passes after a focused condition skips the previous
pointer write when old and new current pointers are equal. No version, native
rendering, package format or compatibility path changes are included.

All 42 installer tests pass with the correction. Full `sh scripts/validate.sh`
passes on macOS and native Linux AArch64, including formatting, strict Clippy,
workspace tests, Python tests, release builds, native smokes, syntax and 59-file
Markdown link validation. Mac records 378 passing Rust tests with nine explicit
opt-in/platform exclusions and 178 Python cases with nine exclusions. Linux
records 380 passing Rust tests with twelve explicit opt-ins and 178 Python cases
with eight exclusions; the separate renderer suite passes its software case and
retains the accelerated opt-in exclusion on both hosts. Device graphics evidence
above remains separately qualified.

Both opt-in real Awesome/X11 fixtures also pass separately in the disposable
Linux image, verifying one Vitrallis startup without PocketHome and the visible
window's real PID/parent. An initial invocation omits the documented
`dbus-run-session` entry point and fails desktop availability. Repeating with
that actual session bus passes both unchanged tests. Evidence:
`reinstall-previous-desktop-fixtures-linux.log` and its `-v2.log` correction.

The first Linux container mount hides its Cargo executable and stops before
validation. The second copy omits `.git`; Cargo then includes generated Python
caches, correctly failing the archive-content assertion. A complete read-only
checkout copied to a disposable workspace passes the original assertion and
full canonical sequence. These harness failures and the original failing
regression remain retained in `reinstall-previous-*.log`. The local correction
is not part of the immutable published 1.0.0 assets. After the concrete correction
passed host and device tests, the owner approved a fresh release on 2026-10-04 UTC.
That authorization is applied to Shell 1.0.1 for this correction, including
review/merge and conditional certification prerelease publication after CI and
exact tagged device gates. It does not authorize later version bumps or a stable
readiness declaration. Full Mac/Linux canonical validation passes again on the
1.0.1 versioned source, with unchanged test counts and all four native version
probes reporting 1.0.1. The lockfile changes only the five workspace package
versions; dependencies are unchanged. Evidence:
`recovery-v1.0.1-canonical-macos.log`, `recovery-v1.0.1-canonical-linux.log`,
`recovery-v1.0.1-canonical-summary.json` and
`reinstall-previous-owner-release-authorization.json`. Exact CI, tagged artifacts
and renewed public installation remain required.

The real PocketCHIP reproduces the published defect using genuine five-member
ARM bundles from source `a57b107`: public rebuild `5ad2944b…` (A) and the
earlier healthy prepared rebuild `07f299b4…` (B), both reporting 1.0.0. Actual
published-helper A→B installation retains A; reinstalling B replaces previous
with B and fails distinct-generation recovery. The corrected helper then
activates A, activates B and reinstalls B; previous remains A. These are actual
normal-user installer executions with native version probes and authenticated
platform provisioning. All 93 tracked data/config/app/integration entries match
after every phase. This is prepared regression verification using the published
helper bytes, not an official public OTA update or a new clean public install.
The correction is committed separately as `305107d`; its installer hash is
`256ea3ee636bef22b8a255a41a0968f1b53cdab7e1e2b97fb596b98846756213`.
The public prerelease notes now disclose the defect; all 19 asset IDs, sizes and
digests remain unchanged.

The final sixth installer execution restores public native bundle A while
retaining B as previous. The corrected installer helper remains installed as a
prepared correction, so this final state is explicitly different from a wholly
published installation. The session relaunch reaches Ready with Shell PID 10068,
the same boot, actual X11 focus on the fullscreen 480×272 launcher, no PocketHome
and no service-owned zombies. An initial observer expects exactly three owned
processes during a transient `bluetoothctl` query and fails; that specific query
PID is independently verified gone, and the later state has exactly the three
normal session processes. All 93 tracked retained entries still match.
`reinstall-previous-device-proof.json`, the six phase logs,
`reinstall-previous-device-relaunched-during-radio-query.json`,
`reinstall-previous-device-relaunched-state.json` and
`retained-state-after-reinstall-previous-device.json` retain the proof.

## Historical gate ledger before final 1.0.3

This snapshot is superseded by the final-public-candidate section and current
[readiness report](release-readiness-2026-10-02.md).

| Gate | Current result and remaining work |
| --- | --- |
| Repository/device baseline | Passed baseline inventory; complete component review continues |
| App filesystem contract | Implemented and host-tested; complete physical app lifecycle pending |
| Filesystem/permissions | Fresh root directory modes repaired and normal-user runtime integrity passed; app and physical fault matrix pending |
| Complete clean public installation | Fully clean public 1.0.1 README installation, completed repeat, all public core hashes/modes, saved state and reboot pass. Earlier Beta2 provisioning and 1.0.0 reinstall failures remain retained and corrected. Final candidate smoke remains open. |
| Installer failure cases | Canonical fixtures pass. Prepared bootstrap download cancellation and real installer SIGINT after current publication pass on device, with rollback, readable exit 130 and successful retry. Remaining physical faults and exact public-candidate checks stay open. |
| Uninstall/reinstall with real app data | Initial removal/reinstall passed; complete persistence sequence pending |
| 480×272 UI | Core launcher, Settings, native utilities and App Center list/Details/transient states inspected on final production; final public-path smoke remains pending. |
| Keyboard/touch | Original cold-boot navigation FAILED with actual X11 focus on hidden PocketHome. Replacement startup passes two reboot focus checks, visible synthetic launcher/Notepad/Settings/Home input and renewed owner physical cold-boot keyboard/touch acceptance. Final public-path smoke remains pending. |
| App Center lifecycle/data preservation | Eight-app lifecycle evidence is retained; current Shell additionally passes Carousel removal/reprovision/public update, Music removal/install/launch/cleanup, Monitor refusal/remove/install and Firefly missing-launcher recovery/public update with saved data unchanged. App internals are deferred to Apps. |
| Real data persistence | Places prepared update, Shell replacement, reboot, offline uninstall and online reinstall retain private state; complete ecosystem and Shell reinstall sequence pending |
| Python runtime | Native Carousel removal/reprovision reclaims derived runtime backups, preserves AppData and generates the current launcher; current Music, Monitor and Firefly generated launchers verify against public packages. Additional app performance work is deferred. |
| Process lifecycle stress | Public 1.0.1 Restore/relaunch exposes a persistent owned `ip` zombie. The prepared correction passes host regressions and a device held-query trial starting from normal production source, with exec deferred until exact child reaping, focused readiness and saved state unchanged. Exact public-candidate verification remains open. |
| Repeated startup | Original physical cold boot FAILED keyboard navigation. Replacement startup passes prepared software reboots and renewed owner physical cold boot. The exact tagged rebuild and its clean public installation each pass a software reboot with current-session readiness, no PocketHome and sustained actual X11 focus. Owner cold-power acceptance of rebuilt bytes is not inferred. |
| Hardware features | Display/GPU backend and radio readback pass; brightness/volume actual readback passes; audible audio, battery/power and remaining acceptance incomplete |
| Every setting persistence | Eight original controls pass the controlled public 1.0.1 reboot, utility smoke and official update/rollback round trip. The clean sequence with an intervening stock desktop resets brightness to 1/10; the retained failure is qualified separately, and the controlled reboot saves/restores 10/10. Prior 1.0.0 configuration-fault cases pass. |
| Offline/network failures | Physical Wi-Fi off/on, cached App Center, offline uninstall and refresh recovery passed; remaining fault matrix pending |
| Low NAND / ENOSPC | Actual UBIFS 64 MiB pressure passed bootstrap preflight rejection, config save and 64 KiB Notepad save; Carousel install completed near 113 MiB free; isolated NAND-backed ext2 ENOSPC/retry passed and all fixtures cleaned; actual public Shell update passes at 112 MiB initial/81 MiB minimum free with an existing verified target; fixture cleanup passes; new-generation low-space extraction and physical power-loss durability are not inferred |
| Shell update/rollback | Genuine published beta baseline with current helpers passes real public 1.0.1 update, interrupted download, checksum rejection, partial extraction interruption, recovery, public retry and actual Settings Restore/relaunch in both directions. Saved data/settings match. Actual UBIFS activation-window interruption, normal startup and public retry pass with the stated beta-source qualifications. Low-space public update passes with the existing-generation qualification. The authorized 1.0.2 corrections still require exact tagged/public-candidate checks. |
| Security/trust boundaries | Concrete path guards repaired; complete audit/fault matrix pending |
| Failure UX | Safe uninstall/trust confirmations, corrected offline refresh/Details and cause-first cancelled update inspected; full operation error retained in private log; other failures pending |
| Files/Terminal/Notepad | Real note save/read, direct editor, footer wraparound, long-note save, Find and dirty Cancel/Discard passed on prepared builds. The exact public installation additionally passes all three utility launch/keyboard/normal-exit smokes with no remaining utility process and all 93 tracked entries unchanged. |
| Shell performance | Final 1800.09-second activity soak averages 1.02% main-process CPU; RSS stabilizes at 38,492 KiB for the last twelve minutes. Earlier short idle/calibration evidence is retained; app-internal performance is deferred. |
| Extended soak | Final-production 1800.09-second activity soak passes all 61 identity/boot/generation/zombie samples, with restored preferences and unchanged saved data. Earlier revision-specific runs and failed harness observations remain retained. |
| Logs | Final current-boot review has no panic/fatal/error, renderer fallback, compositor exit or present failure. Ten repeated warnings concern the stock Get Help entry lacking surf. |
| Code/documentation hygiene | Storage/provenance docs updated; final sweep pending |
| Public owner documentation | Exact clean public 1.0.1 README command and completed repeat pass; published/tagged helpers and artifacts match. Certification terminology and current evidence are recorded; final fault/hardware qualifications continue. |
| License/repository consistency | Artwork, both Arti graphs and Rust notices are reconciled; all 19 tagged/published asset sizes/digests and exact legal/helper bytes verify, including anonymous release metadata and exact public installation on the device. |
| Canonical release builds | Prepared source e905401 passes complete Mac/Linux validation (380/382 Rust tests, 181 Python cases), ARMv7 packaging and six exact-source push/review CI jobs. Earlier exact tagged 1.0.1 builds/device smokes pass. Strict hardware/software screenshot equality fails only by the recorded one-channel-level colour difference; assertions remain unchanged. Final public-candidate checks remain open. |
| Exact clean candidate | Public candidate 1.0.1 is tagged source b2ff6d4 with six passing exact-source review jobs and successful tagged release workflow. All 19 assets and installed five-member ARM bundle verify. Prepared relaunch/cancellation corrections need a fresh authorized exact tagged/public candidate and final physical acceptance. |
| Final physical smoke | Not run |

## Validation recorded so far

Shell `sh scripts/validate.sh` passes on macOS ARM64 and native Linux ARM64 with
the approved 1.0.0 version and reviewed references: formatting, locked
all-target/all-feature check, strict Clippy, workspace tests, Python tests,
release binaries, SDL/native smokes and doc links. The Settings utility-return
runs include the editor/importer, private-root-umask, real-Lua focus and offline
diagnostic regressions, two radio-result regressions and two Settings utility
regressions:
368 Rust tests on macOS, 369 on Linux, and 173 Python tests
(nine existing skips on macOS, eight on Linux), plus the native
renderer suite (two tests, one existing skip).
An earlier macOS run hit a local Tor relay half-close socket timeout. The failure
is retained; the targeted Tor suite and a complete unchanged-code retry passed.
Its transient cause is not established. The final utility-return macOS/Linux
gates both passed independently.
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
files and retained the private marker; private application-created settings/cache
and the keyboard change also pass. Reboot/relaunch and offline uninstall/online
reinstall persistence pass, followed by successful post-reinstall launch,
Home/resume and normal Exit with unchanged persistent data hashes.

The startup hook's two Lua regression scenarios also passed inside the physical
Awesome Lua 5.3 runtime, in an isolated mock environment. The device Python session
suite passed 12 tests with two standalone-Lua skips; those two scenarios were
therefore exercised separately inside Awesome rather than silently omitted.
An Arti QEMU probe initially crashed when the harness mixed the cross-toolchain
loader with the multiarch runtime libraries. With the matching system loader/libc
prefix, all five ARMv7 version probes and exact `c17280d`, `a4a3e8f` and
`fbf2ec7`, `9f6f0d0` and `bce0d86` packaging passed. The
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
