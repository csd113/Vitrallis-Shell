# Licensing and third-party notices

Audit date: **2026-09-24**. Project-owned code, documentation and original artwork
are offered under the root [MIT License](LICENSE), at the owner's direction.
This grants no rights to third-party material or the unresolved items below.
Copyright remains with the respective contributors; no assignment is implied.

[Dependency inventory](docs/dependency-licenses.md) records exact cached crate
versions, declared license expressions and gaps. [Verbatim license and notice
texts](THIRD_PARTY_LICENSES.txt) preserve upstream attributions, including nested
notices. Keep this file, those texts and LICENSE with source distributions and
applicable binary distribution documentation. An MIT/Apache alternative permits
choosing MIT; an AND expression requires both terms. Apache-only components retain
their license and applicable NOTICE/attribution requirements. These files do not
relicense dependencies or certify that existing artifacts contain their notices.

The inventory is deliberately conservative: it also covers optional, build-only
and other-target crates from upstream lockfiles. Those entries are not part of the
shipped binaries and do not by themselves add binary distribution obligations.
Any gap for a crate that *is* linked into a distributed binary still needs its
text or a documented source offer before that binary is cleared.

## Components and assets

- Rust workspace: SDL2 bindings, font8x8 bitmap tables and all locked transitive
  crates are inventoried. font8x8's upstream MIT copyright is retained in the
  collected texts; no external font file is installed by Shell.
- SDL2 is dynamically supplied by the OS through pkg-config, not built with the
  bundled SDL feature. The sdl2-sys crate also carries SDL's zlib terms. Preserve
  those notices if redistributing its sources or a runtime library.
- Arti **2.6.0** is a separate bundled executable, built by
  `scripts/build-arti.sh` with the pinned published feature set. Its metadata
  declares MIT OR Apache-2.0; the MIT text at the exact source revision is
  included, because the registry package omits the top-level license texts.
  The binary links only that pinned feature graph; the conservative inventory
  also lists the crate's wider upstream lockfile, including optional crates that
  are not built (for example `equix`/`hashx` and `dynasm` tooling). Where a
  dependency offers a choice, this project relies on the permissive option:
  **MIT** for MIT/Apache-2.0 crates, **MPL-2.0** for the dual LGPL-3.0-or-later OR
  MPL-2.0 `priority-queue` (a dev/test-only crate in Arti's tooling; its text is
  not part of the collected inventory), with `option-ext` (MPL-2.0) and `ring`
  (Apache-2.0 AND ISC) texts retained. `libsqlite3-sys` bundles SQLite (public
  domain), `zstd-sys` bundles Zstandard 1.5.7 (BSD-3-Clause) and `liblzma-sys`
  bundles XZ Utils liblzma (0BSD); their notices are preserved. A complete
  binary notice set still requires resolving any missing text for a crate that is
  actually linked.
- Python, Tk, Bubblewrap, picom, graphics drivers and optional FFmpeg are system
  packages, not payloads in the native bundle. Installation through the system
  package manager does not relicense them. A redistributed OS image must retain
  its package copyrights and satisfy any GPL/LGPL source/relinking obligations.
- `assets/native/` documents original geometric SVG/PNG utility icons, covered
  by project MIT. Screenshots are dated evidence, not a license for depicted
  third-party programs, trademarks or content.

## Unresolved provenance and distribution limits

- `assets/branding/` and `assets/boot/` derive from supplied logo/cave references.
  The preparation record documents image-tool transformations, but not the
  original reference creator, license or redistribution permission. MIT does
  not cover those references or establish rights to their derivatives.
- `assets/system/*.png` has no recorded creator/source/license in its README.
  Redistribution rights remain unverified.
- These artwork files are embedded in Shell. Therefore the current complete
  binary cannot be represented as wholly cleared for redistribution until their
  rights are documented or the assets are replaced with verified material.
- Existing release inventories list bundles/helpers and checksums, not these new
  license companions. The package format is unchanged by this documentation task.
  Distributors must supply the applicable texts beside binaries and resolve Arti
  gaps; this source change does not repair already published artifacts.

No separate GPL-linked component was identified in the Shell workspace graph or
in Arti's pinned build graph. This does not settle licensing of OS images,
optional tools or unverified assets.
