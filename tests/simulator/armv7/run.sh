#!/bin/sh
# Reproducible ARMv7 PocketCHIP-like software simulator for Vitrallis-Shell.
#
#   sh tests/simulator/armv7/run.sh
#
# Builds a cross toolchain and an ARMv7 Debian 13 runtime image, cross-compiles
# the workspace for armv7-unknown-linux-gnueabihf, stages real test executables
# and a real release bundle, then executes everything in the emulated ARMv7
# runtime as an unprivileged user. Results land in target/armv7-audit/.
#
# Environment:
#   VITRALLIS_ARTI=0     skip the Arti build (skips the five-executable probe)
#   VITRALLIS_MEMORY=480m  container memory limit (default 480m, device-like)
#   VITRALLIS_CPUS=2     container CPU limit (use 1 to expose slow-host assumptions)
#   DOCKER=podman        alternative container CLI
#
# This validates Linux software behavior under ARMv7 emulation. It is not
# physical PocketCHIP hardware validation and does not emulate Mali-400/Lima,
# the kernel, the display controller or NAND storage.
set -eu
cd "$(dirname "$0")/../../.."

DOCKER=${DOCKER:-docker}
toolchain_image=vitrallis-armv7-toolchain:local
runtime_image=vitrallis-armv7-runtime:local
memory=${VITRALLIS_MEMORY:-480m}
cpus=${VITRALLIS_CPUS:-2}
out=$PWD/target/armv7-audit
version=$(sed -n 's/^version = "\(.*\)"$/\1/p' Cargo.toml | head -n 1)
test -n "$version"

mkdir -p "$out"
rm -rf "$out/results"
mkdir -p "$out/results"

"$DOCKER" build -t "$toolchain_image" -f tests/simulator/armv7/Dockerfile.toolchain tests/simulator/armv7
"$DOCKER" build -t "$runtime_image" -f tests/simulator/armv7/Dockerfile.runtime tests/simulator/armv7

"$DOCKER" run --rm \
    -e VITRALLIS_ARTI="${VITRALLIS_ARTI:-1}" \
    -v "$PWD:/workspace" \
    -v vitrallis-armv7-target:/target \
    -v vitrallis-armv7-cargo:/usr/local/cargo/registry \
    "$toolchain_image"

set +e
# The runtime image is pinned to linux/arm/v7 by its FROM line, so Docker runs
# it under ARM emulation without an explicit --platform (which would attempt a
# registry pull for a locally built tag).
"$DOCKER" run --rm \
    --memory "$memory" --cpus "$cpus" \
    -e VITRALLIS_VERSION="$version" \
    -e VITRALLIS_STAGE=/target/armv7-stage \
    -v vitrallis-armv7-target:/target:ro \
    -v "$PWD:/workspace:ro" \
    -v "$out/results:/results" \
    "$runtime_image"
status=$?
set -e

echo "ARMv7 simulator results: $out/results"
cat "$out/results/result.txt" 2>/dev/null || true
exit "$status"
