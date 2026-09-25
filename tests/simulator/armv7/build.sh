#!/bin/sh
# Cross-build the ARMv7 workspace, test executables and release bundle inside
# the toolchain container (Dockerfile.toolchain). The workspace is the mounted
# checkout; all build output goes to the /target volume.
set -eu
cd /workspace
: "${VITRALLIS_ARTI:=1}"
rm -rf /target/armv7-stage
mkdir -p /target/armv7-stage

cargo build --locked --release --workspace --all-features \
    --target armv7-unknown-linux-gnueabihf
cargo test --locked --no-run --workspace --all-features \
    --target armv7-unknown-linux-gnueabihf \
    --message-format=json | python3 /opt/stage-tests.py

mkdir -p /target/armv7-stage/release
for name in vitrallis vitrallis-terminal vitrallis-notepad vitrallis-files; do
    install -m 755 "/target/armv7-unknown-linux-gnueabihf/release/$name" \
        "/target/armv7-stage/release/$name"
done
# Tiny native packages for the App Center lifecycle test. Building them here
# (rather than requiring rustc in the runtime container) keeps that test on its
# documented emulation-only failure: native process identity.
mkdir -p /target/armv7-stage/native-fixtures
for version in v1 v2; do
    printf 'fn main() { println!("%s"); std::thread::sleep(std::time::Duration::from_secs(60)); }\n' \
        "$version" >/tmp/native-fixture.rs
    rustc --target armv7-unknown-linux-gnueabihf \
        -C linker=arm-linux-gnueabihf-gcc -C strip=symbols \
        -o "/target/armv7-stage/native-fixtures/$version" /tmp/native-fixture.rs
done
if [ "$VITRALLIS_ARTI" = 1 ]; then
    # build-arti.sh stages under target/arti-build; keep the workspace mounted
    # read-write so the artifact cache survives repeated simulator runs.
    sh scripts/build-arti.sh armv7-unknown-linux-gnueabihf \
        /target/armv7-stage/release

    version=$(python3 -c 'import json, subprocess
metadata = json.loads(subprocess.check_output(
    ["cargo", "metadata", "--no-deps", "--locked", "--format-version", "1"]))
print(next(p["version"] for p in metadata["packages"] if p["name"] == "vitrallis-shell"))')
    python3 scripts/package-shell-release.py \
        --bin-dir /target/armv7-stage/release \
        --target armv7-unknown-linux-gnueabihf \
        --runner /usr/bin/qemu-arm \
        --output /target/armv7-stage/release-assets \
        --tag "v$version"
    echo "packaged ARMv7 release bundle for v$version"
else
    echo 'VITRALLIS_ARTI=0: skipping Arti and the release bundle' >&2
fi
