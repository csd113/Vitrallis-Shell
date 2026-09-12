# Repository layout

Vitrallis currently provides PocketCHIP support and a generic desktop backend.
Hardware support, display dimensions/scaling, and input handling have separate
responsibilities. A matching screen size does not establish device support.

```text
devices/pocketchip/
  install.py                  Canonical user installer
  vitrallis-session.py         Awesome/systemd session and recovery
  run-pocketchip.sh            Launch the installed user session
scripts/
  validate.sh                 Shared host validation
  build-pocketchip.sh         Host cross-build tooling for the target ABI
src/
  config.rs                   CLI selection and filesystem conventions
  discovery/
    catalog.rs                Bounded catalog file loading and source precedence
    executable.rs             Executable lookup in cwd/PATH order
    pockethome.rs              PocketHome/Marshmallow format compatibility
  preferences.rs              Normalized display preferences and clock formatting
  launcher.rs                 Application selection and lifecycle state
  process.rs                  Child launching, tracking, and cleanup
  layout.rs                   Reusable dimensions and proportional layout
  input.rs                    Shared SDL keyboard, mouse, and touch translation
  platform/                   Hardware/OS and window/session interfaces
    pocketchip.rs             PocketCHIP hardware and session policy
    pocketchip/
      display.rs              X timeout and time-zone settings
      recovery.rs             Marshmallow return-to-home catalog entry
assets/system/                Shared embedded artwork and provenance
docs/devices/
  pocketchip.md               Installation, selection, and recovery
  pocketchip/                 Settings, Store, and device validation notes
tests/                        Desktop, installer, session, and package tests
```

For a new **device adapter**, extend the existing `Platform` and
`platform::System` interfaces in `src/platform/`, and select it explicitly at
configuration entry. Keep OS paths, hardware commands, and session policy in
that adapter. Add unavailable-data behavior and focused tests before claiming
support. PocketCHIP remains the only device with recorded physical testing;
generic mode supplies local time with hardware controls unavailable.

A device's **installer**, recovery/session helper, and device-only templates
belong in `devices/<device>/`, with setup and limitations in `docs/devices/`.
PocketCHIP's `install.py` also contains its current rollback/repair logic;
there is no general uninstall command. Keep its sibling
`vitrallis-session.py` when staging or copying the installer. See the
[PocketCHIP guide](devices/pocketchip.md) for the payload and recovery instructions.

`discovery::catalog::CatalogFile` handles bounded device-menu reads separately
from `discovery::pockethome::parse_catalog`. The latter understands PocketHome's
Apps-page schema, JUCE command tokenization, stable device-menu IDs and display
preferences. This is an integration boundary with the independent PocketCHIP OS.
It is loaded by PocketCHIP mode or an explicit `--app-config`; desktop startup
uses App Center's manifest discovery. `platform/pocketchip/recovery.rs` supplies
the supervised session's return-to-Marshmallow tile.

Follow [AGENTS.md](../AGENTS.md) when replacing an implementation. Current callers
must use the replacement directly; superseded entry points and formats are removed.

For a **display profile**, use the existing `src/config.rs` and `src/layout.rs`
area: `--size WIDTHxHEIGHT` selects dimensions, and `Layout` validates and scales
the grid. `DISPLAY_480X272` and `DISPLAY_800X480` are reusable dimension defaults,
not hardware identities. `--pocketchip` selects the backend and its fullscreen
default; `--size` changes only dimensions. No profile-file loader exists.
Keep new reusable layout defaults here until a real need warrants a separate
format. Input capabilities come from SDL events in `src/input.rs`, independent
of resolution. `src/platform/pocketchip/display.rs` controls OS screen timeout
and time-zone settings; it is not a layout profile.

A **shared helper** belongs in one clearly named file or module under
`scripts/`, reused by its callers rather than copied into device directories.
The cross-build script remains there because it operates on the source tree
and host toolchain. Resolve script-owned inputs relative to the script or
package, preserve caller-relative user arguments, and test checkout and staged
use from another working directory, including paths with spaces. Runtime
PocketHome asset lookup keeps its existing precedence; shared system icons are
embedded from `assets/system/` at build time.

Root Cargo/toolchain files and the original project reference retain their
standard locations. Historical reports and `docs/evidence/` remain useful
validation records. Build outputs, Python caches, local environment files, and
private keys are already ignored; keep device sysroots and local credentials
out of Git.

Native App Center lives in `src/app_center/`: strict catalog/manifest metadata,
GitHub transport, source settings, runtime/process checks, transactional installer,
discovery, and SDL screen state. `src/renderer/app_center.rs` renders its shared
keyboard/touch target model. Runtime catalogs and app assets are fetched; no apps
are compiled into the shell. See [App Center](app-center.md).
