#!/bin/sh
# Requires an image-matched private SDL2 pkg-config directory. Never host SDL.
set -eu
cd "$(dirname "$0")/.."
: "${PKG_CONFIG_LIBDIR:?Set PKG_CONFIG_LIBDIR to the target image SDL2 pkgconfig directory}"
: "${VITRALLIS_ARM_GLIBC:=2.36}"
case "$VITRALLIS_ARM_GLIBC" in
    2.[0-9][0-9]) ;;
    *) echo 'VITRALLIS_ARM_GLIBC must be 2.NN (tested: 2.36)' >&2; exit 1 ;;
esac
if [ ! -f "$PKG_CONFIG_LIBDIR/sdl2.pc" ]; then
    echo 'Missing target sdl2.pc' >&2
    exit 1
fi
export PKG_CONFIG_ALLOW_CROSS=1
# Eliminate a developer's host pkg-config search path from cross builds.
unset PKG_CONFIG_PATH
exec cargo zigbuild --locked --workspace --all-features --release --target "armv7-unknown-linux-gnueabihf.$VITRALLIS_ARM_GLIBC"
