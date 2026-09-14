# Native applications

Every Vitrallis installation includes **Terminal**, **Notepad**
and **Files**. They are separate Rust/SDL2 executables built with the
shell, available offline immediately, and independent of App Center. They use the
current user's permissions. No Python, external GUI program or download is needed.

## Controls and display

All three support 480×272 and larger resizable windows. The native session defaults
to fullscreen; desktop invocation defaults to 800×480. `--size 480x272` selects a
window for testing. `--fullscreen`, `--help`, `--version`, `--smoke-test` and
`--screenshot NEW.bmp` are common options. Screenshots refuse to replace files.
Notepad and Files accept one path, including after `--` for names starting with a
hyphen. Paths are passed as OS arguments, never interpolated into shell strings.

Dialog buttons have visible keyboard selection. Tab and Left/Right move between
choices, Enter activates, and Escape cancels. Destructive confirmations start on
Cancel. Long information dialogs wrap and scroll with Up/Down, Page Up/Down or
wheel. Path/name prompts support typing, Backspace, Ctrl+A to clear, Enter to
accept and Escape to cancel; Tab visits the field and both buttons. Text entry
uses the physical keyboard; an on-screen typing keyboard is not supplied.
Touch/mouse actions require matching press/release targets. Files uses 20px list
rows at 480×272, scaled with the UI.

## Terminal

Terminal starts a real interactive PTY and resolves a valid executable `$SHELL`,
then `/bin/bash`, then `/bin/sh`. Root invocation is refused. It sets `TERM` and
resizes the PTY with the window. ANSI/VT cursor movement, clear/erase, wrapping,
foreground/background/indexed/RGB colors, alternate screen, scrolling, cursor
visibility and common application-key modes are parsed by vt100. The steady
cursor avoids timer-driven repaint. Unsupported glyphs use a visible fallback;
this bitmap font is not a complete Unicode shaping/font engine.

Printable text, Enter, Backspace, Delete, Tab, Escape, arrows, Home/End,
Page Up/Down, function keys, Ctrl combinations and Alt/meta are forwarded.
Ctrl+C/D/Z/L retain their normal terminal meanings. Shift+Page Up/Down or the
mouse wheel browses scrollback; ordinary typing returns to the live screen.
Shift+F10 selects the small Menu header; Enter opens it, and touch activates the
same menu. Ctrl+Shift+Q or window close asks for confirmation before closing the
PTY and its owned shell. Child exit and EOF show a final status, then return.
The terminal is suitable for shell commands and common ANSI programs; it does
not implement images, hyperlinks, mouse reporting or every xterm extension.

At 480×272 the terminal has a compact 20px header and 8×9 cells (60 columns,
28 rows). Geometry is calculated dynamically, capped at 240 rows × 512 columns.
Scrollback is capped at 1,000 rows; pending output at 64 KiB, input at 16 KiB,
and escape strings at 4 KiB. Excess output applies PTY backpressure; it is not
accumulated indefinitely. A single PTY worker blocks in poll. Its one-second
child-status fallback detects an exited shell whose descendants kept the slave
PTY open. Output, input and resize wake the UI; the prompt has no repaint timer.

## Notepad

Notepad supports New, Open, Save, Save As, Find and Close. Its Open dialog reuses
the Files navigation model. Unsaved New/Open/Close requests offer **Cancel,
Save, Discard**, initially Cancel. A cancelled or failed save retains edits.

| Shortcut | Action |
| --- | --- |
| Ctrl+N / Ctrl+O | New / Open |
| Ctrl+S / Ctrl+Shift+S | Save / Save As |
| Ctrl+F | Find text, wrapping once |
| Ctrl+A / C / X / V | Select all / Copy / Cut / Paste |
| Shift+arrows, Shift+Home/End | Extend selection |
| Arrows, Home/End, Page Up/Down | Move cursor |
| Tab | Insert a tab in the document |
| F6 | Select footer controls / return to editing |
| Tab or Left/Right in footer | Move visible button selection |
| Escape or Close | Close with unsaved-change protection |

