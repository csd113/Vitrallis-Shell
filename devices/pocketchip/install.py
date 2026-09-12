#!/usr/bin/python3
"""Stage Vitrallis in the user's home and add an opt-in PocketHome shortcut."""
import fcntl
import hashlib
import json
import os
import re
import resource
import subprocess
from pathlib import Path
import stat
import struct
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
        raise ValueError('Expected a regular file: ' + str(path))


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


def make_directories(path, mode=0o755):
    # Path.mkdir(parents=True) creates intermediate directories with 0777
    # masked by the caller's umask. With umask 002 that leaves OTA ancestors
    # group-writable. Apply an explicit safe mode to every new directory;
    # preserve existing directory permissions for the owner to review.
    if path.is_symlink():
        raise ValueError('Refusing symlink: ' + str(path))
    if path.is_dir():
        info = path.stat()
        if info.st_uid not in (0, os.getuid()) or info.st_mode & 0o022:
            raise ValueError('Unsafe installation directory ownership or permissions: ' + str(path))
        return
    make_directories(path.parent)
    try:
        path.mkdir(mode=mode)
    except FileExistsError:
        if path.is_symlink() or not path.is_dir():
            raise ValueError('Expected a directory: ' + str(path))


def atomic(path, data, mode):
    safe(path)
    make_directories(path.parent)
    fd, temp = tempfile.mkstemp(prefix='.vitrallis-', dir=path.parent)
    try:
        with os.fdopen(fd, 'wb') as stream:
            stream.write(data)
            stream.flush()
            os.fsync(stream.fileno())
        os.chmod(temp, mode)
        os.replace(temp, path)
        directory = os.open(path.parent, os.O_RDONLY | os.O_DIRECTORY)
        try:
            os.fsync(directory)
        finally:
            os.close(directory)
    finally:
        if os.path.exists(temp):
            os.unlink(temp)


BINARIES = ('vitrallis', 'vitrallis-terminal', 'vitrallis-notepad', 'vitrallis-files')
MAGIC = b'VITRALLIS-BUNDLE'


def pointer(path):
    if not path.is_symlink():
        if path.exists():
            raise ValueError('Expected a managed build pointer: ' + str(path))
        return None
    target = os.readlink(path)
    parts = Path(target).parts
    if len(parts) != 2 or parts[0] != 'generations' or len(parts[1]) != 64 or any(c not in '0123456789abcdef' for c in parts[1]):
        raise ValueError('Invalid build pointer: ' + str(path))
    return target


def atomic_pointer(path, target):
    temporary = path.with_name('.' + path.name + '-next')
    if temporary.exists() or temporary.is_symlink():
        raise ValueError('Unexpected build pointer staging file: ' + str(temporary))
    os.symlink(target, temporary)
    try:
        os.replace(temporary, path)
        directory = os.open(path.parent, os.O_RDONLY | os.O_DIRECTORY)
        try:
            os.fsync(directory)
        finally:
            os.close(directory)
    finally:
        temporary.unlink(missing_ok=True)


def file_digest(path):
    safe(path)
    before = path.stat()
    if before.st_size > 64 * 1024 * 1024 or before.st_uid != os.getuid() or before.st_mode & 0o7022 or not before.st_mode & 0o111:
        raise ValueError('Unsafe bundled executable: ' + str(path))
    digest = hashlib.sha256()
    total = 0
    fd = os.open(path, os.O_RDONLY | os.O_NOFOLLOW | os.O_NONBLOCK)
    with os.fdopen(fd, 'rb') as stream:
        while True:
            data = stream.read(65536)
            if not data:
                after = os.fstat(stream.fileno())
                if (before.st_ino, before.st_size, before.st_mtime_ns, before.st_ctime_ns) != (after.st_ino, after.st_size, after.st_mtime_ns, after.st_ctime_ns):
                    raise ValueError('Bundled executable changed during verification')
                return digest.digest()
            total += len(data)
            if total > before.st_size:
                raise ValueError('Bundled executable grew during verification')
            digest.update(data)


def limit_probe_output():
    # Installer is single-threaded. Bound the child's regular stdout/stderr file
    # before exec, so a broken --version cannot fill memory or the staging disk.
    resource.setrlimit(resource.RLIMIT_CORE, (0, 0))
    resource.setrlimit(resource.RLIMIT_FSIZE, (4096, 4096))


def verify_versions(generation):
    expected = None
    for name in BINARIES:
        with tempfile.TemporaryFile() as output:
            subprocess.run([str(generation / name), '--version'], stdin=subprocess.DEVNULL,
                           stdout=output, stderr=output, timeout=5, check=True,
                           preexec_fn=limit_probe_output)
            output.seek(0)
            text = output.read(4097).decode('ascii').strip()
        match = re.fullmatch(re.escape(name) + r' ([0-9]+\.[0-9]+\.[0-9]+(?:-[0-9A-Za-z.-]+)?(?:\+[0-9A-Za-z.-]+)?)', text)
        if match is None or (expected is not None and expected != match.group(1)):
            raise ValueError('Bundled executable version mismatch: ' + name)
        expected = match.group(1)


