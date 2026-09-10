#!/usr/bin/python3
"""Stage Vitrallis in the user's home and add an opt-in PocketHome shortcut."""
import hashlib
import json
import os
from pathlib import Path
import stat
import sys
import tempfile
import time


def safe(path):
    if any(p.is_symlink() for p in (path,) + tuple(path.parents)):
        raise ValueError('Refusing symlink: ' + str(path))
    if path.exists() and not path.is_file():
        raise ValueError('Expected a regular file: ' + str(path))


def atomic(path, data, mode):
    safe(path)
    path.parent.mkdir(parents=True, exist_ok=True)
    fd, temp = tempfile.mkstemp(prefix='.vitrallis-', dir=path.parent)
    try:
        with os.fdopen(fd, 'wb') as stream:
            stream.write(data)
            stream.flush()
            os.fsync(stream.fileno())
        os.chmod(temp, mode)
        os.replace(temp, path)
    finally:
        if os.path.exists(temp):
            os.unlink(temp)


def install(binary, source, home):
    target = home / '.local/share/vitrallis'
    config_path = home / '.pocket-home/config.json'
    safe(config_path)
    original = config_path.read_bytes()
    if len(original) > 1024 * 1024:
        raise ValueError('PocketHome config exceeds 1 MiB')
    config = json.loads(original)
    pages = config.get('pages')
    if not isinstance(pages, list):
        raise ValueError('Missing PocketHome pages')
    page = next((p for p in pages if isinstance(p, dict) and p.get('name') == 'Apps'
                 and isinstance(p.get('items'), list)), None)
    if page is None or not all(isinstance(i, dict) for i in page['items']):
        raise ValueError('Missing or malformed Apps page')
    if any(c in str(home) for c in '\r\n\0"\\%'):
        raise ValueError('Unsupported home directory characters')
    safe(binary)
    if not binary.is_file() or binary.stat().st_size > 64 * 1024 * 1024:
        raise ValueError('Expected a regular binary at most 64 MiB')
    data = binary.read_bytes()
    if len(data) < 20 or data[:7] != b'\x7fELF\x01\x01\x01' or data[18:20] != b'\x28\x00':
        raise ValueError('Expected a little-endian 32-bit ARM ELF binary')
    launch = target / 'launch'
    entry = dict(name='Vitrallis', icon='appIcons/terminal.png', shell='"' + str(launch) + '"')
    matches = [i for i in page['items'] if i.get('name') == 'Vitrallis']
    if matches and (len(matches) != 1 or matches[0].get('shell') != entry['shell']):
        raise ValueError('An unrelated Vitrallis entry exists; refusing replacement')
    if not matches:
        page['items'].append(entry)
    session_source = source / 'vitrallis-session.py'
    safe(session_source)
    session_data = session_source.read_bytes()
    compile(session_data, str(session_source), 'exec')
    desktop = home / '.local/share/applications/vitrallis.desktop'
    writes = {
        target / 'vitrallis': (data, 0o755),
        target / 'vitrallis-session.py': (session_data, 0o644),
        launch: (b'#!/bin/sh\nexec /usr/bin/python3 "$HOME/.local/share/vitrallis/vitrallis-session.py" "$@"\n', 0o755),
        desktop: ((
            '[Desktop Entry]\nType=Application\nName=Vitrallis\nExec="' + str(launch) +
            '"\nTerminal=false\nCategories=System;\n').encode(), 0o644),
        config_path: ((json.dumps(config, indent=2) + '\n').encode(), stat.S_IMODE(config_path.stat().st_mode)),
    }
    receipt_path = target / 'installed.json'
    safe(receipt_path)
    receipt = json.loads(receipt_path.read_bytes()) if receipt_path.exists() else {}
    if not isinstance(receipt, dict):
        raise ValueError('Malformed installation receipt')
    safe(desktop)
    if desktop.exists():
        # Older receipts did not include the shortcut. Accept only the exact
        # generated content when migrating those receipts, never unknown edits.
        expected = receipt.get('desktop_sha256', hashlib.sha256(writes[desktop][0]).hexdigest())
        if hashlib.sha256(desktop.read_bytes()).hexdigest() != expected:
            raise ValueError('Unmanaged or edited desktop shortcut: ' + str(desktop))
    marker = target / '.installation-pending'
    safe(marker)
    pending = json.loads(marker.read_bytes()) if marker.exists() else {}
    if not isinstance(pending, dict):
        raise ValueError('Malformed installation recovery marker')
    for path in writes:
        if path.parent == target and path.exists():
            safe(path)
            if hashlib.sha256(path.read_bytes()).hexdigest() not in [receipt.get(path.name)] + pending.get(path.name, []):
                raise ValueError('Unmanaged or edited installed file: ' + str(path))
    hashes = {p.name: hashlib.sha256(v[0]).hexdigest()
              for p, v in writes.items() if p.parent == target}
    hashes['desktop_sha256'] = hashlib.sha256(writes[desktop][0]).hexdigest()
    writes[receipt_path] = (json.dumps(hashes).encode(), 0o600)
    previous = {}
    for path in writes:
        safe(path)
        previous[path] = (path.read_bytes(), stat.S_IMODE(path.stat().st_mode)) if path.exists() else None
    backup = home / '.local/share/vitrallis-backups' / str(time.time_ns())
    safe(backup / 'paths.json')
    backup.mkdir(parents=True, mode=0o700)
    for index, (path, old) in enumerate(previous.items()):
        if old is not None:
            atomic(backup / str(index), old[0], 0o600)
    atomic(backup / 'paths.json', json.dumps([str(p) for p in writes]).encode(), 0o600)
    atomic(marker, json.dumps({p.name: [hashlib.sha256(old[0]).hexdigest() if old else None,
                                      hashlib.sha256(writes[p][0]).hexdigest()]
                               for p, old in previous.items() if p.parent == target}).encode(), 0o600)
    changed = []
    try:
        for path, (content, mode) in writes.items():
            if path == config_path and path.read_bytes() != original:
                raise ValueError('PocketHome menu changed; retry')
            changed.append(path)
            atomic(path, content, mode)
        marker.unlink()
    except BaseException:
        for path in reversed(changed):
            old = previous[path]
            if old is None:
                path.unlink(missing_ok=True)
            else:
                atomic(path, *old)
        raise
    print('Installed:', target)
    print('Backups:', backup)
    print('Launch:', launch)
    print('Marshmallow remains the default. Restart its menu to see Vitrallis.')
    print('Binary SHA256:', hashlib.sha256(data).hexdigest())


if __name__ == '__main__':
    try:
        if len(sys.argv) != 2 or os.geteuid() == 0:
            raise ValueError('Run as your normal user: install-pocketchip.py ARM_BINARY')
        install(Path(sys.argv[1]), Path(__file__).resolve().parent, Path.home())
    except (OSError, ValueError, TypeError) as error:
        print('Install failed: ' + str(error), file=sys.stderr)
        sys.exit(1)
