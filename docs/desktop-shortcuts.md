# Desktop shortcuts

Choose **Manage [F10] → Add shortcut** on the Apps page or press **F2**. You can add an ordinary
Linux program or script without a Vitrallis package, SDK, Python runtime, or App
Center installation. The desktop action remains available when no apps are shown.

Enter a name and command. **Browse executable** selects a file and quotes its
path. Leave **Working directory** empty to use your home directory, or enter an
existing absolute directory. **Choose icon** accepts PNG or BMP images up to
512×512 pixels and 1 MiB; the editor previews the image. **Use default icon**
restores the built-in placeholder. Save validates these fields without executing
the command. Cancel discards the editor's changes.

Examples in direct mode:

```text
firefox --new-window https://example.com
"/home/alex/My Tools/report" "a file with spaces.txt"
/usr/bin/bash "/home/alex/Scripts/daily report.sh"
"/home/alex/Scripts/executable script"
```

An executable name is resolved through the shell user's `PATH`. Executable
scripts need a valid shebang and executable permission; alternatively, name
their interpreter explicitly as above. Single and double quotes group arguments,
and backslashes escape characters. Empty quoted arguments are retained. Direct
mode does not expand variables, globs, or substitutions and never implicitly
passes the command to a shell.

For pipelines and redirects, explicitly enable **Shell mode (/bin/sh -c)**:

```text
printf '%s\n' "$HOME" | tee "$HOME/shortcut-result.txt"
```

**Run in terminal** opens the bundled native terminal with the executable and
argument vector, preserving argument boundaries and the working directory. It
does not require an external terminal or Python. Shell mode also works inside
the terminal. Commands inherit your session permissions; shortcuts never add
`sudo` or request automatic privilege escalation. Programs remain responsible
for their own runtime dependencies and graphical-session requirements.

Right-click an icon, hold and release a touch on it, or select it and press
**F10** (or the keyboard Menu key) to open its actions. These gestures open the
menu without launching the app. **Tab** moves focus between the desktop and its
footer buttons; Enter activates the highlighted button. In dialogs, Tab/arrows
select controls and Enter activates them. Page Up/Page Down or the visible Up/Down
buttons scroll lists. Text fields provide an on-screen keyboard, Backspace,
Clear, Done, and Cancel; a physical keyboard also works, including Ctrl+A to clear.

Removal depends on where an entry came from:

- **Your shortcuts:** Edit shortcut changes the record. Remove shortcut asks
  for confirmation and deletes only that shortcut and its copied icon. It never
  deletes the executable, script, installed package, or application data—even
  when the shortcut launches an App Center application.
- **App Center applications:** Uninstall app opens App Center's normal
  confirmation. It uses local installation receipts, including for custom
  repositories, without downloading a catalog. The same ownership checks,
  running-app protections, backups, and transactional uninstall apply.
- **Other discovered entries:** Remove shortcut hides the entry in Vitrallis.
  It does not delete a system launcher, operating-system package, or program.
  App Center and System Settings cannot be hidden.

Removal confirmations initially select Cancel. Errors are shown on screen.

Shortcuts live in `$XDG_DATA_HOME/vitrallis/shortcuts`, or
`~/.local/share/vitrallis/shortcuts` when `XDG_DATA_HOME` is unset or empty.
Each namespaced JSON record contains its own copied icon bytes, so one atomic
write saves both together. Moving or deleting the original icon is safe. This
directory is separate from installed packages, receipts, and repository caches;
catalog refreshes and shell updates do not replace it. Malformed records are
reported independently so other shortcuts can still load. Hidden unmanaged
entries are recorded in `hidden.json` in the same directory; removing that file
while Vitrallis is closed restores those entries on the next start.
