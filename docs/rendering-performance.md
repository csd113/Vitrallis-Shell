# SDL rendering performance

## Phase 2 baseline

Baseline source: `c4c6d20467e28126ab00ebbf93679bb6d5d79be1` (Phase 1).
Measurements use the release-profile test harness, SDL dummy/software, 480x272,
200 warm frames per workload. They measure CPU submission plus software rasterization
and presentation, not physical Mali-400 performance or power. The local host is
macOS; PocketCHIP and Raspberry Pi measurements are not available in this session.

Before changing rendering architecture:

- Shell text issues one SDL fill per lit bitmap pixel, including spaces in layout.
- Native text batches points at scale 1 but issues one fill per lit pixel when scaled.
- App Center creates/uploads/destroys a 32x32 RGBA texture for every visible icon every frame.
- Launcher icons persist between frames with a 16 MiB retained-artwork budget.
  Catalogue changes recreate all icons, wallpaper and system artwork together.
- The shell waits for events, with bounded child/system polling. Settings and App
  Center currently mark every delivered event dirty, including irrelevant motion.
- Native applications wait for events; modal loops redraw on ignored events.
- Device reset events do not currently repopulate static textures.

The existing deterministic QA suite was captured before optimization at 320x200,
480x272, 800x480 and 1280x720 in `target/renderer-phase2/baseline/`.
The opt-in workload harness and raw baseline output are retained locally in
`target/renderer-phase2/baseline-workloads.txt`.

## Implementation

Phase 1's verified SDL renderer selection is unchanged: auto tries accelerated
backends (GLES2 first), then software; forced hardware reports initialization
failure rather than silently selecting software. Both paths call the same UI.
All GPU work uses safe SDL canvas/texture APIs. The Mali-400/GLES2 floor rules
out relying on GLES3, compute, Vulkan, modern texture formats, or a render thread.
RGBA textures, integer source/destination rectangles, blending, and nearest/linear
sampling are sufficient here.

The shared font atlas contains BASIC (128), LATIN (96), and BOX (128) font8x8
glyphs, arranged in 16 columns in a 128x176 RGBA texture: **88 KiB per renderer**.
Its temporary pixel buffer is freed after upload. Each visible nonblank character
uses one SDL copy; blank glyphs still advance layout. Unsupported characters use
`?`. The shell retains BASIC-only coverage while native applications retain all
three ranges. Integer scaling, centering, truncation, and SDL clipping are unchanged.
Color modulation changes only when needed. Nearest sampling keeps text crisp.
The per-pixel implementation remains only as a test oracle for all supported
glyphs, clipping, colors, and scales 1–3.

`Screen` owns the shell canvas, atlas and App Center cache; native `Session` owns
initialization and `Ui` borrows its texture creator. The creator outlives every
texture. Initialization retries finish before textures exist. Normal window size
changes recompute layout and preserve textures; device resets rebuild the atlas,
clear App Center textures and rebuild shell artwork. Render-target resets only
invalidate presentation because this implementation has no target textures.
Returning from another application raises/repaints without rebuilding static assets.

Launcher artwork refresh hashes one bounded encoded source at a time with the
existing SHA-256 dependency. Content plus filtering mode identifies reusable
textures across reorder, selection, rename, and catalogue changes. Failed image
decodes are remembered until source changes; budget misses retry when space is
available. Unrelated wallpaper/system textures survive catalogue changes. Source
reads/hashing happen on refresh, never ordinary rendering. Changed bytes are
checked again before upload. Obsolete allocations are released before replacements.
Decoded surfaces and decode buffers are freed after upload. PNG/BMP size, file
and resource protections remain: images are at most 512x512 and 1 MiB encoded;
PNG decoding retains its 8 MiB decoder limit. Icons retain the **16 MiB** budget,
checked before upload. Wallpaper is at most 1 MiB decoded, and system assets form
a fixed additional set. Cache metadata scales with the already bounded catalogue.

App Center's predecoded 32x32 icons use a FIFO cache of at most 128 distinct pixel
arrays. This adds at most **512 KiB texture storage and 512 KiB CPU keys**, excluding
SDL metadata. Cache hits do no allocation, upload or decoding. Navigating beyond
128 distinct images may evict and later re-upload an image; CPU decoding remains
in the existing bounded catalogue/cache loading path. Icons use their existing
filtering policy: launcher/system artwork linear; wallpaper and App Center nearest.
The font and cache add under 1.1 MiB of explicit retained pixel storage per shell.
Driver-internal memory and framebuffer storage are not measured by these bounds.

