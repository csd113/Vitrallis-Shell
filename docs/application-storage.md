# Application storage contract

Replaceable packages and persistent state are separate for all current managed
applications. These locations depend on the launching user's home, never a
hardcoded PocketCHIP username.

| Content | Location | Owner and lifecycle |
| --- | --- | --- |
| App payload, generated Python environment and installation receipt | `$HOME/Documents/Vitrallis/Apps/<id>/` | Desktop user; verified App Center transactions replace or remove owned files; custom `.venv` and unmanaged files remain |
| Persistent settings, saves, media and app state | `$HOME/Documents/Vitrallis/AppData/<id>/` | Desktop user; new directories 0700; retained by repair, update and ordinary uninstall |
| Default user documents | `AppData/<id>/Documents/` | Desktop user; native Notepad creates private directories after path validation |
| Disposable app cache | `${XDG_CACHE_HOME:-$HOME/.cache}/<id>/` | Desktop user; app-specific capacity and cleanup policy |
| App Center catalog/presentation cache, launchers, locks, transaction backups | `${XDG_DATA_HOME:-$HOME/.local/share}/vitrallis/app-center/` | Desktop user; bounded reads and receipt-scoped writes |
| Catalog source configuration | `${XDG_CONFIG_HOME:-$HOME/.config}/vitrallis/app-center.json` | Desktop user; managed atomically |
| Shell generations, current/previous pointers, update staging, session helpers | `$HOME/.local/share/vitrallis/` | Desktop user; immutable generation payloads and locked pointer switches |
| Session logs | `$HOME/.local/share/vitrallis/session.log` and `.log.1` | Desktop user; supervisor bounds and rotates output |
| Shell installer backup | `$HOME/.local/share/vitrallis-backups/` | Desktop user; preserved on uninstall |
| Folder organization | `${XDG_DATA_HOME:-$HOME/.local/share}/vitrallis/folders.json` | Desktop user; retained independently of packages |
| Tor state and configuration | `${XDG_DATA_HOME:-$HOME/.local/share}/vitrallis/tor/` | Desktop user; private 0700 service state; see [Tor](tor.md) |
| X11/IPC and user service sockets | `/run/user/<uid>/` | Desktop user; private runtime, temporary across boots |
| Privileged platform integration | Root-owned helpers/service policy in `/usr/local`, `/etc` and `/var/lib/vitrallis-pocketchip` | Root only for explicit platform provisioning; see [PocketCHIP setup](devices/pocketchip.md) |

Managed launchers export `VITRALLIS_APP_ID`, `VITRALLIS_APP_DIR`,
`VITRALLIS_APP_DATA_DIR` and `VITRALLIS_DOCUMENTS_DIR`. They start with AppData
as their working directory and a private umask. Assets must be found relative
to the entry's own location or APP_DIR. Bundled utilities receive identity and
persistent data/document locations; their binaries belong to the Shell generation.

Traversal, control characters, symlink components and unsafe data directories
are rejected before package mutation. Existing user files are preserved instead
of silently changing their owner or permissions. Permission declarations describe
requirements; managed apps run as the desktop user and are not a filesystem sandbox.

The current pre-release contract replaces the previous scattered storage layout.
The release audit preserved the test device's previous state separately before
clean installation. Saved data from an older build must be reviewed and moved
while the app is closed; do not make aliases into package payloads or copy an
old managed receipt or launcher into the new contract.

The [one-time import tool](../scripts/import-app-data.py) previews explicitly
selected saved files and directories. Run it from a source checkout as the
desktop user, without sudo, while the app is closed. The destination must be
absent: it never merges into existing AppData or overwrites another save.
For example, to import the previous Carousel library, media and preferences:

```sh
python3 scripts/import-app-data.py --app-id io.vitrallis.mediacarousel \
  --file "$HOME/.local/share/io.vitrallis.mediacarousel/library.json" library.json \
  --tree "$HOME/.local/share/io.vitrallis.mediacarousel/media" media \
  --file "$HOME/.config/io.vitrallis.mediacarousel/settings.json" Config/settings.json
```

Review the listed paths and rerun with `--apply`. The tool validates the complete
selected input, requires free space for the copy plus a 32 MiB reserve, streams
into private staging, syncs it and publishes with an exclusive rename. Original
files remain intact. Links, hardlinks, unsafe permissions, overlapping names,
changed files and existing destinations fail closed. A post-commit sync failure
explicitly reports that imported data is present but reboot persistence is
uncertain. Test the imported app before retiring its old saved-data copies.

For Bitcoin, select its previous `settings.json` and `watch.json` individually.
For Places, select `settings.json` and any user-created `levels`/`import` trees;
exclude shipped assets, binaries, receipts, launchers and disposable caches.
Old native documents can be selected with `--tree` and destination `Documents`.
This explicit tool does not add old-path lookup to current applications.

See [App Center](app-center.md) for transactions and recovery, and
[Settings storage](settings-storage.md) for classification and measurement limits.
