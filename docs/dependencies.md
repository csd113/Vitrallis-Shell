# Rust 1.91 and dependency policy

The workspace uses edition 2024, Cargo resolver 3 and `rust-version = "1.91"`.
`rust-toolchain.toml` pins Rust **1.91.1** with rustfmt and Clippy. CI also checks
1.91.0 to verify the declared minimum. Workspace packages inherit `version`,
`edition`, `rust-version` and shared dependencies. The dependency refresh does
not change the Vitrallis release version, compiler baseline or device ABI.

## Dependency review: 2026-09-19

All ten direct registry dependencies use their newest non-yanked stable release.
The manifest records those versions as minimum compatible requirements; the
lockfile fixes the exact resolved graph for release and CI builds.

| Dependency | Selected stable release | Features / purpose |
| --- | --- | --- |
| sdl2 | 0.38.0 | `use-pkgconfig`; no bundled SDL build |
| font8x8 | 0.3.1 | Unicode tables only; defaults disabled |
| libc | 0.2.189 | POSIX boundary in the shared native crate and Terminal |
| vt100 | 0.16.2 | ANSI/VT terminal state |
| serde_json | 1.0.151 | Bounded JSON input |
| serde | 1.0.229 | Recursive duplicate-key-rejecting JSON visitor |
| toml | 1.1.6+spec-1.1.0 | `std`, `parse`, `serde`; defaults disabled |
| semver | 1.0.28 | Standard version precedence |
| sha2 | 0.11.0 | Streaming SHA-256; defaults disabled |
| png | 0.18.1 | Bounded icon decoding and validation |

Versions were verified against the live crates.io index, including all **51**
locked registry package entries. `cargo upgrade` also checked incompatible and
pinned releases with the Rust-version filter disabled. `cargo outdated
--workspace` reports no upgrades, but that result alone does not expose every
upstream transitive constraint.

PNG was upgraded from 0.17.16 to 0.18.1 and flate2 from 1.1.9 to 1.1.10. The PNG
0.18 API requires seekable buffered input and returns an optional output buffer
size. All three decoder paths handle an unrepresentable size as an error before
allocation; existing image dimensions and decoder memory limits remain enforced.
Regression coverage checks RGB, grayscale, grayscale/alpha, 16-bit RGBA, palette
transparency, malformed input and oversized assets.

Sources: [crates.io index](https://index.crates.io/config.json),
[PNG 0.18.1 reader API](https://docs.rs/png/0.18.1/png/struct.Reader.html),
[SDL2 0.38.0](https://docs.rs/crate/sdl2/0.38.0),
[flate2 1.1.10](https://docs.rs/crate/flate2/1.1.10).

## Upstream transitive constraints

| Dependency retained | Newest stable | Required by |
| --- | --- | --- |
| bitflags 1.3.2 | 2.13.2 | SDL2 0.38.0 requires 1.x; PNG uses the latest 2.x alongside it |
| miniz_oxide 0.8.9 | 0.9.1 | PNG 0.18.1 requires 0.8; flate2 uses the latest 0.9 alongside it |
| version-compare 0.1.1 | 0.2.1 | SDL2-sys 0.38.0 has a 0.1 build dependency |

These are the only locked registry entries below the newest stable release.
Cargo cannot replace them with incompatible versions without upstream changes.
SDL3 would change the validated native SDL2 ABI and is a separate migration.

`clippy.toml` allows duplicate versions only for **bitflags** and **miniz_oxide**,
using Clippy's [allowed-duplicate-crates configuration](https://doc.rust-lang.org/clippy/lint_configuration.html#allowed-duplicate-crates).
This replaces the old PNG/flate2 downgrade policy so direct dependencies can stay
current. The required strict Clippy command remains unchanged, and duplicate
versions of other crates still fail validation. Remove each named exception when
upstream requirements converge; do not add forks or unused direct dependencies
to force incompatible transitive upgrades.

Compression retains the pure Rust miniz backend. `zlib-rs` 0.6.8 appears in the
lockfile through an optional PNG/flate2 feature, but that feature is not enabled
by this workspace. There is no new native compression library requirement.
The refreshed lockfile passed `cargo audit` with **1,251** loaded RustSec
advisories and no reported advisories or unmaintained-crate warnings. This does
not audit native OS libraries or establish trust in remote applications.

Reproduce the review after manifest changes:

```sh
cargo upgrade --dry-run --incompatible allow --pinned allow --ignore-rust-version
cargo update
cargo outdated --workspace
cargo tree --workspace --duplicates
cargo tree --workspace --edges features
cargo audit
sh scripts/validate.sh
```

Do not restore the superseded PNG or flate2 pins. Review the regenerated
lockfile and upstream requirements; release and CI builds use `--locked`.

## Build tools and external code

There are no owned pip/npm dependency manifests, Git Cargo dependencies or
third-party build-script dependencies. Python scripts use the standard library.
Device and simulator Python/Tk, SDL2, Debian packages and optional host
cargo-zigbuild/Zig installations are environment prerequisites, not owned Cargo
dependency pins. This refresh does not upgrade global or device packages.

CI's only external action, [actions/checkout 7.0.1](https://github.com/actions/checkout/releases/tag/v7.0.1),
was verified as the latest release and remains pinned to its full commit SHA.
Repository permissions and disabled credential persistence are unchanged. The
release container remains Debian 12 with Rust 1.91.1 to preserve the release ABI.

## Application dependency boundaries

The updater and App Center share semver, SHA-256 and PNG validation. Networking
uses optional system `/usr/bin/curl`, avoiding an HTTP/TLS dependency stack.
App Center parses the complete manifest v1 TOML grammar, rejects duplicate keys
and validates types. Runtime Python is probed only for applications that require
it, using system Python or an app-local virtual environment.

`vitrallis-native` shares SDL2, font8x8 and SHA-256 for bounded Notepad save-conflict
checks. Native applications add no GUI framework, async runtime, HTTP stack,
Python runtime or embedded server. The shell retains `unsafe_code = "forbid"`.
The shared crate and Terminal use a small documented libc boundary for exclusive
rename/no-follow flags, socket ownership, PTYs, polling and controlling terminals.
An owned child guard kills/reaps on early failure and shutdown; the post-fork
callback performs only documented async-signal-safe calls.

Terminal's vt100 parser uses vte 0.15.0, unicode-width 0.2.2 and arrayvec 0.7.8.
The adapter caps control strings at 4 KiB and discards excess through their
terminator. Replies, scrollback, geometry and PTY queues have separate bounds.
The upstream parser remains part of the dependency trust boundary. The focused
POSIX adapter avoids the additional process abstractions and dependencies of
portable-pty. No native library beyond SDL2 and POSIX libc/libutil is required.

Resource measurements are recorded in
[native validation](devices/pocketchip/history/native-validation.md).