Click/tap places the cursor; wheel and page keys move vertically. Long lines
scroll horizontally to keep the cursor visible. The editor loads at most **1 MiB
of UTF-8 text**. It maintains line offsets and edits that index incrementally;
it renders only visible text, without widgets per character or line. File bytes
preserve LF/CRLF, empty documents, no final newline and unrenderable Unicode.
Newline insertion follows the loaded document's newline style. Undo/redo and
syntax highlighting are intentionally absent.

Saves write an adjacent private temporary file, preserve ordinary file mode bits,
flush/sync the complete contents and rename atomically. Save As needs explicit
replacement confirmation for another existing file within the same 1 MiB guard. Symlinks, hard-linked
replacement targets and changed-on-disk files are refused to prevent editing the
wrong object. Conflict checks compare bounded SHA-256 content as well as metadata,
including on filesystems with coarse timestamps; Save As can preserve edits to another file. Failed pre-publication
saves leave the old bytes intact. Absolute/non-UTF-8 OS paths are retained when
opening and saving an existing file; Rename's text prompt refuses names that
cannot be represented as UTF-8. Filesystem durability still depends on the
storage device honoring sync and rename.

## Files

Files is a standalone list-based browser and manager. It starts at `$HOME`, or
an explicitly supplied directory. The list shows the current path, parent entry,
type markers, selection, visible scroll position and entry count. Directories
sort first, then case-insensitive names. Dotfiles are hidden initially.

| Control | Action |
| --- | --- |
| Up/Down, Page Up/Down, Home/End | Select entries |
| Enter / Open | Enter selected directory or open file |
| Left / Backspace | Parent directory |
| F6 or Tab | Visit Open, Menu, Refresh, Close |
| M / Menu | Properties, New folder, Rename, Delete, Copy, Move, Hidden, Go to path |
| F2 / Delete | Rename / confirmed delete |
| F5 / Refresh | Explicit rescan |
| Ctrl+H | Toggle hidden files |
| Escape / Close | Return |

Touch selects a row; tapping the selected row or Open activates it. Wheel scrolls.
A scan reads only one directory and caps inspected entries at **20,000**. It reads
entry types once, caps retained name/path buffers at 8 MiB, and does not continually watch, rescan, recursively size folders
or request metadata for off-screen entries. An inaccessible or oversized listing
keeps the previous valid directory visible. Use Go to path or Terminal for a
directory exceeding the guard.

A bounded 8 KiB content sample dispatches valid text to Notepad, including text
without familiar extensions. Binary, symlink and special files show properties.
Selecting an executable never runs it; textual scripts may be edited. The small
`files::handler` boundary owns type detection separately from launcher/process
code and is the place to register future handlers. Properties show full path,
type, size, modified time and permission bits without recursive directory scans.

New folders require one valid name. Rename and Move never replace an existing
path, even a dangling link, and reject moving a directory into itself. **Move is
an atomic operation within one filesystem**; cross-filesystem moves report a
clear error with Copy/verify/Delete guidance. Delete is permanent, requires
confirmation for the exact selected path, and only removes files, links or empty
folders. Nonempty directories must be emptied explicitly. These limits avoid
partially deleted trees and uncertain copy-then-delete move results.

Copy streams regular files through a 64 KiB buffer. Explicit directory copying
is limited to 64 levels and 100,000 entries, rejects links/devices and stages the
whole destination before exclusive publication. Destination collisions, changing
sources and I/O failures leave the destination unpublished. Progress updates
are coalesced at most ten times per second on one temporary worker. Copy must
finish before Files closes; cancellation is not offered. No operation constructs
a shell command. Privileged or hostile same-account filesystem mutation is not
an isolation boundary; filesystem errors remain visible and recoverable.

## Architecture and lifecycle

The Cargo workspace contains the shell, `crates/vitrallis-native`, and
`apps/{terminal,notepad,files}`. Each app has a small `main.rs` and independently
tested library/model code. Shared SDL initialization, bitmap text,
colors, buttons, focus, dialogs, geometry, browser/picker, bounded document I/O
and filesystem operations live in `vitrallis-native`; it is not a shell rewrite.
The shell and all three native apps use `vitrallis_native::renderer` for renderer
policy, capability verification, GLES2 preference, retries, and diagnostics.
`--renderer auto` (the default) tries SDL accelerated drivers, preferring
`opengles2` when advertised, then falls back to software. `--renderer hardware`
requires acceleration and reports the failed attempts; `--renderer software`
explicitly selects software without GPU attempts. The option is per process.
Native startup logs include the app name, requested/actual mode, SDL flags,
backend, dimensions, and any fallback reason. SDL acceleration flags do not
prove physical GPU use: Mesa software rasterizers can also expose these drivers.

