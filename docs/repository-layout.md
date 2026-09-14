# Repository layout

Vitrallis currently provides a Linux handheld backend and a generic desktop backend.
Hardware support, display dimensions/scaling, and input handling have separate
responsibilities. A matching screen size does not establish device support.

```text
integrations/pocketchip/
  bootstrap.py                Bounded release download and verification
  install-session.py                  Canonical user installer
  uninstall.py                Offline receipt-based removal and recovery
  vitrallis-session.py         Awesome/systemd session and recovery
  run-session.sh            Launch the installed user session
apps/{terminal,notepad,files}/  First-party Rust binary/library workspace packages
crates/vitrallis-native/       Small SDL UI, document, browser, filesystem and IPC helpers
assets/native/                Original SVG sources and embedded 128px PNG icons
scripts/
  package-shell-release.py    Four-binary bundle, ARMv7 session helpers and checksums
  package-source.py           Complete Cargo workspace source archive
  validate.sh                 Shared host validation
  build-armhf.sh         Host cross-build tooling for the target ABI
src/
  native.rs                   Built-in registry integration and supervised open requests
  config.rs                   CLI selection and filesystem conventions
  discovery/
    catalog.rs                Bounded catalog file loading and source precedence
    executable.rs             Executable lookup in cwd/PATH order
    pockethome.rs              Stock PocketHome read-only format integration
  preferences.rs              Normalized display preferences and clock formatting
  renderer.rs                 SDL Canvas/Texture drawing and pre-present screenshot readback
  renderer/backend.rs         Capability selection, fallback and typed startup diagnostics
  launcher.rs                 Application selection and lifecycle state
  process.rs                  Child launching, tracking, and cleanup
  layout.rs                   Reusable dimensions and proportional layout
  input.rs                    Shared SDL keyboard, mouse, and touch translation
  platform/                   Hardware/OS and window/session interfaces
    linux_handheld.rs        Linux sysfs/ALSA hardware and session policy
    linux_handheld/
      display.rs              X timeout and time-zone settings
      recovery.rs             Exit Vitrallis catalog entry
assets/system/                Shared embedded artwork and asset guidance
docs/devices/
  pocketchip.md               Installation, selection, and recovery
  pocketchip/                 Settings, Store, and device validation notes
tests/                        Desktop, installer, session, and package tests
```

For a new **device adapter**, extend the existing `Platform` and
`platform::System` interfaces in `src/platform/`, and select it explicitly at
configuration entry. Keep OS paths, hardware commands, and session policy in
that adapter. Add unavailable-data behavior and focused tests before claiming
support. See the device guide for the limited recorded hardware testing;
generic mode supplies local time with hardware controls unavailable.

A device's **installer**, recovery/session helper, and device-only templates
belong in `integrations/<device>/`, with setup and limitations in `docs/devices/`.
Device integration directories retain their device names; `integrations/pocketchip/`
contains the PocketCHIP installation and Awesome/systemd session helpers.
The ARMv7 `install-session.py` also contains its current rollback/repair logic;
the installer and self-contained offline uninstaller share filesystem guards. Keep
`install-session.py`, `uninstall.py` and `vitrallis-session.py` together when staging. See the
[device guide](devices/pocketchip.md) for the payload and recovery instructions.

`discovery::catalog::CatalogFile` handles bounded device-menu reads separately
from `discovery::pockethome::parse_catalog`. The latter understands PocketHome's
Apps-page schema, JUCE command tokenization and stable imported IDs. It filters
verified stock utility commands without changing their source menu.
It is loaded by Linux handheld mode or an explicit `--app-config`; desktop startup
combines the native registry with App Center's manifest discovery. `platform/linux_handheld/recovery.rs` supplies
the supervised session's Exit Vitrallis tile.

Follow [CONTRIBUTING.md](../CONTRIBUTING.md) when replacing an implementation. Current callers
must use the replacement directly; superseded entry points and formats are removed.

For a **display profile**, use the existing `src/config.rs` and `src/layout.rs`
area: `--size WIDTHxHEIGHT` selects dimensions, and `Layout` validates and scales
the grid. `DISPLAY_480X272` and `DISPLAY_800X480` are reusable dimension defaults,
not hardware identities. `--linux-handheld` selects the backend and its fullscreen
default; `--size` changes only dimensions. No profile-file loader exists.
Keep new reusable layout defaults here until a real need warrants a separate
format. Input capabilities come from SDL events in `src/input.rs`, independent
of resolution. `src/platform/linux_handheld/display.rs` controls OS screen timeout
and time-zone settings; it is not a layout profile.

A **shared helper** belongs in one clearly named file or module under
`scripts/`, reused by its callers rather than copied into device directories.
The cross-build script remains there because it operates on the source tree
and host toolchain. Resolve script-owned inputs relative to the script or
package, preserve caller-relative user arguments, and test checkout and staged
use from another working directory, including paths with spaces. Runtime
PocketHome asset lookup keeps its existing precedence; shared system icons are
embedded from `assets/system/` at build time.

Root Cargo/toolchain and contributor files retain standard locations. Design
guidance lives in `docs/design.md`; consolidated historical observations and
`docs/devices/pocketchip/evidence/` retain useful validation records. Build outputs, Python caches, local environment files, and
private keys are already ignored; keep device sysroots and local credentials
out of Git.

Native App Center lives in `src/app_center/`: strict catalog/manifest metadata,
GitHub transport, source settings, runtime/process checks, transactional installer,
discovery, and SDL screen state. `src/renderer/app_center.rs` renders its shared
keyboard/touch target model. Third-party catalogs and app assets are fetched.
Bundled applications are separate executables resolved from the running shell's
immutable build directory. See [native applications](native-apps.md) and
[App Center](app-center.md).

The root package alone is not a distributable source workspace. Use
`python3 scripts/package-source.py --output target/vitrallis-source.tar.gz` to
archive all Cargo-owned files, original workspace manifests and the shared lockfile.
The source-package test verifies inclusion of every native member and icon.
