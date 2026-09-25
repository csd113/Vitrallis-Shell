# System Settings

Settings use the Linux handheld backend and the existing device utilities.

## Controls

- The launcher and settings header show the device's Wi-Fi IPv4 address, falling back to USB IPv4 when Wi-Fi has no address. Missing addresses display `IP --`. IPv6-only networks are not displayed by this backend.
- Settings headers identify the current section. Software updates display the version compiled into the running binary.
- Gear, Wi-Fi, brightness, volume, restart and power icons are transparent PNG assets embedded in the binary. They load once, use linear filtering at small sizes, and require no runtime files or network access. [Asset guidance](../../../assets/system/README.md) documents the artwork requirements.
- **App Center** is the built-in native application manager. Its [package guide](../../app-center.md) covers catalogs, installation, updates and removal. Native shell updates live in **Software Updates**.
- Brightness and volume retain their existing live 10% sliders on the **Display & Sound** page. The home menu lists every category as a large two-line option: Display & Sound, Date & Time, Wireless Network, Applications, Storage, Device, Software Updates and About. Arrows move between options and Enter opens one.
- **Screen timeout:** Never, 30 seconds, 1, 2, 5, 10 or 30 minutes. Left/right keypad changes the selected timeout; tap the left portion to decrease or the remainder to increase. Wait for Applying to finish before another change. Touching a timeout row does not drag the brightness slider.
- **Time zone:** choose from the installed `zone.tab` database plus UTC. Up/down moves one zone; left/right or page keys moves five. Touch selects a visible row; Previous/Next moves a page. `*` marks the current zone. Choosing a zone applies through timedatectl; when authorization is needed, a supervised terminal requests the normal device password through sudo. Ctrl+C cancels; after completion Enter returns to settings. No password is stored or read by Vitrallis. The clock and zone display refresh from system readback, including an immediate refresh when the authorization terminal closes.
- **Calibrate touchscreen:** launches the image's existing `/usr/local/bin/pocketchip-calibration` utility. Tap its crosshairs accurately; any key cancels. The existing utility saves completed calibration for future logins and restores the previous matrix when cancelled. A missing utility displays unavailable; the installer does not install or replace calibration tools.

A visible Back control remains available on every category while a setting reports Saved or an error. Escape dismisses a pending confirmation first, then returns exactly one level: the zone list returns to Date & Time, Tor returns to Wireless Network, and only the home menu returns to the launcher. F1 remains unbound.

## Wireless switches

Open **Wireless Network** for Wi-Fi and Bluetooth on/off switches, the connection manager and the Tor controls. Up/down selects
a row; Enter or a matched touch release toggles it. Left requests Off and Right
requests On. The Wi-Fi connections row opens the existing connection manager;
the main Settings Wi-Fi button also retains that behavior.

Wi-Fi uses the existing [NetworkManager radio switch](https://networkmanager.pages.freedesktop.org/NetworkManager/NetworkManager/nmcli.html).
Bluetooth controls the [BlueZ default controller power state](https://github.com/bluez/bluez/blob/master/doc/org.bluez.Adapter.rst)
through `bluetoothctl`. Bluetooth On means powered, not paired or connected.
The image must supply these utilities/services and allow the session to control
them. Missing controllers/utilities, denied changes and hardware radio blocks
are reported without installing packages or changing authorization policy.

Both switches run on the existing control worker and confirm state through
readback. Changes do not freeze input or start another poller. Repeated activation
is suppressed while Applying. A failed or mismatched readback never reports Saved.
The shell does not force radios on at startup or maintain another radio preference;
OS service policy determines persistence. Turning a radio off disconnects its links.
The new switches have automated mock/input/render coverage; physical radio
transitions still need validation on the target image.

## Persistence, permissions and recovery

Timeout uses the X server's screen saver and DPMS timers, covering the current graphical session. The user confirmed that physical input wakes the display normally after a 30-second timeout. Synthetic XTest input did not wake DPMS on this X server; that automation limitation does not reproduce with physical input. This turns the display off; it does not lock the session or suspend the device. Applications that explicitly inhibit screen saving can still override X's idle behavior.

A selected timeout is saved atomically with mode 0600 in `~/.config/vitrallis/screen-timeout` and restored when Vitrallis next starts. Missing preferences leave the existing timer alone. Unsupported values and symlinked paths are rejected. A failed application/readback/save attempts to restore the previous X timers. The setting stays effective when returning to PocketHome in the same X session; after a fresh login, Vitrallis restores it when launched. Choose Never to disable idle blanking, or choose the previous value to undo a change.

Time-zone changes use the existing device authorization policy and system zone database; no sudoers or polkit rule is installed. The interactive helper validates its single zone argument against that database and invokes the fixed sudo/timedatectl executables with explicit arguments. Only the password-taking subprocess receives the authentication input. All interactive settings utilities remain owned by the Vitrallis session and return to the additional settings page on exit.

The normal worker continues polling status independently of controls. Timestamped readback prevents an older snapshot from reverting newly applied brightness, volume, timeout, time-zone or radio fields. Bounded, noninteractive helper commands retain the two-second deadline. The deliberate interactive password/calibration applications use the existing supervised app lifecycle instead.

Opening Settings changes no PocketHome binary, Awesome configuration, X startup
file, calibration implementation, default boot setting, system package or
authorization policy. Separately, the bootstrap installs missing Debian
prerequisites, and the platform helpers install the documented GPU trace unit and
the account-specific Carousel media sudoers rule; see [installation and
recovery](../pocketchip.md).

## Validation scope

The [historical settings qualification](history/device-validation.md#settings-qualification--september-10-2026) records the beta.1 device observations. Native utilities and installer/removal were physically exercised on Debian 13 at 480×272 on 2026-09-12 ([USB record](validation-usb-session.md)); this checkout's later changes have host and fixture evidence only.
