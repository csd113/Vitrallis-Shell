# Licensing and third-party notices

Audit refreshed: **2026-10-04**. Project-owned code, documentation and original artwork
are offered under the root [MIT License](LICENSE), at the owner's direction.
This grants no rights to third-party material outside the project-owned artwork described below.
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
- Arti **2.7.0** is a separate bundled executable, built by
  `scripts/build-arti.sh` with the pinned published feature set. Its metadata
  declares MIT OR Apache-2.0; the MIT text at the exact source revision is
  included, because the registry package omits the top-level license texts.
  The binary links only that pinned feature graph; the conservative inventory
  also lists the crate's wider upstream lockfile, including optional crates that
  are not built (for example `equix`/`hashx` and `dynasm` tooling). Where a
  dependency offers a choice, this project relies on the permissive option:
  **MIT** for MIT/Apache-2.0 crates, **MPL-2.0** for the dual LGPL-3.0-or-later OR
  MPL-2.0 `priority-queue`, with `option-ext` (MPL-2.0) and `ring`
  (Apache-2.0 AND ISC) texts retained. `libsqlite3-sys` bundles SQLite (public
  domain), `zstd-sys` bundles Zstandard 1.5.7 (BSD-3-Clause) and `liblzma-sys`
  bundles XZ Utils liblzma (0BSD); their notices are preserved. A complete
  binary notice set still requires resolving any missing text for a crate that is
  actually linked.
  The 1.0.3 refresh verifies all 426 selected ARMv7 normal/build packages and
  records 106 new or updated inventory rows. The Tor-family MIT text is verified
  at registry source `ce8bc6e0998bd5a4efdf06dd62dce53c98ea1087`; derive-deftly's
  MIT text is verified at `4ef993f28240288732ff5b4e0a2803b811e7a30d`.
  Historical inventory rows and notices remain for retained older artifacts.
  The selected ARMv7 normal/build dependency graph includes `priority-queue`
  2.7.0; its MPL text and immutable upstream source location are retained in the
  collected notices. The 2026-10-02 reconciliation also adds missing cookie-factory
  and void MIT notices and rustix-linux-procfs attribution, and matches shared
  Apache/MIT texts to their exact source files. Seven Tor helper crates share
  the MIT text verified at their exact registry VCS revision. The matching Rust
  1.99.0 binary runtime inventory covers the standard library, its 10 external
  normal/build dependencies, compiler-builtins' LLVM exception, Unicode data,
  and in-tree math/channel/backtrace attributions. Exact texts and per-file
  notice excerpts are retained; see [the runtime inventory](docs/rust-runtime-licenses.md).
- Python, Tk, Bubblewrap, picom, graphics drivers and optional FFmpeg are system
  packages, not payloads in the native bundle. Installation through the system
  package manager does not relicense them. A redistributed OS image must retain
  its package copyrights and satisfy any GPL/LGPL source/relinking obligations.
- `assets/native/` documents original geometric SVG/PNG utility icons, covered
  by project MIT. Screenshots are dated evidence, not a license for depicted
  third-party programs, trademarks or content. The per-file record for every
  bundled image is [artwork provenance](assets/PROVENANCE.md).

## Artwork provenance and distribution

The authoritative per-file record is [artwork provenance](assets/PROVENANCE.md).
On 2026-10-02 the project owner confirmed ownership of the logo, storyboard and
cave references and identified all of those references and the system icons as
ChatGPT-generated artwork commissioned for the project. The project MIT grant
includes the generated masters and their derivatives. The provenance record
links OpenAI's output terms, introducing commits, preparation prompts and hashes.
The original reference sheets and exact image model/version were not retained.

`assets/{boot,system,native}/**` are embedded in the Shell executables.
`assets/branding/**` ships in source archives; its only code reference is
test-only. These assets are covered by the recorded project grant. This does not
relicense external programs depicted in screenshots or third-party dependencies.

- Existing release inventories list bundles/helpers and checksums. The package
  format is unchanged by this documentation task, but packaging now stages the
  project `LICENSE`, this file and the collected license texts beside the
  x86_64 bundles so published releases carry the required legal companions.
  Previously published artifacts retain their original bytes and notice sets.

No separate GPL-linked component was identified in the Shell workspace graph or
in Arti's pinned build graph. This does not settle licensing of OS images,
optional tools or unverified assets.
