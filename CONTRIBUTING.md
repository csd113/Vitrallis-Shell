# Contributing to Vitrallis

Thanks for helping make small-screen Linux more useful. Bug reports, clear
reproductions, documentation fixes, and focused patches are welcome. Start with
an [issue](https://github.com/csd113/Vitrallis-Shell/issues/new/choose) for a larger
behavior or architecture change. See [SECURITY.md](SECURITY.md) for sensitive reports.

## Build and check

Install Rust through rustup, SDL2 development libraries, and pkg-config on your
**development host**. The repository pins Rust 1.91.1; all workspace packages
support Rust 1.91. On Debian/Ubuntu the native development packages are
`libsdl2-dev` and `pkg-config`; `python3-tk`, `python3-venv` and
`python3-packaging` support runtime inspection and offline dependency tests.
On macOS, Homebrew's `sdl2` and `pkg-config` provide the native libraries.

```sh
cargo build --workspace --locked
cargo run --locked
sh scripts/validate.sh
```

Build the workspace so Terminal, Notepad, and Files are available beside the
shell. The validation script runs formatting, workspace checks, strict Clippy,
Rust and Python tests, release builds, SDL smoke tests and script/link checks.
Its source-archive test rebuilds the complete workspace offline in a temporary
directory. See [validation](docs/validation.md) for the gates and test boundaries,
[layout](docs/repository-layout.md) for where code belongs, and
[dependencies](docs/dependencies.md) for the dependency policy.

## Engineering rules

- Make focused, conservative changes. Preserve unrelated work and behavior.
  Avoid dependency additions unless the standard library, current crates, or
  a suitable optional external tool cannot solve the problem.
- Use safe, idiomatic Rust, explicit errors and clear types. Avoid production
  `unwrap`, `expect`, and panics without a documented strong invariant. Keep
  public APIs small and avoid unnecessary allocation or global state.
- Do not casually silence Clippy. Keep justified exceptions narrow. Keep tests
  close to the behavior they protect and exercise meaningful failure cases.
- Treat external data, paths, environment, restored content and configuration
  as untrusted. Validate before mutation or execution. Security and ownership
  checks fail closed; multi-file operations need recovery and edit preservation.
- Every visible native navigation/action button must have reachable, visible
  keyboard selection and the same activation behavior as touch. Preserve safe
  confirmation defaults for power and installation actions.
- Preserve route names, IDs, form fields and UI hooks unless the change requires
  replacing them. Avoid cosmetic changes outside the requested area.
- Never bump version numbers without the maintainer's explicit permission.
  Authorization for a named release applies only to that release.
- Vitrallis is pre-release. Backward compatibility with obsolete pre-release
  builds is not a project requirement unless explicitly requested. When an
  internal API, package format, path, configuration format, architecture, or
  behavior is replaced, update current consumers and remove the superseded
  implementation. Do not introduce compatibility layers, legacy fallbacks,
  aliases, dual code paths, migration shims, deprecated formats, or version-gated
  support unless the task specifically requires them. Supported PocketHome OS
  integration and the original desktop recovery are current boundaries.

## Pull requests

Describe the problem, resulting behavior, files or areas changed, and validation
actually performed. Distinguish mocked/host tests from physical-device evidence;
never claim support from a screenshot or emulator alone. Include screenshots for
visible UI changes, with resolution and capture environment. Document remaining
risks and release blockers. Do not rewrite history or include unrelated cleanup.

Use targeted tests while developing, then run `sh scripts/validate.sh` before
handoff. For formatting failures, format only the affected Rust code and rerun
`cargo fmt --all --check`. The full strict Clippy and workspace test commands are
in [validation](docs/validation.md). Never connect to hardware, publish a release,
or change a user's system as part of an ordinary host test.

## Licensing contributions

Project-owned contributions are accepted under [MIT](LICENSE). Submit only work
that you can license on those terms. Preserve third-party copyright, license and
NOTICE files; record imported code/assets, exact source revisions and terms in
[THIRD_PARTY_NOTICES.md](THIRD_PARTY_NOTICES.md). Public availability or a supplied
image is not permission to redistribute it. Unresolved items listed there are
excluded from the project MIT grant until their rights are established.
