# Design direction

Vitrallis makes small Linux screens useful without replacing the operating
system underneath them. The current product is a Rust/SDL2 shell, three native
utilities, a manifest-based App Center, and a PocketCHIP platform adapter.
[Repository boundaries](repository-layout.md) describe that implementation;
this page describes direction, not additional shipped features.

## Principles

Keep interaction legible at 480×272 and usable with keys and touch. Treat display
size independently from hardware identity. Share input and layout behavior;
keep hardware commands, OS paths and recovery policy inside device adapters.

Use small native utilities for the essential offline experience. Third-party
applications remain separate programs with their own runtime requirements.
Catalog metadata describes their identity, content and permissions; it does not
sandbox them. The current [app contract](app-development.md) is authoritative.

Marshmallow is an independent OS component and recovery home. Preserve its
startup, preferences and tools. A replacement Vitrallis interface must not remove
working recovery or imply authentication guarantees it does not implement.
The [reference audit](history/marshmallow-reference.md) records relevant source
observations without vendoring the reference implementation.

## Roadmap, not release promises

- Validate the complete native bundle, update and removal lifecycle on PocketCHIP,
  including endurance, battery drain, physical input and storage failures.
- Improve font coverage and text rendering, and evaluate additional artwork formats.
- Establish independently verifiable publisher signatures and clearer runtime
  and window-identity interfaces before expanding trust or support claims.
- Add another device only with a platform adapter, unavailable-data behavior,
  recovery design and recorded hardware validation.
- Evaluate a public Python SDK, themes and application-creation tooling when
  concrete app requirements justify the maintenance cost.

No SDK, theme format, Forge application, new package format, or second hardware
backend is promised by this roadmap. Superseded pre-release implementations are
removed under the [contributor policy](../CONTRIBUTING.md#engineering-rules).
