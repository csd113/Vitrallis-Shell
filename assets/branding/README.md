# Crystal artwork

The supplied Vitrallis logo sheet, boot storyboard and cave scene are the visual
references. The crystal imagery is used only during boot. Ordinary interface
icons and headings keep their established identity and receive theme colors.

- `source/clean.png`, `source/subtle.png`, `source/full.png`: aligned 192×163 RGBA
  exports, keeping the same crystal placement across the three effects levels.
- `source/small.png`: a separately simplified 64×64 facet master for small marks.
- `crystal-32.png`, `crystal-24.png`: small-master exports, without glow.
- `crystal-16.png`, `crystal-8.png`: individually aligned pixel facets and V
  silhouettes defined in the preparation script. They do not shrink the large
  artwork. These prepared sizes are available for future approved branding uses;
  they are not substituted for application or Settings icons.
- `crystal-64.png`, `crystal-128.png`: clean detailed cluster exports.
- `../boot/*.png`: five native 480×272 playback layers. `scene.png` is the
  supplied cave artwork with the central crystal and energy removed using the
  built-in image tool, preserving the environment and lettering. The player
  keeps the full crystal layer fixed above this background during its reveal.
  Packing uses nearest-neighbor sampling, with no added branding copy.

`python3 scripts/prepare-branding.py` deterministically repacks the reviewed
masters, builds the two tiny marks and composes the four transparent isolated boot layers.
Pillow is an optional maintainer dependency only. The checked-in PNGs are embedded
by Rust; no build-time or runtime dependency is added. Preserve hard pixel edges,
alpha and integer display scaling when replacing an asset.

## Preparation record

The three transparent cluster exports and simplified small master were prepared
with the built-in image tool, using the supplied logo sheet as the visual
reference. The runtime does not call that tool. The prompts requested:

1. Extract only the leftmost CLEAN crystal V cluster, preserving the two
   diverging blue/violet spires, pale lavender/pink tips, small central/side
   crystals and rubble. Transparent background; no glow, stars, arcs, labels or
   scene. Preserve the original faceted pixel-art identity.
2. Keep the clean cluster's exact silhouette, placement and facets; add cyan/blue
   and violet/purple energy arcs and sparse pixel stars matching the upper-right
   FULL EFFECT reference. Transparent background; no text, scene or other objects.
3. Keep the same cluster; add only three small lavender/cyan glints and restrained
   tip highlights matching SUBTLE GLOW. No energy arcs, labels or background.
4. Build an intentional 32-pixel logical icon using the clean reference, a small
   navy/blue/cyan/violet/lavender palette, large legible facets, an open V, small
   center/side crystals and minimal rubble. No glow, stars, border or text.

The final packing uses nearest-neighbor sampling. Small exports apply a hard
alpha cutoff; no runtime resampling or glow is used at tiny sizes.

The supplied logo sheet, boot storyboard and cave scene were provided without a
recorded creator, supplier or license; the originals are not in this repository.
`source/*.png` and `../boot/scene.png` are image-tool derivatives of that
material, and the `crystal-*` and `../boot/{clean,subtle,cyan,full}.png` files
are regenerated from it. None of these files is covered by the project's MIT
grant until a dated permission statement or a replacement is recorded.

See [artwork provenance](../PROVENANCE.md) and
[licensing and third-party notices](../../THIRD_PARTY_NOTICES.md) before redistribution.
