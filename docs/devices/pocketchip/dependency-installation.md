# App Center Python dependency installation — 2026-09-19

The reported Carousel 0.1.4 → 0.2.0 update failed because a private venv hid
Debian's compatible Pillow 11.1.0. Pip attempted a Pillow source build on ARMv7,
where `arm-linux-gnueabihf-gcc` was unavailable. The package requires
`Pillow>=10.4,<13` and `qrcode>=7.4,<9`; qrcode was missing.

## Implementation and trust boundary

New managed environments can read the base Python's system site packages.
Pip's normal resolver retains compatible distributions and installs missing or
incompatible ones into the managed environment. Existing system Python was
already an accepted runtime; this does not create a sandbox. System package
changes can affect these environments later. No apt operation, compiler install,
new Rust dependency, or version bump is part of this change.

Python `-I`, launcher user-site exclusion, removal of inherited pip/Python
settings, and `PIP_CONFIG_FILE=/dev/null` prevent user-site or pip configuration
from changing resolution/destinations. A shared metadata-only validator rejects
unsupported declarations even behind false markers, validates version ranges,
and checks dependencies and Tk in staging before the rename. Existing private
venvs and managed environments are preserved. App files still use the existing
journaled transaction. Errors repeat the useful pip/compiler cause before the
bounded diagnostic tail, which remains scrollable in Details.

## Host validation

`sh scripts/validate.sh` passed on macOS, including:

- `cargo fmt --all --check`.
- Workspace check and strict Clippy with all targets/features and warnings denied.
- `cargo test --workspace --all-features`: 241 tests passed; eight explicitly
  online/device/graphics tests were ignored.
- Python discovery: 102 tests, two skipped for their explicit environment gates.
- Four release binaries, version checks, SDL dummy-driver smoke tests, renderer
  tests, source archive rebuild, shell/Python syntax, local links, and diff checks.

The new offline Python fixtures run real venv/pip against local wheels. They
verify compatible system reuse, local replacement of an incompatible version,
missing-package installation, disabled pip redirects, no system-package changes,
requirement-policy failures, and cleanup at venv, validation, pip and final-check
failure boundaries. Rust tests preserve process completion/deadline coverage and
check that the compiler cause precedes pip's generic wheel failure.

## Physical validation method

The cross-build uses the existing device-matched SDL2 sysroot and:

```sh
PKG_CONFIG_LIBDIR=/tmp/vitrallis-arm-sysroot/lib/pkgconfig \
PKG_CONFIG_ALLOW_CROSS=1 cargo zigbuild --locked --release --lib --tests \
  --target armv7-unknown-linux-gnueabihf.2.36
```

A temporary test executable calls the production App Center `install_one` path:
source trust, running-app checks, pinned GitHub acquisition, inventory/hash checks,
dependency provisioning, transactional installation, receipt and local-state
verification. It does not replace or restart the installed shell. The ignored
`physical_python_managed_update` test requires an explicit home, app ID, source
commit, old version and new version; ambiguous or changed selection fails closed.

The authorized target is `io.vitrallis.mediacarousel`, 0.1.4 → 0.2.0, pinned to
`3e66e70d3eabd059930542abbc7e332648e165c2`. Before mutation, all old receipt hashes
are checked, the old app is backed up, and user settings/data plus system Pillow
are checksummed. This exercises managed installation on hardware; it is not a
physical touchscreen test or deployment of a new shell release.

## Physical result

On Debian 13 ARMv7, Python 3.13.5 / Tk 8.6, the complete managed update passed in
95.00 seconds. The resulting receipt is 0.2.0 at the reviewed commit. Runtime
imports confirmed Pillow 11.1.0 from `/usr/lib/python3/dist-packages/PIL`, qrcode
8.2 from the app's managed `runtime/<requirements hash>` directory, and packaging
25.0. User-site access is disabled. The launcher points at the validated managed
interpreter. No `.pending-*` environment or `.installation-pending` marker remains.
The two runtime process/deadline tests also passed on the physical ARM device.

The installed shell binary and OS packages were not replaced. Shipping or
installing this modified shell requires separate authorization. A missing or
incompatible compiled dependency can still require a compatible wheel or system
prerequisite; this change avoids rebuilding a system dependency that already
satisfies the app's requirements.

After the update, all 380 recorded settings/data/system records were unchanged,
including approximately 259 MiB of Carousel saved data. Every new receipt-owned
file matched its saved checksum. Global package versions were unchanged and
qrcode remained absent from system Python. The old app backup and detailed QA
logs were retained in the development checkout's ignored `target/dependency-qa/`
directory; temporary device test files were removed.

The four offline dependency tests also passed as an unprivileged user in the
Debian simulator container. CI/simulator test prerequisites now explicitly include
`python3-venv` and `python3-packaging`; both workflow YAML files parsed successfully.
The complete host validation script was rerun after the final diagnostic wording
change and passed again. The final ARM build's runtime completion/deadline tests
were rerun on the device.
