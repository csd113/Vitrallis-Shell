# App Center on PocketCHIP

App Center is the built-in Rust/SDL application manager. Follow the
[App Center guide](../../app-center.md) for catalog sources, trust, installation,
updates, repair, removal, controls and recovery. No separate Python updater or
store patch is part of this integration. Shell updates use System Settings.

Separately distributed App Center applications follow the current manifest v1 and catalog v1 contract. Package
files and icons are fetched only when an app is selected for installation. These
packages use `$XDG_DATA_HOME/vitrallis/apps/<id>` (default
`~/.local/share/vitrallis/apps/<id>`) and a generated launcher. No application
receives a special directory, version parser, icon, menu registration, or runtime.

Provide an app-local virtual environment or a working system Python and any
required system Tk libraries. App Center reports missing dependencies without
installing them. In particular, a runtime supplied privately for an independent
application is not a system runtime. The publisher's `installable: false` flag
prevents installation until target integration is verified.

The existing PocketHome menu remains a read-only source of device applications;
App Center does not register packages in that menu. The supervised session still
provides the Exit Vitrallis tile. Follow [session setup](../pocketchip.md)
for installation and recovery. Package launch/return and window identity need
physical verification on the target image; historical store tests do not certify
this package path. See [current validation](../../app-center-validation.md).

Terminal, Notepad and Files are bundled native utilities. They use the shell’s
complete-bundle installer and updater, and are never App Center uninstall targets.
See [native applications](../../native-apps.md).
