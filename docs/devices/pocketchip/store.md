# App Center on PocketCHIP

App Center is the built-in Rust/SDL application manager. Follow the
[App Center guide](../../app-center.md) for catalog sources, trust, installation,
updates, repair, removal, controls and recovery. No separate Python updater or
store patch is part of this integration. Shell updates use System Settings →
Software Updates.

Separately distributed App Center applications follow the current manifest v1 and catalog v1 contract. Package
files and icons are fetched only when an app is selected for installation. These
packages use `$XDG_DATA_HOME/vitrallis/apps/<id>` (default
`~/.local/share/vitrallis/apps/<id>`) and a generated launcher. Every application
uses the same canonical directory, icon, live Vitrallis menu registration and
declared runtime; there is no separate per-application special case.

Provide a working system Python with venv/pip support, or a compatible app-local
virtual environment. When no existing runtime satisfies the declared requirements,
installing an app provisions missing declared Python dependencies in a private
app-local environment; App Center never runs apt and never installs system packages
or undeclared prerequisites. In particular, a runtime supplied privately for an
independent application is not a system runtime. The publisher's
`installable: false` flag prevents installation until target integration is
verified.

The existing PocketHome menu remains a read-only source of device applications;
App Center does not register packages in that menu. The supervised session still
provides the Exit Vitrallis tile. Follow [session setup](../pocketchip.md)
for installation and recovery. Python and Rust launch/update/uninstall lifecycles
were physically exercised on the owner's Debian 13 PocketCHIP on 2026-09-19
([2026-09-19 physical validation](app-manager-validation.md)); other images,
physical touch paths and window-identity edge cases remain unverified. Historical
store tests do not certify this package path. See
[current validation](../../app-center-validation.md).

Terminal, Notepad and Files are bundled native utilities. They use the shell’s
complete-bundle installer and updater, and are never App Center uninstall targets.
See [native applications](../../native-apps.md).
