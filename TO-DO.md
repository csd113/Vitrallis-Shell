# To Do

- [x] Integrate the PocketCHIP Mali-400/Lima GPU utilization fix into Vitrallis Shell by ensuring the required GPU OPP/devfreq device-tree configuration is installed as part of PocketCHIP support, and update Vitrallis Debug to detect Lima GPUs and use the real devfreq_monitor utilization data instead of reporting that no supported utilization counter exists.
- [x] Add a Vitrallis Shell requirement that the desktop environment and all native shell rendering paths must use double buffering and synchronize frame presentation to VSync/vblank to prevent tearing across the system UI.
- [x] App manager can fail on first app install attempt if pip dependency installer lags behind causing a false failure
- [x] refine the bottom row options on main apps page, remove settings [power] and add shortcut just keep Actions
- [x] instead of a * to indicate an app is running there should be some better icon or animation
- [x] create folders and add apps into folders this should be added into actions
- [x] App manager needs rust app support

## App data and storage accounting

- [x] Store app-created data in per-app directories under `/home/<user>/documents/` instead of root-level directories; distinguish user documents/media from internal settings and caches.
- [x] Include app-created files in each app's Settings → Storage total; show installed files, runtime dependencies, user data, and caches separately.
- [x] Attribute Carousel's stored images/media to its shared Python/Rust library; count the shared allocation only once in aggregate totals.

## Background-app policy and startup

- [x] Add a configurable background-app timeout and per-app **Essential / Keep Running** flags; essential apps remain running until manually closed. Remove the manually assigned default policy and hard-coded exemptions.
- [x] Expose the effective policy in Settings and persist it by stable app ID.
- [x] Define safe shutdown behavior for apps with unsaved work; do not silently force-kill them on timeout.
- [x] Continue tracking delayed windows without requiring another launch, spawning duplicates, or stealing focus after the user navigates elsewhere.
      
## Everyday interface fixes

- [x] Restore the 12/24-hour clock setting and persistence.
- [x] Make Escape in Files dismiss active dialogs first, otherwise navigate to the parent directory instead of closing the app. Retain an explicit exit action.
- [x] Merge Wi-Fi, wireless switches, and Tor settings under the existing Wi-Fi button; rename it **Wireless Network Controls**.
- [x] Support reordering apps and folders on the main menu; persist order by stable IDs across restarts and catalog changes.

## Documentation and licensing

- [ ] Update Apps `docs/runtime-integration.md` to describe automatic app-local Python dependency provisioning and remaining system prerequisites.
- [ ] Replace the obsolete Flasher README installer path with the current Shell installation instructions under `integrations/pocketchip/`.
- [ ] Reconcile App Center action names, installation instructions, runtime support, and compatibility statements across repositories.
- [ ] Remove stale release-readiness statements; distinguish current implementation, published artifacts, and dated hardware evidence.
- [ ] Choose and publish project licensing terms for Shell, Apps, and Flasher; align Cargo metadata, READMEs, and contribution guidance.
- [ ] Review imported code, artwork, and bundled dependencies for redistribution requirements and required notices.

## Optional recovery improvement

- [ ] Add **Restore Previous Build** using the existing retained generation and update lock; do not introduce a second updater/recovery subsystem.
- [ ] Validate the previous generation, require apps and mutations to be stopped, confirm the action, and switch atomically.
- [ ] Preserve the current build and user data if validation or rollback fails.

## Application lifecycle, launch feedback and interface pass

- [x] Acknowledge a launch immediately and prepare the process on a worker so the main menu keeps rendering, processing input and staying visibly alive.
- [x] Returning to the main menu moves an app into the background instead of terminating it; explicit close, self-exit and the configured background lifetime remain the only stops.
- [x] Refuse duplicate launches while one is in flight and derive every running indicator from the authoritative process state.
- [x] Show launching/exit notices in the lower-left status area, and an unmistakable RUNNING state chip, instead of a modal loading screen.
- [x] Reorganise Settings into large home options with Storage as a first-class category and predictable one-level back navigation.
- [x] Give the App Center a readable list, state chips, a prioritised details page and one-line failure summaries.
- [x] Reserve Terminal and Notepad vertical space for their content, with a visible cursor and clear save state.

## Non-blocking visual polish

- [x] Correct progress-bar colour blending.
- [x] Remove the stray line at the bottom-right and occasional bottom-left edges of the app selection window.
- [x] Expand Actions menu options across the available screen width.
- [x] Fade the boot animation into the desktop instead of ending abruptly.

## Additional apps and hardware validation (non-blocking)

- [ ] Create a calculator app.
- [ ] Run Raspberry Pi 1 and Raspberry Pi 2 validation; record results and limitations separately for each model.
- [ ] Run Raspberry Pi 4 validation; record results and limitations.
