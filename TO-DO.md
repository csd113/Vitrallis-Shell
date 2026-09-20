# To Do

- [x] Integrate the PocketCHIP Mali-400/Lima GPU utilization fix into Vitrallis Shell by ensuring the required GPU OPP/devfreq device-tree configuration is installed as part of PocketCHIP support, and update Vitrallis Debug to detect Lima GPUs and use the real devfreq_monitor utilization data instead of reporting that no supported utilization counter exists.
- [x] Add a Vitrallis Shell requirement that the desktop environment and all native shell rendering paths must use double buffering and synchronize frame presentation to VSync/vblank to prevent tearing across the system UI.
- [x] App manager can fail on first app install attempt if pip dependency installer lags behind causing a false failure
- [x] refine the bottom row options on main apps page, remove settings [power] and add shortcut just keep Actions
- [x] instead of a * to indicate an app is running there should be some better icon or animation
- [x] create folders and add apps into folders this should be added into actions
- [x] App manager needs rust app support

## App data and storage accounting

- [ ] Store app-created data in per-app directories under `/home/<user>/documents/` instead of root-level directories; distinguish user documents/media from internal settings and caches.
- [ ] Attribute declared external data directories to their owning apps without scanning arbitrary home directories or following unsafe symlinks.
- [ ] Include app-created files in each app's Settings → Storage total; show installed files, runtime dependencies, user data, and caches separately.
- [ ] Attribute Carousel's stored images/media to its shared Python/Rust library; count the shared allocation only once in aggregate totals.
- [ ] Preserve existing user data when changing storage locations; document any required relocation.
- [ ] Test shared directories, missing paths, permission failures, separate filesystems, and refresh after media changes.

## Background-app policy and startup

- [ ] Add a configurable background-app timeout and per-app **Essential / Keep Running** flags; essential apps remain running until manually closed. Remove the manually assigned default policy and hard-coded exemptions.
- [ ] Expose the effective policy in Settings and persist it by stable app ID.
- [ ] Define safe shutdown behavior for apps with unsaved work; do not silently force-kill them on timeout.
- [ ] Update window-readiness handling in `src/process.rs` and `src/ui.rs` for cold starts exceeding the current 30-second deadline.
- [ ] Continue tracking delayed windows without requiring another launch, spawning duplicates, or stealing focus after the user navigates elsewhere.
- [ ] Test slow startup, startup failure, repeated activation, background timeout, exemptions, and return-to-shell behavior.

## Everyday interface fixes

- [ ] Restore the 12/24-hour clock setting and persistence.
- [ ] Make Escape in Files dismiss active dialogs first, otherwise navigate to the parent directory instead of closing the app. Retain an explicit exit action.
- [ ] Merge Wi-Fi, wireless switches, and Tor settings under the existing Wi-Fi button; rename it **Wireless Network Controls**.
- [ ] Support reordering apps and folders on the main menu; persist order by stable IDs across restarts and catalog changes.
- [ ] Verify changed controls with keyboard and touch at 480×272.

## Documentation and licensing

- [ ] Update Apps `docs/runtime-integration.md` to describe automatic app-local Python dependency provisioning and remaining system prerequisites.
- [ ] Replace the obsolete Flasher README installer path with the current Shell installation instructions under `integrations/pocketchip/`.
- [ ] Reconcile App Center action names, installation instructions, runtime support, and compatibility statements across repositories.
- [ ] Remove stale release-readiness statements; distinguish current implementation, published artifacts, and dated hardware evidence.
- [ ] Choose and publish project licensing terms for Shell, Apps, and Flasher; align Cargo metadata, READMEs, and contribution guidance.
- [ ] Review imported code, artwork, and bundled dependencies for redistribution requirements and required notices.

## Release qualification

- [ ] Freeze a release candidate; record Shell commit/tag, bundle checksums, catalog commit, and app versions.
- [ ] Run existing validation and release CI against that candidate; retain results without weakening checks.
- [ ] Install the published candidate on a clean, compatible test system using only public instructions and no development-device configuration.
- [ ] Verify install → launch → settings → app install → launch/return → update → uninstall → shell update → relaunch → reboot.
- [ ] Verify user documents, app data, configuration, and the original desktop remain intact through default update/removal flows; test explicit purge separately.
- [ ] Run controlled network-loss, low-space, interrupted-transaction, and corrupt-package tests on isolated test data; confirm diagnostics and recovery.
- [ ] Run extended idle and representative-app workloads; record duration, CPU, RSS trend, battery consumption, background behavior, and physical tearing/input observations.
- [ ] Record final-artifact results separately from earlier development-build validation; document unresolved limitations.

## Optional recovery improvement

- [ ] Add **Restore Previous Build** using the existing retained generation and update lock; do not introduce a second updater/recovery subsystem.
- [ ] Validate the previous generation, require apps and mutations to be stopped, confirm the action, and switch atomically.
- [ ] Preserve the current build and user data if validation or rollback fails.

## Non-blocking visual polish

- [ ] Correct progress-bar colour blending.
- [ ] Remove the stray line at the bottom-right and occasional bottom-left edges of the app selection window.
- [ ] Expand Actions menu options across the available screen width.
- [ ] Fade the boot animation into the desktop instead of ending abruptly.

## Additional apps and hardware validation (non-blocking)

- [ ] Create a calculator app.
- [ ] Run Raspberry Pi 1 and Raspberry Pi 2 validation; record results and limitations separately for each model.
- [ ] Run Raspberry Pi 4 validation; record results and limitations.
