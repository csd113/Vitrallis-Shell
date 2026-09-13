# Beta2.6 PocketCHIP follow-up

Validated 2026-09-12 after publication on an actual PocketCHIP running Debian 13,
ARMv7 hard-float Linux, kernel `6.12.94+deb13-chip`, with its existing Awesome/X11
desktop at 480×272. Access used the owner's normal desktop account over SSH.

## Fresh installation

The previous beta2.5 updater searched for the obsolete standalone ARM shell
asset. Beta2.6 publishes a complete four-binary bundle, so the old updater's
"no shell build" result did not mean an ARM release was missing.

The owner authorized archiving the old installation and clearing active apps and
App Center data for a fresh release test. The old shell, receipt, helpers, app
files, App Center state and five managed desktop shortcuts were moved into a
private rollback backup outside the active installation. The PocketHome menu was
also backed up. The existing supervised shell was verified and stopped before
replacement; no application subprocesses were running.

Downloaded the published beta2.6 ARM bundle and matching helpers, verified their
sizes and SHA-256 values, extracted the complete bundle, and executed all four
version probes on the device. The installed generation digest was:

```text
5fa11826127643cd8f134f6d3d994ec144fdd5ffe3fda280715e2be755701b2d
```

The published installer passed its real device preflight and installed a fresh
schema-1 receipt and `current` generation. All four installed binaries reported
`0.1.0-beta2.6`. The supervised unit remained active with its shell process
executing the canonical generation path. No pending installation marker remained.
PocketHome configuration was semantically identical to the backup, unrelated
preferences remained, and the old app shortcuts were absent.

## Permissions correction

The device's umask was `002`. The published bootstrap used Python's default
file-creation mode, producing group-writable downloads (`0664`) that the installer
correctly rejected. Staged downloads were made private before installation.
Two existing owner-controlled directories, `.pocket-home` and the Vitrallis
backup root, also required reviewed permission corrections; original modes were
recorded in the rollback backup. Filesystem protections were not bypassed.

The bootstrap on `main` now creates downloads exclusively with mode `0600`.
A regression reproduced the old failure with umasks `002` and `000`; the fixed
test covers those plus `077`, verifies acceptance by the installer's real file
guard, and confirms an existing destination is never overwritten. On the device,
the fixed bootstrap downloaded the actual published bundle/helpers with umask
`002`; every downloaded file was `0600`, helper validation passed, and all four
extracted ARM executables again passed their version probes. This second download
did not replace the running installation.

The README obtains the corrected bootstrap from `main`. The published beta2.6
tag, bootstrap asset, native bundle and installation helpers were not replaced.
No version was bumped and no obsolete-layout compatibility path was added.

## Results and boundaries

- Captured the actual shell and System Settings at 480×272. Injected X11 pointer
  and keyboard input opened the updater and activated its real network check.
  The result displayed "Running Vitrallis Shell 0.1.0-beta2.6" and
  "Vitrallis is up to date."
- App Center initially displayed an empty fresh catalog. Explicit Refresh loaded
  all three published entries. Its Installed filter displayed zero apps, and the
  filesystem confirmed no third-party app installations remained.
- Terminal, Notepad and Files each passed their native ARM 480×272 smoke command
  on the device with SDL's dummy video driver. These are startup checks, not
  interactive usability tests of those applications.
- `sh scripts/validate.sh` passed on the development host: formatting, workspace
  check, strict Clippy, 187 Rust unit tests, six integration tests, 78 Python tests,
  four release builds, shell/native smoke checks, script syntax and documentation
  checks. One existing manual harness remained ignored by the normal Rust suite.

Raw screenshots and the host validation log are local artifacts under
`target/app-center-audit/`. No device address, credentials or private app data
were added to the repository. The device rollback backup remains under
`~/.local/share/vitrallis-backups/beta2.6-fresh-20260912T235442Z/`.

This follow-up does not validate physical touch/keypad hardware, power-loss
durability, sustained performance, Home/resume, native removal or installation
of a later release. The literal README command retains its fixture coverage;
the device used verified staged helpers after manual reconciliation. The broader
App Center lifecycle remains covered by the [Docker audit](../../../app-center-validation.md).
