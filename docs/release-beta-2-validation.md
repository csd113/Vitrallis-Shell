# 1.0.0-beta-2 validation

Release preparation started from clean `main` at
`e0f3daf53c17bfdb24f254747acdfaf270c409df`, after the published `v1.0.0-beta`.
The intended tag is `v1.0.0-beta-2`. No dependencies or rendering policy changed.

## Confirmed fixes

- Wireless drew three rows while input used four. Rendering now uses the same
  row count, exposes Tor and aligns every touch target with its visible row.
- Tor arrow movement visited nonexistent controls 6/7. Partial time-zone pages
  and read-only pages also allowed invisible focus. Navigation now stays on
  visible controls, and empty time-zone rows cannot accept a touch.
- App Center warnings were omitted on list pages and replaced by app details
  during uninstall. Every confirmation now draws its warning, disregards old
  scroll offsets and keeps text above its buttons. Publisher/trust answers use
  checked row lookup. Cancel and the existing transaction safeguards remain.
- Text wrapping uses the actual inset width, and editor text is bounded above
  the keyboard. Wireless headings/hints and busy update text are consistent
  with the existing Settings controls. About no longer repeats a software
  renderer label.

## Rendering review

The reported `system-1280x720.bmp` mismatch did not reproduce on the initial
macOS checkout. Linux had no reference block, so its old test did not compare
pixels at all. Each OS/CPU profile now requires a complete reference inventory.

Fresh SDL dummy/software sweeps produced **337 BMPs per OS/CPU profile** at 320×200,
480×272, 800×480 and 1280×720. Native-size contact sheets and scaled screens were
visually inspected, including home/folders/errors, boot/fallback, App Center
lists/details/search/confirmations, Settings controls and all storage states.
Separate Terminal, Notepad and Files captures were inspected at 480×272,
800×480 and 1280×720. The intended changes are the fourth Wireless row and
heading, visible read-only Back selection, disabled busy-update text, correctly
wrapped App Center text and the restored confirmation warnings. Additional
references cover each Wireless/Tor focus target and publisher/trust/removal
prompts. Unchanged scenes retain their macOS hashes; SDL software icon
blending uses architecture-specific rounding, so Linux ARMv7, Linux AArch64,
Linux x86-64 and macOS AArch64 retain exact separate references. The ARMv7
captures match byte-for-byte on SDL 2.26.5 and 2.32.4; differences from AArch64
are confined to blended artwork, with a maximum two-level RGB change. Native
and scaled ARM captures were visually reviewed. The x86-64 SDL 2.26.5 captures
match the reviewed Linux AArch64 files byte-for-byte.

References were updated only after this review through the documented
`VITRALLIS_QA_DIR=… cargo test --lib system_panels_render_at_device_and_scaled_sizes`
capture and BMP SHA-256 workflow. Missing OS/CPU profiles and unreviewed/missing frames
now fail instead of silently bypassing comparison.

## Validation results

`sh scripts/validate.sh` passed on macOS AArch64 and Debian 12 Linux AArch64
after the beta-2 metadata and implementation changes. This includes formatting,
locked workspace checks, strict Clippy (`all`, `pedantic`, `nursery`, `cargo`
with warnings denied), the full Rust/Python suites, release builds, SDL smoke
tests, binary version checks, script syntax and documentation links. macOS ran
354 Rust tests; Linux ran 355. The existing opt-in Rust fixtures remain marked
as such. Python discovery ran 160 tests per host with 9 macOS/8 Linux
platform-dependent skips; the separately configured native renderer pass ran its
dummy-backend comparison. Optional accelerated checks are recorded below.

All four executable version probes and all five workspace packages report
`1.0.0-beta-2`; only the five local package versions changed in Cargo.lock.

- Targeted Settings and App Center regressions passed, including warning pixels
  above buttons, safe defaults and stale confirmation indices.
- `sh tests/simulator/run.sh` passed the 20 fixture App Center scenarios, 10
  shortcut/touch scenarios and the real Awesome stock-session/native lifecycle
  check. The new Settings scenario passed both clock-format restart checks,
  three visits to every category and Tor pointer/keyboard Back navigation.
  The production beta-2 About screen was reviewed at 480×272. Simulator click
  positions now follow the denser App Center geometry; startup waits boundedly
  for the window manager rather than racing its initial map. Bootstrap command
  mocks import only the modules their fixed responses use, avoiding excess
  ARM/QEMU startup cost while retaining the 15-second watchdog and all safety
  assertions; the previously timed-out failure/cleanup scenario passes unchanged.
- `sh tests/simulator/armv7/run.sh` passed all 32 established software gate
  checks with the default 480 MiB/two-CPU limit: cross-built versions, native
  frames/smokes, all staged Rust binaries, update/restore/storage tests under
  umasks 002 and 022, the five-executable bundle upgrade probe and 155 Python
  tests (9 platform-dependent skips). Debian 12 establishes the build ABI;
  execution used Debian 13 armhf with SDL 2.32.4. The existing script reports
  five exact QEMU process-identity/missing-exec artifacts rather than omitting
  those Rust tests; their native Linux counterparts pass. No unexpected
  emulation failure remains.
