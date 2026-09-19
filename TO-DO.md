# To Do

- [x] Integrate the PocketCHIP Mali-400/Lima GPU utilization fix into Vitrallis Shell by ensuring the required GPU OPP/devfreq device-tree configuration is installed as part of PocketCHIP support, and update Vitrallis Debug to detect Lima GPUs and use the real devfreq_monitor utilization data instead of reporting that no supported utilization counter exists.
- [x] Add a Vitrallis Shell requirement that the desktop environment and all native shell rendering paths must use double buffering and synchronize frame presentation to VSync/vblank to prevent tearing across the system UI.

- [x] App manager can fail on first app install attempt if pip dependency installer lags behind causing a false failure

- [x] refine the bottom row options on main apps page, remove settings [power] and add shortcut just keep Actions
- [ ] choose a new name for Actions: [INSERT NAME HERE WHEN I THINK OF IT]
- [x] instead of a * to indicate an app is running there should be some better icon or animation
- [x] create folders and add apps into folders this should be added into actions
- [x] App manager needs rust app support


- [ ] allow configureable time for apps to run in background before being automaticaly killed, and to set programs as essential meaning they will be allowed to live in the background until user ends them manually
- [ ] create calculator app
- [x] implement beta update channel — [automatic prerelease updates](docs/shell-updates.md): beta builds receive published previews; stable builds remain stable-only. A manual channel selector is not implemented.
- [ ] app manager needs to allow for beta app channels for development builds



- [ ] interface themes, forest theme, 4chan theme, fruger areo theme.

- [ ] Raspberry pi 1/2 validation testing
- [ ] Raspberry pi 4 validation testing
