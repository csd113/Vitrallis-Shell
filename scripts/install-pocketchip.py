#!/usr/bin/python3
"""Forward the original installer path to the PocketCHIP implementation."""
from pathlib import Path
import runpy


def main():
    here = Path(__file__).resolve().parent
    for installer in (here.parent / 'devices/pocketchip/install.py', here / 'install.py'):
        if installer.is_file():
            runpy.run_path(str(installer), run_name='__main__')
            return
    raise SystemExit(
        'Install failed: missing devices/pocketchip/install.py. For standalone '
        'use, place install.py and vitrallis-session.py beside install-pocketchip.py.'
    )


if __name__ == '__main__':
    main()
