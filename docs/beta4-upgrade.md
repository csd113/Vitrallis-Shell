# beta4 release and upgrade paths

## Why beta3.9 exists

Published Shell updaters read exactly four bundle entries and reject trailing
bytes. Adding Arti as a fifth executable would make a direct update fail.
**beta3.9** is therefore a four-executable bridge. It installs through the existing
updater, but its replacement updater reads the complete five-executable v2 bundle
and verifies Arti 2.6.0 independently of the Shell version.

Old clients always choose the highest published version, regardless of whether
GitHub labels it latest. Publishing beta3.9 before beta4 cannot force all users
through the bridge. beta4 therefore also supplies a four-executable entry point
under the original artifact filename. Its four binaries are real beta4 builds;
no release or executable version is misrepresented.

## Updating an existing installation

Use **Settings → Software Updates** and the normal Check, Install and Relaunch
controls. The installation confirmation still defaults to Cancel.

- From beta3.9: install beta4's full v2 bundle and relaunch. Arti is included.
- From an older managed build after beta4 is published: install beta4 and relaunch,
  then repeat Check, Install and Relaunch once. The second check offers
  **Complete this release: 0.1.0-beta4** and installs the full v2 bundle.
- A complete beta4 installation reports up to date and does not reinstall itself.

The initial beta4 transition generation keeps ordinary applications usable. Tor
remains unavailable and required apps fail closed until the second update finishes.
There is no manual reinstall, terminal command or change to app/wallet data.
Both steps use the existing verified-download, immutable-generation and atomic
pointer transaction, with the previous generation retained. A failed download or
verification leaves the current generation active. Same-version completion cannot
select an older release, accept a symlink in place of Arti or omit a core companion.

Arti defaults and private state directories initialize on first use. Required-app
network isolation additionally needs the system Bubblewrap package and working
unprivileged namespaces. Fresh PocketCHIP bootstrap installs that prerequisite;
OTA does not gain root privileges or silently run APT. An older system without
Bubblewrap can use ordinary apps and the shared Tor proxy, but required apps fail
closed with an explicit prerequisite diagnostic. The tested PocketCHIP already
has Bubblewrap. This dependency does not require reinstalling Vitrallis.

## Published assets

beta3.9 retains the original four-executable artifact names. beta4 publishes:

- `vitrallis-<target>-glibc2.36-v2.vtrbundle` and its SHA-256 sidecar: full five-executable build.
- `vitrallis-<target>-glibc2.36.vtrbundle` and its sidecar: four-executable beta4 OTA entry point.
- Matching ARM installer/session helpers and sidecars, as before.

New beta4 installations use the matching bootstrap and v2 bundle directly.
Arti remains a standalone shared executable, never a library linked into Shell.
The extra entry point is restricted to beta4 packaging; it is not a general
legacy format or an automatically generated fallback for future releases.
Existing session helpers continue to launch the atomic current pointer; OTA
retains those helpers and user configuration. Older offline uninstallers may
conservatively retain five-file generation directories as unknown content.

## Verification

Release gates include strict formatting/Clippy, Rust and Python suites, packaging
inventory/checksum tests and actual release-bundle update probes. Regression tests
cover four-file to five-file switching, same-version completion, complete builds
remaining current, independent Arti version validation, unsafe/missing companions,
concurrent inventory changes and refusal to downgrade.

The [Tor validation report](tor-validation.md) records service, isolation,
simulator and PocketCHIP UI checks performed before release versioning.
