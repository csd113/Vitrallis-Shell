# Rust binary runtime notices

Audit date: **2026-10-02**. The release toolchain is **Rust 1.99.0**,
commit `b940084d7eb6a299eb4bfeb8e34901bc051e7ac4` (LLVM 23.1.1).
The Rust standard library and compiler intrinsics are separate from a project's
Cargo.lock. Their terms remain applicable to code incorporated in an executable.
This inventory supplements [the crate inventory](dependency-licenses.md) and
[release notices](../THIRD_PARTY_NOTICES.md).

The matching `rust-src` component, toolchain `COPYRIGHT-library.html`, and exact
upstream [REUSE annotations](https://github.com/rust-lang/rust/blob/b940084d7eb6a299eb4bfeb8e34901bc051e7ac4/REUSE.toml)
were reconciled with the standard library's locked normal/build graph, using
`backtrace,panic-unwind`, for all three distributed Linux targets:
`armv7-unknown-linux-gnueabihf`, `aarch64-unknown-linux-gnu`, and
`x86_64-unknown-linux-gnu`. Those conservative source graphs resolve the same
10 external package versions. This does not assert that every listed build
crate, math routine, or source file is linked into every executable.
Compiler tooling, documentation, tests, and other-platform code in the full
compiler copyright report are outside this binary runtime inventory.

Where terms offer MIT as an alternative, MIT is selected. AND expressions keep
all required terms. The exact upstream files and copyright/permission comment
excerpts are retained in [the collected texts](../THIRD_PARTY_LICENSES.txt),
with Text IDs equal to the first 16 hex digits of their SHA-256 hashes.
The PocketCHIP Places payload carries the same runtime notices locally.

| Runtime component | Terms / attribution | Text IDs |
| --- | --- | --- |
| std, core, alloc, unwind, panic runtimes and std_detect | MIT option; The Rust Project Developers / Contributors | b71bd43a069ca064 |
| core Unicode data | Unicode-3.0; 1991–2024 Unicode, Inc. | f5062c9a188d81df |
| std backtrace | MIT option; 2014 Alex Crichton and The Rust Project Developers | b71bd43a069ca064; retained REUSE annotation |
| std mpmc channels | MIT option; 2019 The Crossbeam Project Developers and The Rust Project Developers | b71bd43a069ca064; retained REUSE annotation |
| compiler_builtins 0.1.160 | MIT AND Apache-2.0 WITH LLVM-exception AND (MIT OR Apache-2.0); MIT chosen for the alternative group | ab6eec6caf0fa577, 3823dda7cf046602 |
| bundled math attribution | Jorge Aparicio, musl contributors, Sun Microsystems, David Schultz, Bruce D. Evans, Alexei Sibidanov and port contributors; upstream MIT, permissive Sun and BSD notices retained | 3823dda7cf046602 and 18 per-file copyright/permission excerpts |

| External crate | Version | Declared terms | Retained Text IDs (MIT option) |
| --- | --- | --- | --- |
| addr2line | 0.27.1 | Apache-2.0 OR MIT | e99d88d232bf57d7 |
| adler2 | 2.0.1 | 0BSD OR MIT OR Apache-2.0 | 23f18e03dc49df91 |
| cfg-if | 1.0.4 | MIT OR Apache-2.0 | 378f5840b258e277 |
| gimli | 0.34.0 | MIT OR Apache-2.0 | 7b63ecd5f1902af1 |
| hashbrown | 0.17.1 | MIT OR Apache-2.0 | ff8f68cb076caf8c |
| libc | 0.2.189 | MIT OR Apache-2.0 | 123a331b5dbf04c3 |
| memchr | 2.8.3 | Unlicense OR MIT | 01c266bced4a434d, 0f96a83840e146e4 |
| miniz_oxide | 0.9.1 | MIT OR Zlib OR Apache-2.0 | 799e9ca9d179295e, 4108245a1f2df9d4 |
| object | 0.39.1 | Apache-2.0 OR MIT | 0b74dfa0bcee5c42 |
| rustc-demangle | 0.1.28 | MIT/Apache-2.0 | 378f5840b258e277 |

The retained compiler-builtins file includes the LLVM exception and compiler-rt
attribution; its AND terms are not reduced to an MIT-only claim. The bundled
libm file includes musl and CORE-MATH attribution. Individual source excerpts
also preserve the Sun permission notices, David Schultz's binary redistribution
conditions, and Alexei Sibidanov's copyright notices.

An ARM ELF dependency inspection confirmed that the four native Shell/utility
executables dynamically require OS `libSDL2`, `libgcc_s`, and `libc`; Arti also
requires OS `libm` and the ARM loader. Those OS libraries are not files in the
native Vitrallis bundle. This record does not certify a redistributed OS image
or a future build made with another compiler or dependency graph.