The existing Canvas drawing calls, layout, colors, input, and dirty-frame loops
are unchanged. Font lookup and glyph point/rectangle generation remain CPU-side;
SDL executes drawing and presentation through the selected backend. There is no
new framebuffer upload, shader, raw GL/EGL code, or GPU API requirement beyond
SDL's GLES2-capable architecture. Software rendering remains complete. Screenshots
read the completed backbuffer before presentation invalidates it. SDL device/reset
events request one redraw rather than starting a continuous rendering loop.

Run `sh scripts/validate.sh` for all host gates and native dummy-driver policy,
fallback, readback, and overwrite checks. With a graphical SDL backend available,
run `VITRALLIS_RENDERER_BIN_DIR=target/release VITRALLIS_TEST_ACCELERATED=1
python3 -m unittest discover -s tests -p 'test_native_renderer.py'` for exact native
software/accelerated BMP comparisons at 480x272, 800x480, and 1280x720. The Linux
`tests/simulator/session.py` also checks launch, Home/resume, close, crash/relaunch,
and session restoration; set `VITRALLIS_TEST_ACCELERATED=1` to require hardware
selection in each native application's diagnostics. Host and Mesa/Xvfb checks do
not replace physical Mali-400/Lima or Raspberry Pi vc4/V3D testing of display
handoff, driver recovery, memory pressure, readback, and idle power.

Original geometric icon sources and 128×128 PNGs live in `assets/native/`.

`vitrallis_native::APPLICATIONS` defines stable IDs `io.vitrallis.terminal`,
`io.vitrallis.notepad`, `io.vitrallis.files`. `src/native.rs` integrates them with
normal catalog entries. `AppSource` distinguishes Native, AppCenter, PocketHome,
System and Demo. Reserved native IDs cannot be replaced by discovered entries.
Missing executables retain their tile with a repair diagnostic. Native entries
use the normal Opening transition, `ProcessSet`, running indicator and reap/
resume behavior, without another supervisor or fake manifest-v1 package.

Files sends an absolute OS-byte path to a session-private Unix datagram broker.
The shell validates text dispatch and starts or resumes its owned Notepad.
Existing Notepad receives a bounded open request and applies its unsaved-document
confirmation. Other dialogs/actions finish before the shell activates the request.
Native focus uses the same private inbox and SDL window raise, including during
dialogs. Per-app inbox queues hold at most eight paths; overflow reports an error
without closing Notepad. Socket directories are mode 0700 and user-owned. A
single blocking inbox thread exists per app only during a shell session.
Standalone Files directly starts and waits for its one Notepad child, then reaps
it; normal launcher sessions use the existing shell process owner throughout.

All UI main loops wait for SDL events, repaint changed/exposed state and have no
idle frame timer. Notepad and Files have no idle worker outside a shell session.
Terminal's worker and session inboxes block until needed. No app uses a network
service. See [dependency review](dependencies.md) and [measured validation](devices/pocketchip/history/native-validation.md).

## Builds and installation

Use `cargo build --workspace --locked` or `cargo build --workspace --release --locked`
to produce `vitrallis`, `vitrallis-terminal`, `vitrallis-notepad`, `vitrallis-files`.
`sh scripts/validate.sh` checks every member, the production inventory and both
480×272/800×480 native SDL smoke frames. Release packaging requires all four
matching versions and architectures; a shell-only artifact is rejected.

The [installer](devices/pocketchip.md) and [self-updater](shell-updates.md) publish
an immutable generation containing all four binaries and atomically switch one
`current` pointer. Apps resolve companions alongside their own executable, so an
already-running generation stays internally consistent after an update. Previous
generations remain for rollback. Third-party App Center installs and user files
are independent. Native apps cannot be uninstalled separately in App Center.
