# Engineering report

1. **Gaps found:** Store entry/discovery refresh, interrupted-install state, kernel battery support, Home routing, activation-click loss, running-app resume, concurrent child ownership, existing wallpaper/clock/cursor preferences, reversible session installation and bounded supervision logs.
2. **Changes made:** Implemented those paths with validated metadata, shell-free app arguments, known-window focus, private process groups and user systemd cgroup cleanup. Existing device settings and Marshmallow installation preserved.
3. **Store architecture:** Adapted the existing explicit Python APPS catalogue and reviewed repository adapters. No parallel package format/index. Store remains the upstream updater UI; catalogue Check verifies current source at pinned GitHub commits/blobs.
4. **Bitcoin result:** Installed v1.1.0 fresh through Store on the physical device, created/discovered the existing menu entry, displayed its icon, launched the live app and returned. Original installation kept as a backup; upstream Bitcoin source unchanged.
5. **Automated validation:** Formatting and strict Clippy passed. 41 Rust unit tests, four SDL integration tests and 13 Python tests passed. The patched updater's 44 tests passed on host and physical ARM. ARM build succeeded; existing Zig linker-setting deprecation warning remains.
6. **Physical testing:** User confirmed arrows, Return/Escape, readable layout, Store touch/Home, physical Home and one-tap Bitcoin resume, and second-page Marshmallow fallback. Injected input on physical hardware additionally covered fresh Store install, multiple apps/resume, missing command, child/network failure, settings entry points, brightness/volume changes/restoration, reboot cancellation, repeated restarts and launcher/supervisor crash cleanup. Confirmed reboot returned to Marshmallow; Vitrallis and Bitcoin launch/Home/one-tap resume also passed after reboot. User-confirmed full shutdown/power-on and the subsequent Marshmallow tile → Vitrallis → Bitcoin → Vitrallis → Marshmallow workflow passed.
7. **Fallback:** Original Marshmallow wrapper/binary/Awesome hashes matched before reboot. Actual fallback tile and crash recovery returned to the original home. Default boot configuration remains unchanged. Post-reboot/cold-start hashes and focused Marshmallow were verified. Temporary SSH access was removed and a fresh connection rejected; other keys and serial recovery were preserved.
8. **Limits:** Full battery/radio transitions, audible range and long-term performance not measured. SVG/JPEG/full Unicode are unsupported. Advanced login/sleep/timezone administration remains through Marshmallow; FEL and filling shared storage were intentionally avoided. Original Jessie ABI is not established by this Debian 13 test.
9. **Files:** Rust: app, discovery/{mod,marshmallow,store}, launcher, preferences, platform/{mod,pocketchip}, process, renderer, ui, lib; tests/desktop.rs and three Python test modules. Scripts: install-pocketchip.py, vitrallis-session.py, apply-store-patch.py. Integration: reviewed Store patch/hash manifest. Documentation: README, session/store/compatibility report, evidence, and historical-report pointers. .gitignore excludes Python caches. No commits made.
10. **Upstream changes:** Pocketchip-update-apps receives the reviewable local patch for persistent incomplete-install markers, repair detection, conservative manual self-update and regressions. It retains repository conventions and safe verified-source workflow. PocketChip-Bitcoin-Display has no source changes. No upstream commits or pushes.

See [the differential matrix](compatibility-step3.md), [installation/recovery](session.md) and [Store workflow](store.md) for detailed evidence and reproduction.


## Changed files

- `.gitignore`
- `README.md`
- `docs/compatibility-step3.md`
- `docs/device-validation.md`
- `docs/engineering-report.md`
- `docs/session.md`
- `docs/store.md`
- `docs/system-status.md`
- `integration/pocketchip-store.patch`
- `integration/store-patch-manifest.json`
- `scripts/apply-store-patch.py`
- `scripts/install-pocketchip.py`
- `scripts/vitrallis-session.py`
- `src/app.rs`
- `src/discovery/marshmallow.rs`
- `src/discovery/mod.rs`
- `src/discovery/store.rs`
- `src/launcher.rs`
- `src/lib.rs`
- `src/platform/mod.rs`
- `src/platform/pocketchip.rs`
- `src/preferences.rs`
- `src/process.rs`
- `src/renderer.rs`
- `src/ui.rs`
- `tests/desktop.rs`
- `tests/test_installer.py`
- `tests/test_session.py`
- `tests/test_store_patch.py`
- `docs/evidence/step3/`: device logs, failure results, Store/Bitcoin/Marshmallow screenshots and access-revocation evidence.
