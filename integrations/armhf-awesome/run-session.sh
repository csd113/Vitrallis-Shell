#!/bin/sh
# Run the reversible user session installed by install-session.py.
set -eu
: "${DISPLAY:?Run from a terminal in the existing ARMv7 Linux X11 session}"
: "${HOME:?Expected the normal device user home}"
exec "$HOME/.local/share/vitrallis/launch" "$@"