def extract_bundle(bundle, stage):
    safe(bundle)
    fd = os.open(bundle, os.O_RDONLY | os.O_NOFOLLOW | os.O_NONBLOCK)
    whole = hashlib.sha256()
    with os.fdopen(fd, 'rb') as stream:
        if not stat.S_ISREG(os.fstat(stream.fileno()).st_mode):
            raise ValueError('Expected a regular native bundle')
        def read_exact(count):
            data = stream.read(count)
            if len(data) != count:
                raise ValueError('Incomplete native bundle')
            whole.update(data)
            return data
        if read_exact(16) != MAGIC:
            raise ValueError('Expected a complete Vitrallis native bundle')
        for name in BINARIES:
            size = struct.unpack('<Q', read_exact(8))[0]
            expected = read_exact(32)
            if not 64 <= size <= 64 * 1024 * 1024:
                raise ValueError('Invalid bundled executable size: ' + name)
            header = read_exact(64)
            validate_arm(header, size)
            digest = hashlib.sha256(header)
            with (stage / name).open('xb') as output:
                output.write(header)
                remaining = size - 64
                while remaining:
                    data = read_exact(min(65536, remaining))
                    output.write(data)
                    digest.update(data)
                    remaining -= len(data)
                output.flush()
                os.fsync(output.fileno())
            if digest.digest() != expected:
                raise ValueError('Bundled executable checksum failed: ' + name)
            (stage / name).chmod(0o755)
        if stream.read(1):
            raise ValueError('Unexpected trailing bundle content')
    directory = os.open(stage, os.O_RDONLY | os.O_DIRECTORY)
    try:
        os.fsync(directory)
    finally:
        os.close(directory)
    return whole.hexdigest()


def validate_arm(header, size):
    if len(header) < 52 or header[:7] != b'\x7fELF\x01\x01\x01':
        raise ValueError('Expected a little-endian 32-bit ARM ELF binary')
    kind, machine, version, _, phoff, _, flags, ehsize, phsize, phcount = struct.unpack_from('<HHIIIIIHHH', header, 16)
    if (kind not in (2, 3) or machine != 40 or version != 1 or ehsize != 52
            or phsize != 32 or not phcount or phoff < 52
            or phoff + phsize * phcount > size
            or flags & 0xff000000 != 0x05000000 or not flags & 0x400):
        raise ValueError('Expected a complete ARM EABI5 hard-float executable')


def load_inputs(source, home):
    config_path = home / '.pocket-home/config.json'
    safe(config_path)
    original = read_file(config_path, 1024 * 1024)
    if len(original) > 1024 * 1024:
        raise ValueError('PocketHome config exceeds 1 MiB')
    config = json.loads(original)
    if not isinstance(config, dict):
        raise ValueError('PocketHome config must be an object')
    pages = config.get('pages')
    if not isinstance(pages, list):
        raise ValueError('Missing PocketHome pages')
    page = next((p for p in pages if isinstance(p, dict) and p.get('name') == 'Apps'
                 and isinstance(p.get('items'), list)), None)
    if page is None or not all(isinstance(i, dict) for i in page['items']):
        raise ValueError('Missing or malformed Apps page')
    if any(c in str(home) for c in '\r\n\0"\\%'):
        raise ValueError('Unsupported home directory characters')
    session_source = source / 'vitrallis-session.py'
    safe(session_source)
    session_data = read_file(session_source)
    compile(session_data, str(session_source), 'exec')
    return config_path, original, config, page, session_data


def install(bundle, source, home):
    inputs = load_inputs(source, home)
    target = home / '.local/share/vitrallis'
    safe(home / '.pocket-home/config.json')
    safe(home / '.local/share/vitrallis-backups/.preflight')
    make_directories(target)
    stage = target / '.vitrallis-update'
    make_directories(stage, 0o700)
    if stage.stat().st_uid != os.getuid() or stage.stat().st_mode & 0o077:
        raise ValueError('Native update staging must be private to this user')
    lock_path = stage / 'lock'
    safe(lock_path)
    fd = os.open(lock_path, os.O_CREAT | os.O_RDWR | os.O_NOFOLLOW, 0o600)
    with os.fdopen(fd, 'a+') as lock:
        if os.fstat(lock.fileno()).st_nlink != 1:
            raise ValueError('Refusing hardlinked install lock')
        fcntl.flock(lock, fcntl.LOCK_EX | fcntl.LOCK_NB)
        for name in ('.current-next', '.previous-next'):
            stale = target / name
            if stale.exists() or stale.is_symlink():
                pointer(stale)  # Validate only our fixed relative generation format.
                stale.unlink()
        with tempfile.TemporaryDirectory(prefix='install-', dir=stage) as temporary:
            generation = Path(temporary) / 'generation'
            generation.mkdir(mode=0o755)
            digest = extract_bundle(bundle, generation)
            verify_versions(generation)
            install_locked(generation, digest, home, inputs)


