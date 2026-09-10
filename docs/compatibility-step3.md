> Historical Step 3 record. Current candidate changes and validation are in [release-candidate.md](release-candidate.md).

# PocketCHIP compatibility and store integration

Work began 2026-09-10. This pass supersedes the earlier restriction on device access. The physical PocketCHIP is connected over USB Ethernet (`192.168.81.1`) and USB serial (`ttyGS0`). It is Debian 13, ARMv7 hard float, Linux 6.12.94, SDL 2.32.4, Awesome 4.x, 480×272. The LAN address was confirmed to have the same SSH host key as USB. Password SSH is disabled; initial access used the supplied login over USB serial. A temporary USB-source-restricted SSH key was added after preserving `authorized_keys`; the exact temporary key was removed during final cleanup, preserving all other authorized-key bytes.

## Evidence and changes

The Step 2 matrix remains the starting point, with system features described in [the system audit](devices/pocketchip/system-status.md). Source reference: Marshmallow `dccbd38dc233f3f45ebd6ea130d6a787268e23f5`. Current store reference: `csd113/Pocketchip-update-apps` at `1f394452d6acd124d940154234b0eb8dd7150b70`. Installed updater source and deployment helper matched that commit byte-for-byte before patching. The installed Bitcoin source matched current upstream before any changes.

- Imported the device's five existing applications in order, without conversion: Bitcoin CAD, Update Apps, Terminal, Write, Browse Files.
- Built ARM on macOS using cargo-zigbuild and an SDL shared library copied read-only from the device. All loader dependencies resolved on the device. No OS packages or runtime libraries were installed/replaced.
- Ran Vitrallis within the existing Awesome session. X capture confirmed 480×272 display with correct icons. A framebuffer read showed the inactive console, so screenshots used the actual X display instead; console framebuffer content is not evidence of the visible launcher.
- The user physically verified arrows, Return, Bitcoin launch/Escape return, tapping Update Apps/Home return and readable layout. These are user-observed physical keyboard/touch results, not inferred from injected events.
- Added Store alias/discovery, post-app catalogue refresh with identity-preserving selection, incomplete-install launch rejection and kernel sysfs battery support. Kernel battery data does not fall back to forced I²C on invalid values.
- Installed a separate per-user Vitrallis launch target and desktop shortcut. Only an additional Vitrallis item was added to the PocketHome menu; Awesome configuration, Marshmallow executable/wrapper, greetd and boot configuration were untouched.
- The user systemd manager supervises the launcher and its descendants, with no restart loop. Temporary Awesome Home routing is restored by `ExecStopPost`, including when the Python supervisor is SIGKILLed. Hardware test observed the unit become inactive, no surviving Vitrallis process, restored key table and focused `pocket-home` after supervisor SIGKILL. The immediate `stop-post` snapshot in the evidence file was followed by an `inactive/dead`, keys-restored snapshot; it is not the final state by itself.
- Session stdout/stderr drains to two logs of at most 128 KiB each. Journald only receives supervisor/service diagnostics. No PID file or root operation is needed.

## Store lifecycle on the physical device

The existing updater remains the Store UI and explicit catalogue authority. The patch retains repository/source/branch conventions and GitHub commit/blob verification. It never executes installer fragments from metadata. Compatibility fixes are delivered as a reviewable upstream diff and exact before/after hash manifest in `devices/pocketchip/integration/`.

The original Bitcoin directory was preserved by rename to `/home/chip/.local/share/pocket-bitcoin.before-vitrallis-validation`; its source hash matched upstream. Through the Store opened from Vitrallis, injected X keyboard/pointer actions checked GitHub, observed “not installed” and latest v1.1.0, selected Bitcoin and installed it. The result UI reported one installed app and v1.1.0. The new app directory contains source, icon and an executable launch wrapper. Source SHA256: `87902ae5671fea88af84e1e3ed56214f2b2c3dab9c97153eb38e1ed8273f749e`.

Store closed successfully, Vitrallis reloaded the menu, a direct tap launched the newly installed Bitcoin CAD v1.1.0, X capture showed its live application UI, and Escape closed it with exit code zero and restored focus to Vitrallis. A rapid Escape/Left/Return sequence initially reopened Store because icon reload consumed the navigation event; unchanged catalogues now retain icon textures. Subsequent device launch/return and multiple-app tests exercised the retained-texture path.

