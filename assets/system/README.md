# System artwork

Seven 128×128 RGBA PNG icons are embedded in the shell: `gear`, `wifi`, `sun`,
`speaker`, `power`, `restart`, and `apps`. Their transparent edges and simple
silhouettes are intended for the dark system panels at small sizes.

The renderer loads each icon once and uses linear filtering. These files require
no runtime downloads. Preserve alpha, dimensions and legibility when replacing
artwork; check the actual 480×272 and 800×480 screens with visible keyboard focus.
