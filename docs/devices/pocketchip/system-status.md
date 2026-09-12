# PocketCHIP system integration

The device backend supplies status and controls through `platform::System`;
window/session policy remains in `Platform`. Generic desktop mode supplies local
time and unavailable hardware values. Read [settings](settings.md) for controls
and [historical qualification](../../history/device-validation.md) for observations
on the inspected Debian 13 image.

## Mechanisms

| Capability | Current adapter and limits |
| --- | --- |
| Battery and external power | Prefer the kernel `axp20x-battery`, `axp20x-usb` and `axp20x-ac` power-supply sysfs interfaces. If the kernel battery driver is present, malformed data never triggers forced I²C access. |
| Optional battery backend | Without that kernel driver, an already installed `i2cget` can read the AXP209 gauge/status registers. This is a supported OS integration option, not an older Vitrallis implementation. It installs no package or permissions. |
| Brightness | Bounded backlight sysfs I/O. Requires the inspected native range of ten lit levels; unsupported ranges disable the control. No missing node is created. |
| Audio | Existing `amixer` Power Amplifier control; validated percentages and actual readback. Missing, malformed or inconsistent channels are unavailable. |
| Wi-Fi and IP | Existing NetworkManager `nmcli`; state includes off, disconnected, connecting, connected and unavailable. Header address prefers Wi-Fi IPv4 with USB IPv4 as fallback. Connection management opens the configured OS utility. |
| Clock and time zone | Existing local clock and device zone database; selected zone changes use normal OS authorization. The status clock respects the PocketHome time format. |
| Reboot and power-off | Fixed `systemctl` actions, explicit Cancel-default confirmation and normal OS permissions. No privilege rules are installed. |
| Bluetooth | No validated live pairing backend; no connection claim or control is supplied. |

The optional register interpretation follows the
[AXP209 datasheet](https://aw-som.com/docs/public/products/AXP209_Datasheet_v1.0en.pdf):
the gauge's suspended/uninitialized states are unavailable, and external power
and battery charging are interpreted separately. Vitrallis does not mutate PMIC
configuration. Forced I²C access requires appropriate bus ownership and permissions
on the image; use the kernel interface when it is bound.

## Responsiveness and unavailable data

A worker uses bounded queues, one outstanding control and cached snapshots.
A full refresh occurs ten seconds after the preceding refresh finishes. Controls
reread their affected fields without triggering an unrelated full probe. Missing
data replaces old values; after 30 seconds without an update, hardware status is
cleared. The renderer shows unavailable values rather than inferred success.

Noninteractive helpers use explicit argv, C locale, fixed executable directories,
bounded output and a two-second timeout. Timed-out children are reaped. Sysfs reads
are bounded, missing paths are never created, and permissions are not escalated.
An OS/filesystem stall can outlive a userspace timeout; stale data then becomes
unavailable. These design bounds are not frame-time or battery-life measurements.

Tests cover percent/range limits, malformed and missing output, failed commands,
timeouts, kernel ownership of battery access, invalid gauge/status fields,
readback, stale snapshots, queue behavior and safe power confirmation. Power tests
use mocks. Historical hardware observations do not establish full battery/radio
transitions, audible range, endurance, other images, or current-bundle certification.