The patched store's 44 tests passed on the physical PocketCHIP in 41.637 seconds, including real abrupt installer exit followed by repair, malformed data/checksums, rollback, missing launcher, symlinks, update behavior and original self-update regression tests. Application-self-update UI is intentionally manual under the local compatibility patch so a remote self-update cannot silently discard the safety fix. Bitcoin updates remain available. The store has no safe uninstall workflow; no uninstall implementation is claimed.

Evidence snapshots live in `docs/evidence/step3/`. Physical touch was verified by the user earlier; the fresh installation used injected X input on the real hardware and is labeled accordingly.

## Validation scope

Hardware results, host regressions and deliberate compatibility limits are distinguished below. PASS applies to the stated behavior and evidence; PARTIAL identifies remaining differences rather than claiming exact Marshmallow parity.

### Later verified results

- The user reported that physical Home followed by tapping Bitcoin initially required two taps. SDL's default discards activation clicks; enabling `SDL_MOUSE_FOCUS_CLICKTHROUGH` preserved the first press. The user then confirmed **one-tap resume works**, and confirmed the second-page Marshmallow tile returns to the original home. The matching press/release and duplicate filtering logic remain intact.
- Tk windows on this image do not publish `_NET_WM_PID`. Resume first matches the child's private process group; only the inspected Bitcoin/Store launch paths use a closed Tk class/title fallback for focus. This fallback never selects processes for termination or confers ownership. No Bitcoin source changes were required.
- Five repeated supervised starts/stops passed: launch-to-focus measurements 8.81, 8.79, 9.78, 8.72 and 7.93 seconds. Each stop left the unit inactive and Marshmallow focused; original binary/wrapper/Awesome config hashes still matched. RSS snapshots varied during startup (about 9.7–38.6 MiB), so these are not steady-state memory/CPU measurements.
- Terminal, Write, Browse Files and the configured `lxterminal -e nmtui` Wi-Fi settings path all created new windows and returned to Vitrallis after those exact test windows were closed. Existing user files and network configuration were untouched. GTK emitted existing theme deprecation warnings; they did not prevent launch/return.
- SIGKILL of only the Rust launcher while its Bitcoin child was running removed both via the supervisor's unit and restored the original Home binding/focus. SIGKILL of the Python supervisor was tested separately with the same recovery result.
- A 0400 session log caused startup to fail closed, stop the unit and restore Marshmallow. Permissions were restored in `finally`. The unprivileged journal-reading step was denied, so that test command's nonzero exit is a log-access limitation rather than a failed recovery assertion; subsequent `stat` verified restoration.
- Kernel `CanReboot` and `CanPowerOff` both returned `yes` for the user. The later reboot and user-confirmed shutdown/power-on tests are recorded below.


## Differential matrix (current evidence)

PASS describes the stated behavior and evidence level, not exact visual parity. Historical Step 1/2 exclusions do not apply to this hardware pass.

