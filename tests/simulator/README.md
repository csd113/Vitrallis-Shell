# App Center Docker simulator

Run from the Vitrallis-Shell checkout. Docker is the only host prerequisite.
The image contains Rust 1.91.1, SDL2, Python/Tk, Xvfb, Openbox, Awesome, D-Bus and xdotool.

```sh
docker build -t vitrallis-app-center-simulator -f tests/simulator/Dockerfile .
docker run --rm \
  -v "$PWD:/workspace" \
  -v vitrallis-app-center-target:/target \
  -v vitrallis-app-center-cargo:/usr/local/cargo/registry \
  vitrallis-app-center-simulator
```

The runner builds all workspace binaries, runs the complete Rust suite, then
operates the real shell at 480×272. It checks actual installed files and receipts,
menu activation, executed fixture versions, search/filter pixels and HTTP counts.
Screenshots, process logs, request logs and scenario results are copied to
`target/app-center-audit/docker/`, including on failure. This directory is ignored
by Git. Isolated homes are created under `/sim`, never under the host's home.

`repositories.py` serves deterministic GitHub metadata, pinned inventories and
package files over HTTPS. A private test certificate and `/etc/hosts` entries are
installed **only inside the disposable container**. Production curl, host policy,
TLS verification, repository parsing, runtime preflight, installer, transaction,
discovery and launcher code are unchanged by the test environment. No ports are
published and no host certificate store is modified. Fixture versions intentionally
differ; tests cover changed entry points, removed files, invalid payloads, invalid
release notes, unavailable repositories and a running old version.

## Published applications

An additional pass installs the actual published Carousel and Debug packages.
Prepare the exact reviewed publisher checkout first:

```sh
git clone https://github.com/csd113/Vitrallis-Apps.git target/app-center-audit/published-repo
git -C target/app-center-audit/published-repo checkout --detach cd1cbf913044bfe7edd3e2ade656a85b90b06e9c
docker run --rm -e VITRALLIS_TEST_PUBLISHED=1 \
  -v "$PWD:/workspace" \
  -v vitrallis-app-center-target:/target \
  -v vitrallis-app-center-cargo:/usr/local/cargo/registry \
  vitrallis-app-center-simulator
```

The test materializes source bytes from the catalog's exact Git pins. App Center
still obtains them through its real HTTPS acquisition path. Carousel's existing
Python/Pillow/packaging prerequisites are explicitly installed by the test in an
app-local virtual environment; **App Center does not install dependencies**.
`published-runtime.txt` records resolved dependency versions. This optional pass
needs network access to provision those prerequisites. It opens real Tk windows,
checks their process script paths, updates Debug 0.1.1 to 0.1.2 and removes Carousel.
Upstream Debug 0.1.2 changes release metadata/changelog only; the synthetic fixture
provides the distinct-code execution regression.

## Reproducing the original failures

The audited baseline is Shell commit `02df65ba08e694777031fe3bd7a0b8274ddf4e45`.
Build that checkout separately in the image and copy its binary to
`/target/vitrallis-baseline`. Start a disposable container with this workspace,
run `sh tests/simulator/start.sh`, then `python3 tests/simulator/baseline.py`.
The script corrupts only its fixture device-menu configuration after startup,
installs Carousel, and asserts the original catalog wipe and missing live menu
entry. A separate fresh home then installs old Debug and updates to a manifest
with a different entry, demonstrating that the original launcher still executes
the old entry and keeps obsolete files. The normal lifecycle suite retests both
failure conditions successfully.

The simulator verifies Linux software behavior, not physical the target device touch,
ARMv7 performance, hardware media decoding, battery behavior or display electronics.

## Stock session checks

`session.py` runs under `dbus-run-session` on a separate Xvfb display with real
Awesome 4. It uses the verified upstream stock command fixture and an isolated
home with no writable launcher config. It exercises native Terminal/Notepad/Files
launch, Home, resume and graceful close, then checks original focus/key restoration,
preservation of a concurrently added keybinding and missing-utility repair tiles.
It records `stock-session.json`, screenshots and logs beside the App Center results.
The supervisor runs directly because this container has no systemd user manager;
unit command construction, ownership checks, stop and removal are covered by the
Python temporary-filesystem tests. Physical key delivery and actual user-manager
integration on the supported target still need hardware validation.
