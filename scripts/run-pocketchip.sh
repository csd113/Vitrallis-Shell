#!/bin/sh
# FUTURE manual use only. Does not install files or modify session startup.
set -eu
: "${DISPLAY:?Run from a terminal in the existing PocketCHIP X11 session}"
if [ ! -x /opt/vitrallis/vitrallis ]; then
    echo 'Expected a separately staged executable at /opt/vitrallis/vitrallis' >&2
    exit 1
fi
export SDL_VIDEODRIVER=x11
exec /opt/vitrallis/vitrallis --pocketchip "$@"
