# System Settings

Settings use the PocketCHIP backend and the existing device utilities.

## Controls

- The launcher and settings header show the device's Wi-Fi IPv4 address, falling back to USB IPv4 when Wi-Fi has no address. Missing addresses display `IP --`. IPv6-only networks are not displayed by this backend.
- The System Settings header displays the version compiled into the running binary, rather than a potentially different installed receipt.
- Gear, Wi-Fi, brightness, volume, restart and power icons are transparent PNG assets embedded in the binary. They load once, use linear filtering at small sizes, and require no runtime files or network access. [Asset guidance](../../../assets/system/README.md) documents the artwork requirements.
- **App Center** is the built-in native application manager. Its [package guide](../../app-center.md) covers catalogs, installation, updates and removal. Native shell updates live in **More → Check for Updates**.
- Brightness and volume retain their existing live 10% sliders. Choose **More** at the bottom, move down to More and press Enter, or press Page Down to reach the additional settings.
- **Screen timeout:** Never, 30 seconds, 1, 2, 5, 10 or 30 minutes. Left/right keypad changes the selected timeout; tap the left portion to decrease or the remainder to increase. Wait for Applying to finish before another change. Touching a timeout row does not drag the brightness slider.
- **Time zone:** choose from the installed `zone.tab` database plus UTC. Up/down moves one zone; left/right or page keys moves five. Touch selects a visible row; Previous/Next moves a page. `*` marks the current zone. Choosing a zone applies through timedatectl; when authorization is needed, a supervised terminal requests the normal device password through sudo. Ctrl+C cancels; after completion Enter returns to settings. No password is stored or read by Vitrallis. The clock and zone display refresh from system readback, including an immediate refresh when the authorization terminal closes.
- **Calibrate touchscreen:** launches the image's existing `/usr/local/bin/pocketchip-calibration` utility. Tap its crosshairs accurately; any key cancels. The existing utility saves completed calibration for future logins and restores the previous matrix when cancelled. A missing utility displays unavailable; the installer does not install or replace calibration tools.

Back and More remain visible while a setting reports Saved or an error. Escape returns from the zone list to additional settings, from additional settings to the first settings page, and from there to the launcher. F1 remains unbound.

## Persistence, permissions and recovery

Timeout uses the X server's screen saver and DPMS timers, covering the current graphical session. The user confirmed that physical input wakes the display normally after a 30-second timeout. Synthetic XTest input did not wake DPMS on this X server; that automation limitation does not reproduce with physical input. This turns the display off; it does not lock the session or suspend the device. Applications that explicitly inhibit screen saving can still override X's idle behavior.

A selected timeout is saved atomically with mode 0600 in `~/.config/vitrallis/screen-timeout` and restored when Vitrallis next starts. Missing preferences leave the existing timer alone. Unsupported values and symlinked paths are rejected. A failed application/readback/save attempts to restore the previous X timers. The setting stays effective when returning to Marshmallow in the same X session; after a fresh login, Vitrallis restores it when launched. Choose Never to disable idle blanking, or choose the previous value to undo a change.

Time-zone changes use the existing device authorization policy and system zone database; no sudoers or polkit rule is installed. The interactive helper validates its single zone argument against that database and invokes the fixed sudo/timedatectl executables with explicit arguments. Only the password-taking subprocess receives the authentication input. All interactive settings utilities remain owned by the Vitrallis session and return to the additional settings page on exit.

The normal worker continues polling status independently of controls. Timestamped readback prevents an older snapshot from reverting newly applied brightness, volume, timeout or time-zone fields. Bounded, noninteractive helper commands retain the two-second deadline. The deliberate interactive password/calibration applications use the existing supervised app lifecycle instead.

No Marshmallow binary, Awesome configuration, X startup file, calibration implementation, default boot setting, system package, or authorization policy is changed by the Vitrallis installer.

## Validation scope

The [historical settings qualification](../../history/device-validation.md#settings-qualification--september-10-2026) records the beta.1 device observations. Current native utilities and installation/removal changes require fresh physical validation.
