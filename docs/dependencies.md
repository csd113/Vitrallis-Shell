# Rust 1.91 and dependency policy

The workspace uses edition 2024, Cargo resolver 3 and `rust-version = "1.91"`.
`rust-toolchain.toml` pins Rust **1.91.1** (including rustfmt and Clippy), so the
release baseline uses the latest 1.91 patch. Rust 1.91.0 is also checked separately to verify the declared minimum. Future
workspace packages should inherit `version`, `edition`, `rust-version` and
shared dependencies. Python-only applications do not require Rust.

The historical 2026-09-10 audit below covered all four direct dependencies, the complete Cargo
lockfile, features, optional/build/dev dependencies, external Store code and
build/CI tooling. There are no owned pip/npm dependencies, Python SDK package,
Git Cargo dependencies, build script dependencies, at that time. The native-application additions below supersede that workspace inventory. Python scripts use the standard library. The Store uses standard-library
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
on the target device. The native image retains SDL **2.32.4** and its existing Python
**3.13.5**/Tk runtime; these are image compatibility inputs, not owned dependency
pins to be upgraded globally.

CI uses [actions/checkout 7.0.1](https://github.com/actions/checkout/releases/tag/v7.0.1)
at its full commit SHA, read-only repository permissions and no persisted Git
credentials. There were no previous CI action pins. Ubuntu's packaged SDL/Tk
are host test prerequisites. The workflow has been reviewed locally; a remote
Actions run is not claimed before committing/pushing.

Rust 1.91.1 includes the upstream 1.91 patch fixes; the declared MSRV remains
1.91. CI checks both 1.91.0 and 1.91.1. See the
[official Rust release notes](https://doc.rust-lang.org/stable/releases.html#version-1911-2025-11-10).

## Shell updater additions

The shell updater adds `semver` 1.x for standard version precedence (including
prereleases and build metadata) and `sha2` 0.11 with default features disabled
for streaming SHA-256 verification. Neither existing dependencies nor the
standard library provide those operations. The hashing crate avoids relying on
varying external checksum utilities; both additions support Rust 1.91 and keep
the graph free of duplicate crate versions. Networking uses optional system
`/usr/bin/curl` rather than adding an HTTP/TLS dependency stack.


## Native App Center additions

App Center adds the Rust `toml` parser (1.1, parse/std/serde features only) because
manifest v1 requires complete TOML parsing, duplicate-key rejection, and strict
types. No existing crate parsed TOML. `serde`, already in the dependency graph,
is now direct for the recursive duplicate-key-rejecting JSON visitor. The lockfile
retains the existing Rust 1.91 policy and no duplicate crate versions. No package
or workspace version was changed. HTTP uses the existing optional system curl
approach; SHA-256 and PNG verification reuse existing crates. Runtime Python is
probed only for the apps being checked; there is no Python/Tk App Center UI or
updater dependency. Python runtime probes use system Python or an app-local virtual environment.


## Bundled native utilities

`vitrallis-native` reuses SDL2 0.38, font8x8 and the existing SHA-256 crate for
bounded Notepad save-conflict checks. The app binaries have no GUI
framework, async runtime, HTTP stack, Python runtime or embedded server.
`libc` was already transitive; it is now direct in the shared crate and Terminal
for a small documented POSIX boundary (exclusive rename/no-follow flags,
private socket ownership, PTY creation, poll, resize, controlling terminal).
The shell retains its existing `unsafe_code = "forbid"` policy.

Terminal adds **vt100 0.16.2** for maintained ANSI/VT state rather than an ad-hoc
parser. Its new transitive crates are **vte 0.15.0**, **unicode-width 0.2.2**, and
**arrayvec 0.7.8**. Source review found vte's std-enabled OSC accumulator uses a
Vec; the adapter caps control strings at 4 KiB and discards excess through their
terminator. Terminal replies, scrollback, geometry and PTY queues have separate
bounds. The parser sources contain no application-owned unsafe boundary; vte's
upstream implementation remains part of the dependency trust boundary.

`portable-pty 0.9.0` was evaluated. Its cross-platform process abstraction adds
several dependencies unnecessary for the supported POSIX hosts, including a
second bitflags major version rejected by this workspace's strict Cargo lint.
A focused openpty/setsid/TIOCSCTTY adapter therefore uses existing libc. The
post-fork callback performs only documented async-signal-safe OS calls. An owned
child guard kills/reaps on every early failure and shutdown. Linux ARMv7 and
AArch64 compilation plus real host PTY tests exercise the platform boundary.

The workspace remains Rust 1.91, has one version of each dependency, and adds
no native library requirement beyond SDL2 and normal POSIX libc/libutil.
Versioned references: [vt100 0.16.2](https://docs.rs/vt100/0.16.2/vt100/),
[vte 0.15.0](https://docs.rs/vte/0.15.0/vte/),
[portable-pty 0.9.0](https://docs.rs/portable-pty/0.9.0/portable_pty/).
Resource measurements are recorded in [native validation](devices/pocketchip/history/native-validation.md).
