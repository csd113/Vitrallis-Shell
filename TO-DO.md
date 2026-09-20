# To Do

- [x] Integrate the PocketCHIP Mali-400/Lima GPU utilization fix into Vitrallis Shell by ensuring the required GPU OPP/devfreq device-tree configuration is installed as part of PocketCHIP support, and update Vitrallis Debug to detect Lima GPUs and use the real devfreq_monitor utilization data instead of reporting that no supported utilization counter exists.
- [x] Add a Vitrallis Shell requirement that the desktop environment and all native shell rendering paths must use double buffering and synchronize frame presentation to VSync/vblank to prevent tearing across the system UI.
- [x] App manager can fail on first app install attempt if pip dependency installer lags behind causing a false failure
- [x] refine the bottom row options on main apps page, remove settings [power] and add shortcut just keep Actions
- [x] instead of a * to indicate an app is running there should be some better icon or animation
- [x] create folders and add apps into folders this should be added into actions
- [x] App manager needs rust app support


- [ ] allow configureable time for apps to run in background before being automaticaly killed, and to set programs as essential meaning they will be allowed to live in the background until user ends them manually and remove the manually assigned default
- [ ] progress bars do not blend colours properly
- [ ] timezone settings removed 12 or 24 hour clock control
- [ ] app selection window has a small line on the bottom right and sometimes left
- [ ] boot animation should fade out into the desktop not abruptly vanish
- [ ] allow reordering apps and folders on main menu
- [ ] app storage calculation should include but differentate files stored in that apps created directory, so for example the media carousel apps stored images should be included in the media carousel under settings storage
- [ ] in settings the wifi option and wireless + tor should be merged so that all wireless and tor settings are under the wifi button, and rename it to wireless network controls
- [ ] action button menu options should extend across entire screen
- [ ] files app uses the esc key to close the app instead of going back one folder level
- [ ] apps should store their data in the /home/(device)/documents folder instead of some root level folder











- [ ] create calculator app

- [ ] Raspberry pi 1/2 validation testing
- [ ] Raspberry pi 4 validation testing