| Feature | Status | Evidence and deliberate differences |
| --- | --- | --- |
| Normal session startup | PASS | Separate transient user service on the physical panel; original Awesome/greetd startup preserved. |
| Repeated restart | PASS | Five real-device start/stop cycles; original home regained each time. |
| App grid/order/pages | PASS | Existing five app entries plus configured Wi-Fi and fallback tile; host boundary tests and user-confirmed second page/fallback. |
| Keyboard/touch/layout | PASS | User confirmed physical arrows, Return, Escape, tapping Store, readable panel and one-tap resume. Injected X tests are separately identified. |
| Selection/focus | PASS | Selection retained by identity; physical Home/resume confirmed. Session-scoped Awesome binding restored on exit. |
| Application launch/return | PASS | Fresh Bitcoin, Store, Terminal, Write, Browse Files and nmtui windows observed on hardware. |
| Multiple running applications | PASS | Physical device with injected input: Bitcoin → Home → Store → Home → resume Bitcoin → close → resume Store → close; both original PIDs reaped, no duplicate starts. Test first tapped an incorrect Store Home coordinate, then the supported Home key closed it; preboot log confirms both exits. |
| Child crash/missing executable | PASS | Real `/bin/false` reaped; missing command showed dismissible error; subsequent Store launch/return worked. |
| Launcher/supervisor crash | PASS | Separate SIGKILL tests restored original home/key bindings and removed owned descendants. |
| Malformed/missing entries/config | PASS | Parser fixtures reject malformed fields, preserve visible unavailable entries; startup remains usable, refresh keeps last valid catalogue. Host tests. |
| Icons/wallpaper | PARTIAL | PNG/BMP bounded decode, real icons visible, host wallpaper/color pixel test. SVG/JPEG and Unicode/font parity unsupported. |
| Personalization | PARTIAL | Reads existing background, clock visibility/12-hour mode and cursor settings; editing uses preserved Marshmallow settings. No duplicate preferences writer. |
| Battery/external power/Wi-Fi/clock | PARTIAL | Kernel sysfs and nmcli available on hardware; no forced I²C when kernel driver exists. Full charge/radio transitions were not measured; the final current-state readings and Marshmallow battery-label difference are recorded below. |
| Brightness/volume | PASS | Real UI taps changed backlight 7→6→7 and amplifier raw 47→41→47; original values verified restored. Audible output and physical full-range perception not measured. |
| Power cancel/denial | PASS | Real reboot confirmation cancelled with Escape; boot ID unchanged. Host tests cover default cancel, expiry, focus-loss cancellation and denied fixed command. |
| Reboot/cold start | PASS | Confirmed UI reboot returned to Marshmallow; user separately confirmed full shutdown and physical power-on. New boot ID and actual post-cold-start menu/Bitcoin/return workflow verified. |
| Settings entry points | PASS | F1/touch system controls plus configured nmtui entry; real terminal and nmtui launch/return. |
| Sleep/lock/login/advanced settings | PARTIAL | Preserved Marshmallow remains reachable. Reference sleep couples DPMS with its own lock UI; copying just screen-off would omit authentication semantics. No replacement password storage or fabricated security boundary. |
| FEL flashing mode | NOT SAFELY TESTABLE | Reference invokes recovery/flashing behavior; excluded by device-preservation requirement. Preserved Marshmallow retains its original features. |
| Bluetooth | NOT APPLICABLE | Reference is fixture UI, not a live pairing backend; Vitrallis reports unavailable. |
| Date/timezone administration | PARTIAL | Reference invokes sudo dpkg-reconfigure in a terminal; no additional privileged editor installed. Existing terminal/Marshmallow route retained; display format read. |
| Store catalogue/metadata | PASS | Existing explicit APPS catalogue and pinned GitHub commit/blob workflow; real Check displayed Bitcoin version and installation state. |
| Bitcoin install/entry/launch | PASS | Fresh installation through Store UI on physical device; generated launch/icon/menu, exact upstream source hash, live Bitcoin UI and return. |
| Installed/reinstall/update state | PASS | Real installed state/Check; upstream regression suite and repair marker tests on ARM. |
| Store network failure | PASS | Child-only unreachable proxy on device; visible refusal, install disabled, clean Home/child exit. Device network configuration unchanged. |
| Malformed/incomplete packages | PASS | 44 upstream tests on physical ARM include checksum/metadata errors, missing files, rollback and abrupt installer exit/repair. Launcher refuses pending install. |
| Store self-update | PARTIAL | Manual under reviewed safety patch to avoid remote replacement silently removing it; Bitcoin update supported. |
| Uninstall | NOT APPLICABLE | Existing Store has no safe removal workflow; no parallel package manager introduced. |
| Read-only storage | PASS | Real non-writable session log failed closed and restored Marshmallow; original permissions restored. Installer rollback fixtures also pass. |
| Full filesystem | NOT SAFELY TESTABLE | Filling shared device storage would risk existing data/services. File-operation failures and rollback tested in isolated fixtures. |
| Bounded logs/relaunch/orphans | PASS | Two bounded 128 KiB logs; no restart loop/PID file; cgroup cleanup on real Rust/supervisor failure. Focus fallback never grants ownership of external processes. |
| Marshmallow/recovery | PASS | Original binary/wrapper/Awesome hashes unchanged, physical fallback tile confirmed, crash recovery focused original home. Reboot returned to focused Marshmallow; Vitrallis/Bitcoin also passed after reboot. |