Filtering is set once at creation and included in artwork identity. The Linux
simulator exposed GLES2 texture-binding corruption when repeatedly setting scale
mode on retained textures. Removing those redundant changes fixes the full
lifecycle and the dedicated accelerated refresh regression. SDL's
[GLES2 implementation](https://raw.githubusercontent.com/libsdl-org/SDL/release-2.26.5/src/render/opengles2/SDL_render_gles2.c)
provides the backend context for that test.

The shell retains 16 ms dirty-frame pacing and 250 ms bounded polling. Identical
system status no longer schedules a frame; irrelevant motion/queue events do not
redraw App Center or desktop panels. Settings compares slider selection/preview
while preserving drag handling. Native waits consume ignored events without
repainting modal dialogs. There is no idle render loop or new render thread.
Zero-area fills are skipped and redundant fill colors avoided. Full-screen redraws
remain appropriate when dirty; no retained scene graph or regional damage system
was introduced.

## Measurements

[Raw workload measurements](renderer-workloads.json) preserve all sampled screens.

The following local samples are **not device GPU benchmarks**. Each row is 200
warm frames in a release-profile test binary, macOS SDL dummy/software, including
software rasterization and present but excluding initial asset loading and screenshot
I/O. Test-only counters add some overhead, especially to the old per-pixel path;
wall times are illustrative single samples, not acceptance thresholds. Baseline
runs use an isolated copy of the Phase 1 commit with the same test harness.
The original baseline was captured before migration; extra icon/wallpaper/input
workloads were subsequently run against that unchanged isolated implementation.

| Workload | Baseline ms/frame | Final ms/frame | Text SDL operations/frame, before → after |
| --- | ---: | ---: | ---: |
| home | 0.553 | 0.103 | 2543 → 125 |
| settings | 0.411 | 0.085 | 2275 → 118 |
| modal | 0.509 | 0.098 | 3276 → 163 |
| app-center-update-badge | 0.751 | 0.109 | 4793 → 240 |
| desktop-editor | 0.663 | 0.088 | 4256 → 215 |
| many-icons | 0.414 | 0.089 | 2459 → 119 |
| wallpaper | 0.693 | 0.373 | 2543 → 125 |
| rapid-keyboard | 0.407 | 0.071 | 2543 → 125 |
| rapid-pointer | 0.484 | 0.083 | 3112.68 → 154.165 |

Home text calls fall from 2,543 to 125 per frame (95.1% fewer); the text-heavy
App Center update screen falls from 4,793 to 240 (95.0% fewer). These count SDL
API submissions, not actual GPU draw calls after SDL batching. A separate repeated
App Center icon workload makes 200 texture uploads before and zero after warm-up;
its measured total falls from 1.490 ms to 0.162 ms. Every warm screen workload
records zero static decodes and zero uploads. The many-icon workload contains
1,000 actual icon references while rendering only the visible page. Rapid input
workloads exercise selection actions; real keyboard/mouse/touch delivery is tested
by the simulator rather than inferred from those CPU timings.

The two-second real event-loop observation records two startup/exposure frames,
all within the first 500 ms, and zero subsequent idle frames. This proves bounded
idle redraw behavior locally, not physical power consumption. PocketCHIP Mali-400
and Raspberry Pi vc4/V3D hardware were not available during Phase 2.
The later [Phase 3 physical record](devices/pocketchip/graphics-phase3.md) covers
PocketCHIP only; both Raspberry Pi physical validations remain pending. Their input latency,
frame-time tails, driver reset behavior, memory/RSS, thermal and battery effects,
and sustained navigation with maximum artwork still require hardware measurement.

## Validation and reproduction

```sh
sh scripts/validate.sh
cargo test --release --lib -p vitrallis-shell rendering_workloads -- --ignored --nocapture
cargo test --lib -p vitrallis-shell idle_loop_stops_after_startup -- --ignored --nocapture
# In a graphical session with a real accelerated backend:
cargo test --lib -p vitrallis-shell accelerated_atlas_survives_unchanged_artwork_refresh -- --ignored
cargo test --test desktop accelerated_ -- --ignored
VITRALLIS_RENDERER_BIN_DIR="$PWD/target/release" VITRALLIS_TEST_ACCELERATED=1 python3 -m unittest discover -s tests -p 'test_native_renderer.py'
```

The performance harness is compiled only for tests. Optionally set
`VITRALLIS_PERF_QA_DIR` to an existing empty directory for workload BMPs;
`VITRALLIS_QA_DIR` captures the existing system QA suite. Screenshots refuse
replacement. The checked-in Phase 1 hashes assert 123 exact software screenshots
at four sizes on each of macOS and Linux. They are platform-specific because SDL
image filtering already differed slightly before migration. New platforms still
run the complete glyph pixel oracle and rendering tests; platform screenshot
references should be captured from a reviewed build. Version labels are intentionally
part of the references, so an explicitly authorized future version change requires
reviewing regenerated images/hashes.

Additional local comparisons matched 20 home/wallpaper/catalogue/navigation states
and nine native screenshots byte for byte against Phase 1. Cache tests cover
source changes, reorder, failed decodes, budget exhaustion/recovery, invalid pixels,
FIFO bounds and reset invalidation. Native glyph/reset parity covers all supported
characters, integer scales, clipping and color changes. The Linux simulator runs
real HTTPS App Center install/update/remove flows, Awesome session restoration,
keyboard/mouse/touch shortcuts, and GLES2 hardware/auto/software readback. See
[simulator instructions](../tests/simulator/README.md) for running the same suite.

Raw local logs and screenshots live under `target/renderer-phase2/` (ignored).
The published third-party network variant is optional; the complete deterministic
fixture lifecycle, smoke, integration, screenshot and host gates are the required
local checks. No version numbers, dependencies or commits were added.

### Recorded results

The final host `scripts/validate.sh` completed successfully: formatting, locked
workspace check, strict Clippy (all/pedantic/nursery/cargo denied), workspace tests
with all features, 83 Python tests (two connector/display-gated skips), release
builds, native readback, shell/native smoke tests, shell/Python syntax, documentation
links and diff whitespace. The separately enabled accelerated tests passed on
macOS and Linux, covering the display-gated native checks. Linux additionally
passed both accelerated desktop tests including fullscreen dimensions, the atlas
refresh test, stock-session integration, all deterministic App Center lifecycle
scenarios, and the shortcut keyboard/mouse/touch simulator. The final focused
cache test also verifies complete static-artwork reconstruction after reset.

The opt-in idle observation passed with two initial frames over 2.005 seconds;
no frame occurred after the first 500 ms. This is a short deterministic idle check,
not a long-term system polling or battery benchmark. Exact physical-device
acceptance remains the hardware work listed above.
