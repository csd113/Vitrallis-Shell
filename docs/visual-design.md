# Vitrallis visual design

Everyday screens retain their existing icons, controls, routes, keyboard actions,
and layout, with **Manage [F10]** aligned to the rightmost tile and no grid separator
lines. The crystal logo and environment are reserved for startup. No crystal
marks are added to the home header, Settings, App Center, or bundled app icons.

## Shared interface theme

`crates/vitrallis-native/src/theme.rs` owns the near-black/navy backgrounds, dark
blue surfaces, thin blue borders, lavender text, muted light-blue labels, and
cyan/blue/violet emphasis. Pink identifies warnings and destructive actions.
The shared card primitive keeps keyboard focus visible even when an action is
unavailable: focus changes the fill and the one-pixel border colour only, with
nothing drawn inside the rectangle, so no corner or edge mark can read as a stray
line. Progress bars use one continuous cyan/blue/violet ramp that belongs to the
track, so a pixel keeps its colour as the value changes. Sliders share that ramp
at reduced intensity and add a solid thumb that brightens with keyboard focus,
which keeps inactive track, active track and handle distinguishable. Neither
primitive animates and neither allocates per frame.

The Shell's existing renderer delegates to those primitives. Settings, wireless,
storage, updates, App Center, desktop dialogs and bundled native application
controls use the same palette. Body text, secondary labels, disabled text and
focus colors are checked for a minimum 4.5:1 contrast ratio against the interface
surfaces.

## Text layout and overflow

`src/renderer.rs` owns the one text measurement for the Shell. The atlas is a
fixed 8-pixel cell and the theme names the 12-pixel text line, spacing and border
metrics, so `advance(scale)` and `fit_columns(width, scale)` are exact
multiplications with no font queries and no allocation. Every centered label uses
`text` (glyphs centered inside the content rectangle) and every left-aligned
label uses `text_left`; both share one drawing path and one overflow policy:

- Text that does not fit its rectangle is shortened to the last whole character
  that leaves room for a three-cell ellipsis. Nothing is ever cut through a
  glyph, and the marker is the same everywhere.
- A rectangle too narrow for the marker keeps its leading characters, which is
  what the tiny arrow and badge cells need.
- Rectangles too narrow for a whole cell draw nothing rather than a dot.
- Measurement stops one character past the rectangle, so a label that cannot fit
  never scans its whole string.
- Typographic punctuation the 8-pixel atlas lacks (`’ “ — •`) maps onto the
  ASCII glyph that stands in for it, so an authored label never shows the
  unsupported-character fallback. Document and terminal content is untouched.

Content padding scales with the display text scale, while the one-pixel card
border stays one pixel at every size. A debug assertion in the shared drawing
path fails any future caller whose rectangle cannot hold the glyph cell it
centers, which is how text used to cross a row border. Measurement, ellipsis,
wrapping, chip containment and the App Center details body are covered by
deterministic unit tests in `src/renderer.rs`.

User wallpaper/color preferences remain available on the home screen. Settings
uses the clean background so wallpaper cannot obscure its information. External
application icons and existing system icons remain unchanged.

## Startup

`src/boot.rs` uses five embedded 480×272 PNG keyframes: clean, subtle, cyan sweep,
full energy, and the cave background with VITRALLIS lettering. The final reveal
fades the background behind the unchanged full-energy crystal texture, keeping
its position and scale fixed. Crystal
illumination and transitions are stepped at 12 FPS over three seconds. Rendering
uses at most two texture copies and color/alpha modulation per frame; there are
no particle simulations, blur passes or shaders.

Catalog discovery runs on a scoped worker while SDL presentation stays on the
main thread. The final scene holds until discovery finishes. Escape/Home skips
the remaining decorative sequence once discovery is ready. Other boot input is
consumed, so a startup tap or Enter cannot launch an app. Close requests are
handled; the bounded local discovery worker is joined before shutdown.

