#!/usr/bin/python3
"""Apply only the reviewed compatibility files to an exact installed store."""
import hashlib
import json
import os
from pathlib import Path
import stat
import sys
import tempfile
import time

if sys.version_info < (3, 8):
    raise SystemExit("Vitrallis installer requires Python 3.8 or newer")


def safe(path):
    if any(p.is_symlink() for p in (path,) + tuple(path.parents)):
        raise ValueError('Refusing symlink: ' + str(path))
    if path.exists() and path.stat().st_nlink != 1:
        raise ValueError('Refusing hardlink: ' + str(path))
    if path.exists() and not path.is_file():
        raise ValueError('Expected regular file: ' + str(path))


def read_file(path, limit=2 * 1024 * 1024):
    safe(path)
    fd = os.open(path, os.O_RDONLY | os.O_NOFOLLOW | os.O_NONBLOCK)
    with os.fdopen(fd, 'rb') as stream:
        if not stat.S_ISREG(os.fstat(stream.fileno()).st_mode):
            raise ValueError('Expected regular input: ' + str(path))
        data = stream.read(limit + 1)
    if len(data) > limit:
        raise ValueError('Input exceeds size limit: ' + str(path))
    return data


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
        directory = os.open(path.parent, os.O_RDONLY | os.O_DIRECTORY)
        try:
            os.fsync(directory)
        finally:
            os.close(directory)
    finally:
        if os.path.exists(name):
            os.unlink(name)


def apply(source, manifest, target):
    data = json.loads(read_file(manifest))
    previous, writes = {}, {}
    names = {'update_apps.py', 'deployment.py', 'test_deployment.py', 'test_update_apps.py', 'test_self_update.py'}
    if set(data['files']) != names:
        raise ValueError('Unexpected patch file list')
    for name, hashes in data['files'].items():
        path = target / name
        safe(path)
        old = read_file(path)
        safe(source / name)
        new = read_file(source / name)
        if hashlib.sha256(old).hexdigest() not in [hashes['before'], hashes['after']] + hashes.get('previous', []):
            raise ValueError('Installed file differs from reviewed versions: ' + name)
        if hashlib.sha256(new).hexdigest() != hashes['after']:
            raise ValueError('Patch content does not match manifest: ' + name)
        compile(new, name, 'exec')
        previous[path], writes[path] = old, new
    marker = target / '.installation-pending'
    safe(marker)
    if all(previous[p] == v for p, v in writes.items()) and not marker.exists():
        print('Store patch already applied')
        return
    # The store itself uses this lock; no live replacement of an active updater.
    import fcntl
    lock_path = target / 'updater.lock'
    safe(lock_path)
    fd = os.open(lock_path, os.O_RDWR | os.O_CREAT | os.O_NOFOLLOW | os.O_NONBLOCK, 0o600)
    with os.fdopen(fd, 'a+') as lock:
        if os.fstat(lock.fileno()).st_nlink != 1:
            raise ValueError('Refusing hardlinked updater lock')
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
                if read_file(path) != previous[path]:
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
                safe(path)
                if path.exists() and read_file(path) == writes[path]:
                    atomic(path, previous[path])
            raise
        print('Store compatibility patch applied; backup:', backup)


if __name__ == '__main__':
    try:
        if len(sys.argv) != 3 or os.geteuid() == 0:
            raise ValueError('Run as normal user: apply-store-patch.py PATCHED_SOURCE MANIFEST')
        apply(Path(sys.argv[1]), Path(sys.argv[2]), Path.home() / '.local/share/pocket-update-apps')
    except (OSError, ValueError, KeyError, TypeError, SyntaxError) as error:
        print('Store patch failed: ' + str(error), file=sys.stderr)
        sys.exit(1)
