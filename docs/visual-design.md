# Vitrallis visual design

Everyday screens retain their existing icons, controls, routes, keyboard actions,
and layout, with Actions aligned to the rightmost tile and no grid separator
lines. The crystal logo and environment are reserved for startup. No crystal
marks are added to the home header, Settings, App Manager, or bundled app icons.

## Shared interface theme

`crates/vitrallis-native/src/theme.rs` owns the near-black/navy backgrounds, dark
blue surfaces, thin blue borders, lavender text, muted light-blue labels, and
cyan/blue/violet emphasis. Pink identifies warnings and destructive actions.
The shared card primitive keeps keyboard focus visible even when an action is
unavailable. Progress bars use three solid color segments, with warning capacity
bars retaining one alert color. Neither primitive animates.

The Shell's existing renderer delegates to those primitives. Settings, wireless,
storage, updates, App Manager, desktop dialogs and bundled native application
controls use the same palette. Existing text sizing and geometry are preserved;
the theme names the 8-pixel cell, 12-pixel text line, spacing and border metrics.
Body text, secondary labels, disabled text and focus colors are checked for a
minimum 4.5:1 contrast ratio against the interface surfaces.

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

## Reproduction and checks

Artwork preparation and asset sizes are documented in
[the branding notes](../assets/branding/README.md). Source artwork is already
packaged; building or starting the Shell needs no image tool, Python dependency,
network access or external assets.

The existing multi-size renderer fixture now captures startup stages and its
fallback alongside the Settings/App Manager scenes. It checks 320×200, 480×272,
800×480 and 1280×720. Run with a new empty output directory:

```sh
mkdir -p target/visual-qa
VITRALLIS_QA_DIR="$PWD/target/visual-qa" cargo test --lib system_panels_render_at_device_and_scaled_sizes
```

The checked pixel hashes are reviewed snapshots, not generated automatically by
normal tests. Boot lifecycle coverage exercises decoder rejection, renderer reset,
Escape handoff, close requests and discovery failure. The full host gates are
`sh scripts/validate.sh`; Linux interactive checks use the existing Docker
simulator described in [its README](../tests/simulator/README.md).
