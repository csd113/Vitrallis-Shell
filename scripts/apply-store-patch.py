#!/usr/bin/python3
"""Apply only the reviewed compatibility files to an exact installed store."""
import hashlib
import json
import os
from pathlib import Path
import sys
import tempfile
import time


def safe(path):
    if any(p.is_symlink() for p in (path,) + tuple(path.parents)):
        raise ValueError('Refusing symlink: ' + str(path))
    if path.exists() and not path.is_file():
        raise ValueError('Expected regular file: ' + str(path))


def atomic(path, data):
    safe(path)
    fd, name = tempfile.mkstemp(dir=path.parent, prefix='.vitrallis-patch-')
    try:
        with os.fdopen(fd, 'wb') as stream:
            stream.write(data)
            stream.flush()
            os.fsync(stream.fileno())
        os.chmod(name, 0o644)
        os.replace(name, path)
    finally:
        if os.path.exists(name):
            os.unlink(name)


def apply(source, manifest, target):
    data = json.loads(manifest.read_bytes())
    previous, writes = {}, {}
    names = {'update_apps.py', 'deployment.py', 'test_deployment.py', 'test_update_apps.py', 'test_self_update.py'}
    if set(data['files']) != names:
        raise ValueError('Unexpected patch file list')
    for name, hashes in data['files'].items():
        path = target / name
        safe(path)
        old = path.read_bytes()
        new = (source / name).read_bytes()
        if hashlib.sha256(old).hexdigest() not in [hashes['before'], hashes['after']] + hashes.get('previous', []):
            raise ValueError('Installed file differs from reviewed versions: ' + name)
        if hashlib.sha256(new).hexdigest() != hashes['after']:
            raise ValueError('Patch content does not match manifest: ' + name)
        compile(new, name, 'exec')
        previous[path], writes[path] = old, new
    if all(previous[p] == v for p, v in writes.items()) and not (target / '.installation-pending').exists():
        print('Store patch already applied')
        return
    # The store itself uses this lock; no live replacement of an active updater.
    import fcntl
    lock_path = target / 'updater.lock'
    safe(lock_path)
    with lock_path.open('a+') as lock:
        fcntl.flock(lock, fcntl.LOCK_EX | fcntl.LOCK_NB)
        backup = target / ('before-vitrallis-' + str(time.time_ns()))
        backup.mkdir(mode=0o700)
        for path, old in previous.items():
            atomic(backup / path.name, old)
        marker = target / '.installation-pending'
        atomic(marker, b'Store patch incomplete; rerun apply-store-patch.py.\n')
        changed = []
        try:
            for path, new in writes.items():
                if path.read_bytes() != previous[path]:
                    raise ValueError('Store changed during patch installation')
                changed.append(path)
                atomic(path, new)
            for name in names:
                for cache in (target / '__pycache__').glob(Path(name).stem + '.*.pyc'):
                    safe(cache)
                    cache.unlink()
            marker.unlink()
        except BaseException:
            for path in reversed(changed):
                atomic(path, previous[path])
            raise
        print('Store compatibility patch applied; backup:', backup)


if __name__ == '__main__':
    try:
        if len(sys.argv) != 3 or os.geteuid() == 0:
            raise ValueError('Run as normal user: apply-store-patch.py PATCHED_SOURCE MANIFEST')
        apply(Path(sys.argv[1]), Path(sys.argv[2]), Path.home() / '.local/share/pocket-update-apps')
    except (OSError, ValueError, KeyError, TypeError) as error:
        print('Store patch failed: ' + str(error), file=sys.stderr)
        sys.exit(1)