At native resolution every artwork pixel maps to one display pixel. Larger
windows use centered integer enlargement with dark borders; smaller supported
windows crop the surrounding environment while preserving the central logo and
name. All artwork uses nearest-neighbor sampling. No fractional scale or logical
renderer transform is installed, so the normal Shell hit boxes remain unchanged.

The five textures occupy 2,611,200 bytes (about 2.49 MiB) at RGBA32, plus one
bounded PNG decode working buffer during loading. Compressed assets occupy less
than 400 KiB. Textures are released on handoff and before rebuilding after a
renderer reset. A failed decorative load logs one diagnostic and presents plain
VITRALLIS text; it does not prevent discovery or Shell startup. There is no
remaining boot thread, texture cache or animation timer after handoff.

Every frame, including the initial dark frame and Shell handoff, uses the existing
`Screen::present` / `PresentationClock` boundary and renderer selection policy.
No window-surface drawing, single-buffered path or change to the device's VSync
configuration is introduced. A desktop simulator cannot establish physical
PocketCHIP scanout or ARM performance; physical follow-up results are recorded in [validation](visual-design-validation.md).

## Interface structure

Settings, the App Center and the desktop share one presentation vocabulary:

- **Settings** uses the same large two-line option for every category. The home
  menu lists eight entries (Display & Sound, Date & Time, Wireless Network,
  Applications, Storage, Device, Software Updates, About); each category lists
  its own settings. Nothing important is hidden behind a footer shortcut, and one
  Escape always returns exactly one level, with the home menu as the only exit.
- **App Center** rows show the app name, a state chip (RUNNING, UPDATE, INSTALLED,
  AVAILABLE, UNAVAILABLE, FAILED), the short description and the current
  operation. The details page leads with the name, state, description and the
  metadata a user needs, and failures are summarised in one line while the full
  backend error stays in the log. The details body is laid out inside the space
  above the pinned action row: the description yields its lines to the fields
  when a screen is too short for both, an omitted field is marked with the shared
  ellipsis, and a recorded failure keeps its line. Long operations reuse the
  shared progress ramp.
- **Running state** is one filled chip derived from the authoritative process
  state, identical in the main menu, inside folders and in the App Center list.
  Launching and exit notifications appear in the lower-left status area, never as
  a modal screen.
- **Bundled applications** reserve their vertical space for content: Terminal
  keeps one ten-pixel status line at the bottom, and Notepad uses a single-line
  header so the editor and its height-1 status row own the rest of the display.

## Reproduction and checks

Artwork preparation and asset sizes are documented in
[the branding notes](../assets/branding/README.md). Source artwork is already
packaged; building or starting the Shell needs no image tool, Python dependency,
network access or external assets.

The existing multi-size renderer fixture now captures startup stages and its
fallback alongside the Settings/App Center scenes. It checks 320×200, 480×272,
800×480 and 1280×720. Run with a new empty output directory:

```sh
mkdir -p target/visual-qa
VITRALLIS_QA_DIR="$PWD/target/visual-qa" cargo test --lib system_panels_render_at_device_and_scaled_sizes
```

The checked pixel hashes are reviewed snapshots, not generated automatically by
normal tests. The current `macos` block was regenerated for the text-geometry
audit pass, which also added deterministic `home-folder`, `home-error`,
`app-center-apps-long` and `app-center-details-long` samples (widest plausible
catalogue content, folder contents and the error dialog) at the same four sizes.
The `linux` block was removed with the same change, so Linux runs skip the
comparison until the documented command above is run on the simulator and the
new screenshots are reviewed; that re-baseline is still pending and must happen
before the next release validation. Boot lifecycle coverage exercises decoder
rejection, renderer reset, Escape handoff, close requests and discovery failure.
The full host gates are `sh scripts/validate.sh`; Linux interactive checks use the
existing Docker simulator described in [its README](../tests/simulator/README.md).
