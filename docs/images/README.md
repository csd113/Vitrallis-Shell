# README screenshot provenance

The six images linked from the [project README](../../README.md) were captured on
2026-10-06 from **Vitrallis 1.0.4 running on a physical PocketCHIP** over USB SSH.
They are real 480×272 display captures, not simulator images or mockups.

## Build and environment

- PocketCHIP, Debian 13 ARMv7 hard-float; X11/Awesome, native 480×272 display.
- Published `v1.0.4` ARMv7 complete bundle, downloaded from the
  [release](https://github.com/csd113/Vitrallis-Shell/releases/tag/v1.0.4).
  Bundle SHA-256: `eb2b9be59066112668f2802a91bfb5205ba3c7ded6f181dbda2b5d378b62212c`.
  The sidecar checksum and all five embedded executable hashes were verified before
  extraction and execution.
- Release source: `874526879bf0defb25d68fd493d066623c0f3fd8`. Current `main` at
  capture time was `1fd29aecd26546866f002a176207d4b0ed7fca9e`; only README/release
  documentation differs between those commits. Application code is identical.
- The release binaries ran from a private temporary directory. The existing
  installed 1.0.3 generation, session, preferences and apps were preserved.
- Auto selected SDL `opengles2`; startup reported Mesa `Mali400`, OpenGL ES 2.0
  Mesa 25.0.7, 480×272 window/output/display and requested SDL VSync.
  These diagnostics and still images do not measure frame pacing or endurance.

## Actual states

| Asset | State shown |
| --- | --- |
| [Launcher](launcher-480x272.png) | Terminal, Notepad, Files, App Center and System Settings. Battery/charging, Wi-Fi and clock readings come from the live device. |
| [Terminal](terminal-480x272.png) | Real Bash PTY: a colored greeting, `ls` and `cat todo.txt` against sample files. The command waits for input after displaying its output. Terminal's static `--screenshot` preview fixture was not used. |
| [Notepad](notepad-480x272.png) | Sample `notes.txt` containing a weekend checklist, opened without edits. |
| [Files](files-480x272.png) | Sample Documents directory with empty Notes/Projects folders, `notes.txt` and `todo.txt`. |
| [Settings](settings-480x272.png) | Category overview with live device status; no setting or power action was activated. |
| [App Center](app-center-480x272.png) | Before first refresh in an isolated profile. No catalog entries or third-party apps are shown; no package operation was performed. |

A private temporary HOME and XDG configuration/data directories kept existing app
installs, catalogs, folders and personal data out of the captures. A temporary
PocketHome-format menu contained no third-party entries and retained the device's
existing Wi-Fi utility command for the System Settings tile. Native apps and App
Center were discovered normally, without `--demo`. Sample documents lived in the
same temporary workspace; their displayed paths contain no personal files.

The running windows used `--size 480x272` and fullscreen (the shell selected
`--linux-handheld`). Each capture used Pillow's X11 `ImageGrab.grab()` against the
physical device's entire 480×272 display. The frames were saved as lossless PNG
with compression optimization and transferred unchanged. No image was cropped,
resized, composited, annotated, retouched or AI-generated. Temporary capture
processes were closed and the original session regained focus.

## Repeating captures and evidence limits

Use an isolated profile and reviewed matching binaries; preserve the installed
session and user data. Wait for the shell's `event=ready` and live status refresh
before capturing the launcher. Open Settings from its tile, then close it and open
App Center without refreshing its empty profile. For native utilities, open only
sample documents/directories and run harmless commands through Terminal's
`--command` option. Capture each active fullscreen window on the device, verifying
that the resulting image is 480×272.

For desktop-only documentation work, the existing SDL `--screenshot NEW.bmp`
exports and [Linux simulator](../../tests/simulator/README.md) remain useful.
Label those environments explicitly; Terminal's static export is not evidence
of a live PTY. Lossless BMP-to-PNG conversion should preserve identical RGB pixels.

This bounded hardware session establishes the displayed states on this device.
It is **not full 1.0.4 hardware certification** and does not retest public
installation, cold boot, physical keyboard/touch, audio, power-loss durability or
long-running behavior. Earlier release-qualified results remain in the
[device guide](../devices/pocketchip.md) and [validation records](../validation.md).

The earlier [desktop launcher image](shell-480x272.png) is retained at its original
path for existing references. Historical physical-device screenshots under
`docs/devices/pocketchip/evidence/` are unchanged.
