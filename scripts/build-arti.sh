#!/bin/sh
# Standalone dependency, deliberately outside the Shell workspace/link graph.
# Use the publisher's locked dependency graph and a compact pure-Rust TLS build.
set -eu
cd "$(dirname "$0")/.."
arti_target=${1:-$(rustc -vV | sed -n 's/^host: //p')}
case "$arti_target" in *[!a-zA-Z0-9_.-]*|'') echo 'Invalid target' >&2; exit 1 ;; esac
arti_output=${2:-target/release}
arti_build_root=$(pwd)/target/arti-build/$arti_target
cargo install --locked --version 2.6.0 --no-default-features \
    --features tokio,rustls-ring,compression,onion-service-client,harden,static-sqlite \
    --target "$arti_target" --root "$arti_build_root" arti
mkdir -p "$arti_output"
cp "$arti_build_root/bin/arti" "$arti_output/arti"