Final host checks: 41 Rust unit tests, four SDL integration tests, 13 Python installer/session/patch tests passed. Strict Clippy and formatting passed. ARM build succeeded with the existing Zig deprecated-linker-setting warning and no dependency additions or unsafe Rust.


### Reboot evidence

Invoked reboot through Vitrallis's real system panel, selected Confirm explicitly, and observed the remote connection close. Pre-reboot boot ID was `8bd58ade-6c1e-4630-8a51-1da96b68758b`; only Vitrallis and pocket-home windows were present. USB Ethernet and serial did not immediately re-enumerate, so boot success was not claimed at that point. Physical screen observation was requested before attempting any further power action. No startup configuration was changed to run this check.

SSH returned with boot ID `422dbf67-1fde-40d5-b071-768279081983`. All three original binary/wrapper/Awesome hashes still match. At one minute uptime greetd/startx were active but Awesome had not registered its D-Bus name; graphical readiness is checked separately from kernel boot.

At two minutes uptime Awesome reported focused `pocket-home`, confirming the unchanged original graphical session returned normally. Earlier D-Bus readiness errors were transient startup observations, not a persistent boot failure.


Post-reboot Vitrallis launch succeeded, followed by Bitcoin launch → Home → one tap resume → Escape → Vitrallis on the physical device with injected X input. The user subsequently confirmed full shutdown and physical power-on; final results follow.

The final installer audit added preservation of edited desktop shortcuts, original menu file permissions, and pre-mutation rejection of symlinked backup roots. The 13 Python regression tests pass, including these cases. These installer-only changes do not change the running ARM binary.


### Final cold-start validation

The user explicitly confirmed **full shutdown, wait until off, then physical power-on**, with Marshmallow returning. USB reconnected with boot ID `831f3c72-0183-4158-9c6a-72e952b7f3f9`. X capture (`final-marshmallow.png`) showed the original home with all five original applications and the additional Vitrallis tile.

Injected X input then selected the actual Marshmallow Vitrallis tile, launched Bitcoin CAD v1.1.0, closed Bitcoin, and selected Vitrallis's second-page Marshmallow tile. Every expected window was observed. The first cleanup assertion sampled the service after only two seconds and saw `deactivating`; the configured stop timeout is five seconds. A subsequent authoritative check showed `ActiveState=inactive`, `SubState=dead`, no Vitrallis PID, `pocket-home` focused and original Home keys restored. The intermediate test exception is retained in evidence and is not misrepresented as the final state.

Original wrapper, Marshmallow binary and Awesome configuration hashes still match. Installed Vitrallis SHA256 is `3ac58fb2bb475159999175cf4ac9ef7ab8ffe84371104399b2912a265aa78395`; Bitcoin still matches upstream. Brightness remained 7 and amplifier volume 47/63 (75%). The 23,677-byte session log was mode 0600; no rotated file was needed yet. Final installer staging hash matched the local script. Kernel battery reported 95%, not charging, with NetworkManager connected; Marshmallow's capture displayed 100%, so exact battery-label equivalence is not claimed.


Temporary access was revoked by removing exactly the generated key line while preserving all other bytes. A fresh SSH connection using only that key returned `Permission denied (publickey)`, verifying revocation. The generated local private/public key files were then removed. Existing serial recovery, SSH policy and other keys were preserved. The final device was left on Marshmallow with Vitrallis selectable from its menu.

## Validation commands

| Command | Result |
| --- | --- |
| `cargo fmt --all --check` | PASS |
| `cargo clippy --workspace --all-targets --all-features -- -D warnings -D clippy::all -D clippy::pedantic -D clippy::nursery -D clippy::cargo` | PASS |
| `cargo test --workspace --all-features` | PASS: 41 unit + 4 SDL integration tests |
| `python3 -m unittest discover -s tests -p 'test_*.py'` | PASS: 13 installer/session/patch tests after final installer edits |
| Patched upstream updater unittest suite | PASS: 44 on host and physical ARM; device output retained |
| `PKG_CONFIG_ALLOW_CROSS=1 PKG_CONFIG_LIBDIR="$PWD/target/arm-libs/pkgconfig" cargo zigbuild --release --target armv7-unknown-linux-gnueabihf.2.36` | PASS; Zig deprecated linker optimization warning |
| `git diff --check` | PASS |
