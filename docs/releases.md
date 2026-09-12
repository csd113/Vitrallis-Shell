# Release readiness

## Published assets inspected on 2026-09-12

The official GitHub releases API lists `v0.1.0-beta2.5` as a published prerelease.
Its four assets are standalone ARMv7 and x86-64 shell executables with their
SHA-256 sidecars. It has no `.vtrbundle`, installer, session helper or uninstaller.
Earlier published prereleases inspected have the same standalone shape.
[Published beta2.5](https://github.com/csd113/Vitrallis-Shell/releases/tag/v0.1.0-beta2.5).

Current source builds four native binaries and expects one complete `.vtrbundle`.
The bootstrap deliberately rejects the old assets. Neither a successful staged
command nor a source build proves that the public one-line installation works.
The new bootstrap URL must exist on `main`, and compatible assets must be published
before the README command can install from public GitHub.

## Prepared packaging

`package-shell-release.py` validates all four executable targets and versions,
then creates a bundle and checksum atomically. ARM packaging also includes
`bootstrap.py`, `install.py`, `uninstall.py`, `vitrallis-session.py` and one exact
SHA-256 sidecar for each. Helpers and bundle therefore come from the same checkout
and release. The bootstrap downloads the matching installer/session/removal
helpers; the shell's own updater continues to update only the native generation.

The tag workflow builds and validates Linux x86-64 and ARMv7 against Debian 12's
glibc 2.36 / SDL2 2.26.5 baseline. It prepares a **draft**, marks prerelease tags
as prereleases, and refuses to modify an already published release. Review drafts
before publishing. The source archive includes all workspace members, artwork,
installer/removal helpers and documentation.

## Publication blockers

1. Review and publish the bootstrap and repository changes to `main`.
2. Obtain explicit authorization for a release version. The workspace remains
   `0.1.0-beta2.5`; another version or replacement of that published release needs separate
   maintainer authorization. Equal-version native builds do not become self-updates.
3. Build the complete artifacts through the release workflow, review the draft
   inventory and checksums, and test the bundle on the intended PocketCHIP image.
4. Publish the reviewed compatible release, then test the literal README command
   against live URLs and the offline uninstall on that device.

No tag, commit, push, draft creation, publication or hardware connection is part
of the repository-polish validation. Host fixtures and local tests are recorded
in [validation](validation.md).

## Suggested repository About

Description: **A compact Rust + SDL2 launcher for small-screen Linux, with native
Terminal, Notepad and Files, App Center, and reversible PocketCHIP integration.
Currently in beta.**

Topics: `pocketchip`, `embedded-linux`, `rust`, `sdl2`, `launcher`, `touchscreen`.
These are suggestions; repository settings are not changed by this checkout.
