# Rust 1.91 and dependency policy

The workspace uses edition 2024, Cargo resolver 3 and `rust-version = "1.91"`.
`rust-toolchain.toml` pins Rust **1.91.1** (including rustfmt and Clippy), so the
release baseline uses the latest 1.91 patch. Rust 1.91.0 is also checked separately to verify the declared minimum. Future
workspace packages should inherit `version`, `edition`, `rust-version` and
shared dependencies. Python-only applications do not require Rust.

The 2026-09-10 audit covers all four direct dependencies, the complete Cargo
lockfile, features, optional/build/dev dependencies, external Store code and
build/CI tooling. There are no owned pip/npm dependencies, Python SDK package,
Git Cargo dependencies, build script dependencies, or additional workspace
members. Python scripts use the standard library. The Store uses standard-library
Tk with an existing app-local runtime; do not upgrade global/device Python or
Debian packages for this release.

## Selected direct releases

| Dependency | Selected | Newest stable checked | Result |
| --- | --- | --- | --- |
| SDL2 Rust bindings | 0.38.0 | 0.38.0 | Current; `use-pkgconfig` enabled, bundled SDL disabled |
| font8x8 | 0.3.1 | 0.3.1 | Current; only Unicode tables enabled, defaults disabled |
| serde_json | 1.0.151 | 1.0.151 | Current; standard defaults, bounded JSON input |
| png | 0.17.16 | 0.18.1 | Explicit exception below |

Sources: [SDL2](https://docs.rs/crate/sdl2/0.38.0),
[font8x8](https://docs.rs/crate/font8x8/0.3.1),
[serde_json](https://docs.rs/crate/serde_json/1.0.151),
[PNG](https://docs.rs/crate/png/0.18.1).
The registry index was refreshed with Cargo, then every non-yanked stable
version was compared with every locked registry package. `cargo outdated` was
also run; its workspace-inherited direct-dependency output alone misses the
exact PNG pin, so an "all up to date" result is not sufficient evidence.

## Exact exceptions and migration blockers

| Dependency | Selected | Newest | Concrete blocker |
| --- | --- | --- | --- |
| png | 0.17.16 | 0.18.1 | 0.18 requires bitflags 2 while the newest SDL2 requires bitflags 1. The migrated decoder compiled (seekable input plus checked optional output size), but strict `clippy::multiple_crate_versions` rejected the resulting graph. No compatible upstream SDL2 release exists to unify these dependencies. |
| bitflags | 1.3.2 | 2.13.2 | Required by SDL2 0.38.0 and the retained PNG release; cannot substitute 2.x through a lockfile update. |
| flate2 | 1.1.9 | 1.1.10 | 1.1.10 requires miniz_oxide 0.9 while PNG directly requires 0.8. Both versions fail the same required duplicate-dependency gate. |
| miniz_oxide | 0.8.9 | 0.9.1 | PNG's direct 0.8 requirement prevents unification with 0.9. |
| version-compare | 0.1.1 | 0.2.1 | SDL2-sys 0.38.0 build dependency is constrained to 0.1. |

These are policy/upstream constraints, not Rust 1.91 compiler failures. No major
Rust dependency migration is included in the final candidate: the only newer
direct incompatible release was attempted and is explicitly deferred. This is
not a claim that every dependency is latest. Resolving these constraints needs
coordinated upstream SDL2/PNG changes or a separately reviewed dependency policy
change. SDL3 would also require replacing the device's validated native SDL2 ABI
and is outside a conservative launcher hardening pass. No vendored forks,
redundant direct dependencies, or lint suppressions were added to hide this.

The lockfile is version 4 and has no duplicate crate versions. All other locked
registry packages matched the newest stable release at review. `cargo audit`
with 1,243 loaded RustSec advisories reported no advisories or unmaintained-crate
warnings. This does not audit the device's native OS libraries or establish
trust in remote applications.

Reproduce resolution after manifest changes:

```sh
cargo update
# Reviewed exception: preserve one miniz_oxide version with the PNG constraint.
cargo update -p flate2 --precise 1.1.9
cargo outdated --workspace
cargo tree --duplicates
cargo tree --edges features
cargo audit
sh scripts/validate.sh
```

Review the regenerated lockfile; release and CI builds use `--locked`. Do not
silently remove the documented exception after a general `cargo update`.
Compression remains the pure Rust miniz backend; no native compression library,
network feature, or bundled SDL build is enabled by this candidate.

## Build tools and external code

The ARM pass upgrades the optional cargo-zigbuild tool from 0.22.1 to **0.23.4**,
installed into ignored `target/beta/tools`, and uses Zig **0.16.0**. Tool binaries
are not application dependencies. Install them on the development host, never
on PocketCHIP. The native image retains SDL **2.32.4** and its existing Python
**3.13.5**/Tk runtime; these are image compatibility inputs, not owned dependency
pins to be upgraded globally.

CI uses [actions/checkout 7.0.1](https://github.com/actions/checkout/releases/tag/v7.0.1)
at its full commit SHA, read-only repository permissions and no persisted Git
credentials. There were no previous CI action pins. Ubuntu's packaged SDL/Tk
are host test prerequisites. The workflow has been reviewed locally; a remote
Actions run is not claimed before committing/pushing.

The Store's reviewed upstream commit remains
`1f394452d6acd124d940154234b0eb8dd7150b70`, which still matched remote HEAD during
this pass. The compatibility patch and SHA-256 manifest were updated together;
previous reviewed hashes remain accepted for migration. It has no pip lockfile
or third-party Python dependency to upgrade. The patch preserves manual Store
self-update so a remote update cannot remove local safety changes.

Rust 1.91.1 includes the upstream 1.91 patch fixes; the declared MSRV remains
1.91. CI checks both 1.91.0 and 1.91.1. See the
[official Rust release notes](https://doc.rust-lang.org/stable/releases.html#version-1911-2025-11-10).
