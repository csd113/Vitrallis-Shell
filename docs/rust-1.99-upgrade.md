# Rust 1.99 compiler upgrade — 2026-10-01

Development and release builds now pin Rust 1.99.0 instead of 1.91.1. CI checks
1.99.0, latest stable and the existing Rust 1.91.0 minimum, selecting each with
RUSTUP_TOOLCHAIN so the repository pin cannot override the matrix. Advance exact
release pins after the newest stable passes the canonical validation gates.
There is no background updater or floating release compiler.

The Rust 1.91 MSRV, edition 2024, application version 1.0.0-beta-2, dependency
versions and Cargo.lock remain unchanged. Linux builds retain Debian 12 glibc
2.36 / SDL2 2.26.5 and ARMv7 hard-float contracts. The official Rust 1.99 Docker
tag was not yet published during this upgrade, so containers bootstrap from
rust:1.98.1-bookworm and install exact 1.99.0 using official rustup. The installed
release compiler is 1.99.0; the older base tag does not select the compiler.

## Code and fixture changes

New strict Clippy diagnostics are resolved with checked division, fixed-size
pixel chunks, simpler polling and duration constructors, clearer existing test
assertions and named mutable writers. Zero-total download progress remains
100%. No lint allowances or test removals were introduced.

The Arti build uses cargo install --force so an already installed Arti 2.6.0
cannot silently reuse an executable produced by the previous compiler. The
existing Arti version, locked dependencies and features remain unchanged.

ARM emulation exposed bootstrap fixture startup overhead: dozens of tiny
standard-library-only Python command mocks initialized optional site packages.
Those mocks now use Python -S. Command behavior, assertions, coverage and the
existing 15-second subprocess deadlines remain unchanged.

## Validation record

- macOS Apple Silicon: complete scripts/validate.sh with Rust 1.99.0 passed:
  formatting, locked check, strict all-target/all-feature Clippy, 354 Rust tests
  (nine existing ignored), 160 Python tests (nine existing platform skips),
  four release executables, native renderer goldens, SDL smoke checks at both
  supported sizes, script syntax, Python compilation and documentation links.
- Rust 1.91.0: all-target/all-feature locked check and strict Clippy passed.
- Debian 12 Linux ARM64: complete canonical validation passed, including 355
  Rust tests (12 existing ignored), Python, release, renderer and SDL gates.
- After the mock startup change, all 160 native Python tests passed again.
- ARMv7: all four release applications, ten test executables and Arti 2.6.0 were
  rebuilt with Rust 1.99.0. The five-executable release bundle passed packaging,
  version and ELF hard-float checks under QEMU. The final ARM runtime harness
  passed all 31 enabled gates, including 155 Python tests (nine existing skips).
  The separately mounted real bundle passed the production upgrade probe.
  Initial ARM Python timeout failures and all successful reruns are preserved
  in the local validation logs; no deadlines were raised.

- Linux x86-64: all four applications and Arti 2.6.0 rebuilt with Rust 1.99.0
  on Debian 12; the five-executable bundle passed packaging and QEMU version
  checks. Every executable requires at most GLIBC_2.34, within the retained
  glibc 2.36 contract.

The existing ARM simulator explicitly recognizes five QEMU process identity
artifacts; its production checks and existing exceptions are unchanged. This is
software emulation, not physical PocketCHIP hardware or GPU validation.

## Local preservation and cleanup

After validation finished and all writers/builds were idle, runnable macOS and
Linux binaries, both release bundles, ARM test executables and all historical
screenshots/reports/recovery evidence were preserved outside target. Unique
historical evidence was moved on the same filesystem rather than copied.

Native cargo clean removed 217,221 files (31.4 GiB logical); the retained cache
used 15,417,602,048 allocated bytes after evidence preservation. Only verified
Cargo-generated source-archive caches were included. The two completed Docker
target volumes were also cleaned: 20,189 files (8.8 GiB) and 9,236 files (3.1 GiB).
Their old cache tags were missing/damaged; valid Cargo-generated tags were
restored only after inventory and byte-verification of preserved unique output.
No images, registry caches, unrelated volumes or other repositories were pruned.

No push, tag, release publication or deployment is performed by this upgrade.
