# System artwork

Seven 128×128 RGBA PNG icons are embedded in the shell: `gear`, `wifi`, `sun`,
`speaker`, `power`, `restart`, and `apps`. Their transparent edges and simple
silhouettes are intended for the dark system panels at small sizes.

The renderer loads each icon once and uses linear filtering. These files require
no runtime downloads. Preserve alpha, dimensions and legibility when replacing
artwork; check the actual 480×272 and 800×480 screens with visible keyboard focus.

The seven icons were generated with an AI image tool at the project owner's
request on 2026-09-10 and 2026-09-11, one asset at a time, then visually
inspected and exported as 128×128 RGBA PNGs. The generation record and the
complete prompts remain in commits `7895a1c` (six icons) and `718f4e6`
(`apps.png`). The tool's output and redistribution terms were not recorded, so
these files are **not covered by the project's MIT grant** until that basis is
documented or the artwork is replaced.

See [artwork provenance](../PROVENANCE.md) and
[licensing and third-party notices](../../THIRD_PARTY_NOTICES.md) before redistribution.