def install_locked(generation, digest, home, inputs):
    target = home / '.local/share/vitrallis'
    config_path, original, config, page, session_data = inputs
    generations = target / 'generations'
    make_directories(generations)
    destination = generations / digest
    if destination.exists() or destination.is_symlink():
        for name in BINARIES:
            if file_digest(destination / name) != file_digest(generation / name):
                raise ValueError('Existing generation has been modified')
    else:
        generation.rename(destination)
        directory = os.open(generations, os.O_RDONLY | os.O_DIRECTORY)
        try:
            os.fsync(directory)
        finally:
            os.close(directory)
    current_link = target / 'current'
    old_pointer = pointer(current_link)
    new_pointer = 'generations/' + digest
    launch = target / 'launch'
    entry = dict(name='Vitrallis', icon='appIcons/terminal.png', shell='"' + str(launch) + '"')
    matches = [i for i in page['items'] if i.get('name') == 'Vitrallis']
    if matches and (len(matches) != 1 or matches[0].get('shell') != entry['shell']):
        raise ValueError('An unrelated Vitrallis entry exists; refusing replacement')
    if not matches:
        page['items'].append(entry)
    desktop = home / '.local/share/applications/vitrallis.desktop'
    writes = {
        target / 'vitrallis-session.py': (session_data, 0o644),
        launch: (b'#!/bin/sh\nexec /usr/bin/python3 "$HOME/.local/share/vitrallis/vitrallis-session.py" "$@"\n', 0o755),
        desktop: ((
            '[Desktop Entry]\nType=Application\nName=Vitrallis\nExec="' + str(launch) +
            '"\nTerminal=false\nCategories=System;\n').encode(), 0o644),
        config_path: ((json.dumps(config, indent=2) + '\n').encode(), stat.S_IMODE(config_path.stat().st_mode)),
    }
    receipt_path = target / 'installed.json'
    safe(receipt_path)
    receipt = json.loads(read_file(receipt_path)) if receipt_path.exists() else {}
    if not isinstance(receipt, dict):
        raise ValueError('Malformed installation receipt')
    safe(desktop)
    if desktop.exists():
        expected = receipt.get('desktop_sha256')
        if hashlib.sha256(read_file(desktop)).hexdigest() != expected:
            raise ValueError('Unmanaged or edited desktop shortcut: ' + str(desktop))
    marker = target / '.installation-pending'
    safe(marker)
    pending = json.loads(read_file(marker)) if marker.exists() else {}
    if not isinstance(pending, dict) or any(
            not isinstance(values, list) or any(v is not None and not isinstance(v, str) for v in values)
            for values in pending.values()):
        raise ValueError('Malformed installation recovery marker')
    for path in writes:
        if path.parent == target and path.exists():
            safe(path)
            if hashlib.sha256(read_file(path, 64 * 1024 * 1024)).hexdigest() not in [receipt.get(path.name)] + pending.get(path.name, []):
                raise ValueError('Unmanaged or edited installed file: ' + str(path))
    hashes = {p.name: hashlib.sha256(v[0]).hexdigest()
              for p, v in writes.items() if p.parent == target}
    hashes['desktop_sha256'] = hashlib.sha256(writes[desktop][0]).hexdigest()
    writes[receipt_path] = (json.dumps(hashes).encode(), 0o600)
    previous = {}
    for path in writes:
        safe(path)
        previous[path] = (read_file(path, 64 * 1024 * 1024), stat.S_IMODE(path.stat().st_mode)) if path.exists() else None
    backup = home / '.local/share/vitrallis-backups' / str(time.time_ns())
    safe(backup / 'paths.json')
    make_directories(backup.parent)
    backup.mkdir(mode=0o700)
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
            safe(path)
            current = read_file(path, 64 * 1024 * 1024) if path.exists() else None
            old = previous[path]
            if current != (old[0] if old else None):
                raise ValueError('Installation file changed; retry: ' + str(path))
            changed.append(path)
            atomic(path, content, mode)
        if pointer(current_link) != old_pointer:
            raise ValueError('Active build changed during installation')
        if old_pointer is not None:
            atomic_pointer(target / 'previous', old_pointer)
        atomic_pointer(current_link, new_pointer)
        marker.unlink()
    except BaseException:
        if pointer(current_link) == new_pointer and new_pointer != old_pointer:
            if old_pointer is None:
                current_link.unlink()
            else:
                atomic_pointer(current_link, old_pointer)
        for path in reversed(changed):
            safe(path)
            if not path.exists() or read_file(path, 64 * 1024 * 1024) != writes[path][0]:
                # Preserve concurrent user edits; leave the repair marker intact.
                continue
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
    print('Native bundle SHA256:', digest)


if __name__ == '__main__':
    try:
        if len(sys.argv) != 2 or os.geteuid() == 0:
            raise ValueError('Run as your normal user: install.py ARM_BUNDLE')
        install(Path(sys.argv[1]), Path(__file__).resolve().parent, Path.home())
    except (OSError, ValueError, TypeError, SyntaxError, subprocess.SubprocessError) as error:
        print('Install failed: ' + str(error), file=sys.stderr)
        sys.exit(1)