- The explicit live GitHub catalog contract passed using the ARMv7 test binary:
  all six current published packages' pinned inventories and manifests validated.
  The same opt-in check cannot pass on macOS/AArch64 when the publisher supplies
  no compatible native binary; that host limitation does not change the catalog.
- macOS accelerated shell readback/presentation and native software/hardware/auto
  pixel equivalence passed; native comparisons cover 480×272, 800×480 and 1280×720.
  The separately ignored in-process boot/atlas GPU fixtures reported `No
  available video device` before renderer initialization on this macOS test
  process; their source is unchanged, and they remain target graphical-session
  checks. Actual-binary accelerated readback/presentation passed. Linux
  Mesa/Xvfb rejection of software rendering as hardware also passed.
- The opt-in two-second home/Tor idle checks passed: three startup frames, last
  redraw about 280 ms, no later redraw. Warm rendering workloads passed with zero
  image decodes and zero texture uploads for every 200-frame workload. These are
  host observations, not PocketCHIP throughput or power measurements.
- Ten-second Linux dummy/software idle samples measured Shell at about 1.4% CPU
  and 11 MiB RSS, with 4 KiB RSS growth; Terminal/Notepad/Files measured about
  1.1–1.2% CPU and 8 MiB RSS. No physical device timing/power result is inferred.

Evidence is retained locally in ignored `target/beta2-qa/` (captures and logs)
and `target/app-center-audit/docker/` (interactive scenario reports and images).
An interrupted Docker run left a Terminal test ELF that crashed in the dynamic
loader before tests started; GDB reported invalid debug headers. Rebuilding that
package fixed the invalid artifact; the full suite was rerun without exclusions.
Docker disk exhaustion also caused temporary-file failures during ARM emulation;
unused build cache was cleared and the affected checks were rerun. No source,
release artifacts or Docker data volumes were removed.

## Scope and remaining limitations

No physical PocketCHIP testing was performed for this release. Real radio/time
services, suspend/resume, physical touch and Fn keys, Mali/Lima VSync/scanout,
UBIFS/NAND power-loss behavior and endurance remain in the
[hardware acceptance checklist](devices/pocketchip/v1.0-hardware-acceptance.md).
Simulated window focus, Home/resume, crashes and restarts validate software
lifecycle behavior only. Optional graphical tests require a working backend;
Mesa/Xvfb does not prove physical GPU execution.

App Center provisioning needs the publisher's runtime prerequisites and network;
acquisition cancellation can wait for a bounded request, while an active commit
finishes or rolls back. Apps run as the user and are not sandboxed. Fonts lack
full Unicode shaping. Existing third-party artwork/licensing caveats remain in
[third-party notices](../THIRD_PARTY_NOTICES.md).

## Release workflow

The established tag workflow rebuilds and validates Linux x86-64 and ARMv7,
packages Shell/Terminal/Notepad/Files/Arti, verifies the upgrade probe and stages
19 assets including sidecars, matching helpers and notices. Publication is held
until branch CI, that workflow and review of the downloaded draft assets pass.
The release UI and GitHub Actions retain the remote publication evidence.

## Files changed

- [Cargo.lock](../Cargo.lock)
- [Cargo.toml](../Cargo.toml)
- [README.md](../README.md)
- [docs/release-beta-2-validation.md](../docs/release-beta-2-validation.md)
- [docs/releases.md](../docs/releases.md)
- [docs/validation.md](../docs/validation.md)
- [docs/visual-design-validation.md](../docs/visual-design-validation.md)
- [docs/visual-design.md](../docs/visual-design.md)
- [src/app_center/screen.rs](../src/app_center/screen.rs)
- [src/renderer.rs](../src/renderer.rs)
- [src/renderer/app_center.rs](../src/renderer/app_center.rs)
- [src/renderer/system.rs](../src/renderer/system.rs)
- [src/renderer/system_wireless.rs](../src/renderer/system_wireless.rs)
- [src/settings.rs](../src/settings.rs)
- [src/settings/device.rs](../src/settings/device.rs)
- [src/settings/footer.rs](../src/settings/footer.rs)
- [src/settings/keyboard_tests.rs](../src/settings/keyboard_tests.rs)
- [src/settings/pointer.rs](../src/settings/pointer.rs)
- [src/settings/tor.rs](../src/settings/tor.rs)
- [src/settings/wireless.rs](../src/settings/wireless.rs)
- [src/ui.rs](../src/ui.rs)
- [src/updater/tests.rs](../src/updater/tests.rs)
- [tests/bootstrap_fixture.py](../tests/bootstrap_fixture.py)
- [tests/fixtures/renderer/phase1-sha256.json](../tests/fixtures/renderer/phase1-sha256.json)
- [tests/simulator/README.md](../tests/simulator/README.md)
- [tests/simulator/lifecycle.py](../tests/simulator/lifecycle.py)
- [tests/simulator/published.py](../tests/simulator/published.py)
- [tests/simulator/run.sh](../tests/simulator/run.sh)
- [tests/simulator/settings.py](../tests/simulator/settings.py)
- [tests/simulator/start.sh](../tests/simulator/start.sh)
