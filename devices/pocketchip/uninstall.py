#!/usr/bin/env python3
"""Remove a managed PocketCHIP installation offline, preserving personal data."""
import argparse
import fcntl
import hashlib
import importlib.util
import json
import os
from pathlib import Path
import re
import stat
import struct
import subprocess
import sys
import tempfile

# Keep dry runs and normal removal free of persistent import caches.
sys.dont_write_bytecode = True


BINARIES = ('vitrallis', 'vitrallis-terminal', 'vitrallis-notepad', 'vitrallis-files')
MAGIC = b'VITRALLIS-BUNDLE'
HELPERS = ('install.py', 'uninstall.py', 'vitrallis-session.py')


def safe(path):
    if any(p.is_symlink() for p in (path,) + tuple(path.parents)):
        raise ValueError('Refusing symlink: ' + str(path))
    if path.exists() and path.stat().st_nlink != 1:
        raise ValueError('Refusing hardlink: ' + str(path))
    if path.exists() and not path.is_file():
        raise ValueError('Expected a regular file: ' + str(path))
    for parent in path.parents:
        if parent.exists():
            info = parent.stat()
            # /tmp is allowed only as the system's sticky temporary root.
            sticky_root = info.st_uid == 0 and info.st_mode & stat.S_ISVTX
            if info.st_uid not in (0, os.getuid()) or (info.st_mode & 0o022 and not sticky_root):
                raise ValueError('Unsafe directory ownership or permissions: ' + str(parent))
    if path.exists() and (path.stat().st_uid != os.getuid() or path.stat().st_mode & 0o7022):
        raise ValueError('Unsafe file ownership or permissions: ' + str(path))


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
    if any(p.is_symlink() for p in (path,) + tuple(path.parents)):
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


def pointer(path):
    if not path.is_symlink():
        if path.exists():
            raise ValueError('Expected a managed build pointer: ' + str(path))
        return None
    if path.lstat().st_uid != os.getuid():
        raise ValueError('Build pointer belongs to another user')
    target = os.readlink(path)
    parts = Path(target).parts
    if len(parts) != 2 or parts[0] != 'generations' or len(parts[1]) != 64 or any(c not in '0123456789abcdef' for c in parts[1]):
        raise ValueError('Invalid build pointer: ' + str(path))
    return target


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


BASE = Path('.local/share/vitrallis')
DESKTOP = Path('.local/share/applications/vitrallis.desktop')
AUTOSTART = Path('.config/autostart/vitrallis.desktop')
MENU = Path('.pocket-home/config.json')
AWESOME = Path('.config/awesome/rc.lua')
STARTUP = """-- BEGIN optional Vitrallis startup
require('gears').timer.start_new(5, function()
    require('awful').spawn({os.getenv('HOME') .. '/.local/share/vitrallis/launch'}, false)
    return false
end)
-- END optional Vitrallis startup"""
PURGE = (BASE / 'session.log', BASE / 'session.log.1',
         Path('.config/vitrallis/screen-timeout'), Path('.config/vitrallis/app-center.json'))
HEX = re.compile(r'[0-9a-f]{64}')
INSTALL_STAGE = re.compile(r'install-[a-z0-9_]{8}')
MAX_BUNDLE = 256 * 1024 * 1024 + 176


def exists(path):
    return path.exists() or path.is_symlink()


def fingerprint(path):
    if path.name in ('current', 'previous', '.current-next', '.previous-next'):
        safe(path.parent / '.path-check')
        target = pointer(path)
        return {'link': target} if target is not None else None
    safe(path)
    if not path.exists():
        return None
    info = path.stat()
    limit = MAX_BUNDLE if path.name == 'download' and path.parent.name == '.vitrallis-update' else 64 * 1024 * 1024
    if info.st_size > limit:
        raise ValueError('Managed file exceeds removal limit: ' + str(path))
    digest = hashlib.sha256()
    with path.open('rb') as stream:
        for chunk in iter(lambda: stream.read(65536), b''):
            digest.update(chunk)
    return {'sha256': digest.hexdigest(), 'mode': stat.S_IMODE(info.st_mode)}


def allowed(relative, replacement):
    path = Path(relative)
    if path.is_absolute() or path.as_posix() != relative or '..' in path.parts:
        return False
    if replacement is not None:
        return path in (MENU, AWESOME)
    if path in (DESKTOP, AUTOSTART) + PURGE:
        return True
    if path.parent == BASE:
        return path.name in HELPERS + ('launch', 'installed.json', '.installation-pending',
                                      'current', 'previous', '.current-next', '.previous-next')
    stage = BASE / '.vitrallis-update'
    if path == stage / 'download':
        return True
    if path.parent == stage / 'generation' and path.name in BINARIES:
        return True
    if (path.name in BINARIES and path.parent.name == 'generation'
            and path.parent.parent.parent == stage and INSTALL_STAGE.fullmatch(path.parent.parent.name)):
        return True
    return (len(path.parts) == len(BASE.parts) + 3 and path.parts[:len(BASE.parts)] == BASE.parts
            and path.parts[-3] == 'generations' and HEX.fullmatch(path.parts[-2]) is not None
            and path.name in BINARIES)


def load_receipt(path):
    if not exists(path):
        return None
    receipt = json.loads(read_file(path))
    return validate_receipt(receipt)


def validate_receipt(receipt):
    if not isinstance(receipt, dict) or receipt.get('schema') != 1:
        raise ValueError('Invalid installation receipt')
    for name in HELPERS + ('launch', 'desktop_sha256'):
        if not isinstance(receipt.get(name), str) or HEX.fullmatch(receipt[name]) is None:
            raise ValueError('Invalid receipt digest: ' + name)
    if not isinstance(receipt.get('menu'), dict):
        raise ValueError('Invalid menu receipt')
    return receipt


def plan(home, purge=False):
    target = home / BASE
    safe(target / '.path-check')
    receipt = load_receipt(target / 'installed.json')
    marker = target / '.installation-pending'
    pending = None
    if exists(marker):
        recovery = json.loads(read_file(marker))
        if not isinstance(recovery, dict) or recovery.get('schema') != 1:
            raise ValueError('Invalid installation recovery marker')
        pending = validate_receipt(recovery.get('receipt'))
    receipts = [r for r in (receipt, pending) if r is not None]
    expected_menu = dict(name='Vitrallis', icon='appIcons/terminal.png',
                         shell='"' + str(target / 'launch') + '"')
    if any(r['menu'] != expected_menu for r in receipts):
        raise ValueError('Receipt menu does not belong to this installation')
    if not receipts:
        generations = target / 'generations'
        safe(generations / '.path-check')
        if (any(exists(target / name) for name in ('current', 'launch'))
                or (generations.exists() and any(generations.iterdir()))):
            raise ValueError('Missing installation receipt; refusing to guess ownership')
        return []
    actions = []
    def add(path, replacement=None, expected=None):
        before = fingerprint(path)
        if expected is not None and before != expected:
            raise ValueError('File changed during removal planning: ' + str(path))
        if before is not None:
            relative = path.relative_to(home).as_posix()
            if not allowed(relative, replacement):
                raise ValueError('Unexpected managed path: ' + relative)
            actions.append({'path': relative, 'before': before,
                            'replacement': replacement.hex() if replacement is not None else None})
    def managed(path, digests):
        before = fingerprint(path)
        if before is not None:
            if before.get('sha256') in digests:
                add(path, expected=before)
            else:
                print('Preserving edited or unmanaged file:', path)
    for name in HELPERS + ('launch',):
        managed(target / name, [r[name] for r in receipts])
    for relative in (DESKTOP, AUTOSTART):
        managed(home / relative, [r['desktop_sha256'] for r in receipts])
    config_path = home / MENU
    if exists(config_path):
        original = read_file(config_path, 1024 * 1024)
        config = json.loads(original)
        if not isinstance(config, dict) or not isinstance(config.get('pages'), list):
            raise ValueError('Invalid PocketHome menu; preserve and repair it before removal')
        changed = False
        for page in config['pages']:
            if isinstance(page, dict) and isinstance(page.get('items'), list):
                keep = [item for item in page['items'] if not any(item == r['menu'] for r in receipts)]
                changed |= len(keep) != len(page['items'])
                page['items'] = keep
        if changed:
            expected = {'sha256': hashlib.sha256(original).hexdigest(),
                        'mode': stat.S_IMODE(config_path.stat().st_mode)}
            add(config_path, (json.dumps(config, indent=2) + '\n').encode(), expected)
    awesome = home / AWESOME
    if exists(awesome):
        original = read_file(awesome)
        block = STARTUP.encode()
        if original.count(block) == 1:
            expected = {'sha256': hashlib.sha256(original).hexdigest(),
                        'mode': stat.S_IMODE(awesome.stat().st_mode)}
            add(awesome, original.replace(block, b'', 1), expected)
        elif b'Vitrallis startup' in original:
            print('Preserving edited startup block:', awesome)
    generations = target / 'generations'
    safe(generations / '.path-check')
    if generations.exists():
        for generation in sorted(generations.iterdir()):
            if generation.is_symlink() or not generation.is_dir():
                raise ValueError('Unsafe generation directory: ' + str(generation))
            if HEX.fullmatch(generation.name) is None:
                print('Preserving unrecognized generation:', generation)
                continue
            whole = hashlib.sha256(MAGIC)
            paths = [generation / name for name in BINARIES]
            expected_files = {}
            if any(not exists(path) for path in paths):
                print('Preserving incomplete generation:', generation)
                continue
            for path in paths:
                digest = file_digest(path)
                expected_files[path] = {'sha256': digest.hex(), 'mode': stat.S_IMODE(path.stat().st_mode)}
                whole.update(struct.pack('<Q', path.stat().st_size) + digest)
                with path.open('rb') as stream:
                    for chunk in iter(lambda: stream.read(65536), b''):
                        whole.update(chunk)
            if whole.hexdigest() != generation.name:
                print('Preserving modified generation:', generation)
                continue
            for path in paths:
                add(path, expected=expected_files[path])
    for name in ('current', 'previous', '.current-next', '.previous-next'):
        add(target / name)
    stage = target / '.vitrallis-update'
    add(stage / 'download')
    staged = [stage / 'generation']
    staged.extend(path / 'generation' for path in stage.iterdir() if INSTALL_STAGE.fullmatch(path.name))
    for directory in staged:
        safe(directory / '.path-check')
        for name in BINARIES:
            add(directory / name)
    if purge:
        for relative in PURGE:
            add(home / relative)
    add(marker)
    add(target / 'installed.json')
    return actions


def session(home, dry_run):
    path = home / BASE / 'vitrallis-session.py'
    # Use the release's helper, not an edited installed helper. When recovering,
    # the verified copy beside this script remains available offline.
    source = helper_directory(home) / 'vitrallis-session.py'
    safe(source)
    if not source.exists():
        # A failed initial install may have written the uninstaller before its
        # session helper. Without that helper only a proven absent unit is safe.
        with tempfile.TemporaryFile() as output:
            subprocess.run(['/usr/bin/systemctl', '--user', 'show', 'vitrallis-session.service',
                            '--property=LoadState', '--value'], stdin=subprocess.DEVNULL,
                           stdout=output, stderr=output, timeout=8, check=True)
            output.seek(0)
            if output.read(128).strip() != b'not-found':
                raise ValueError('Session helper missing; cannot safely stop an existing unit')
        return False
    receipt = load_receipt(home / BASE / 'installed.json')
    if receipt is not None and hashlib.sha256(read_file(source)).hexdigest() != receipt['vitrallis-session.py']:
        raise ValueError('Edited session helper; reconcile it before stopping a session')
    if receipt is not None and exists(path) and hashlib.sha256(read_file(path)).hexdigest() != receipt['vitrallis-session.py']:
        raise ValueError('Edited installed session helper; reconcile it before stopping a session')
    spec = importlib.util.spec_from_file_location('vitrallis_session', source)
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module.stop_owned(path, dry_run=dry_run)


def helper_directory(home):
    source = Path(__file__).absolute().parent
    return source if (source / 'install.py').is_file() else home / BASE


def validate_journal(value):
    if (not isinstance(value, dict) or value.get('state') not in ('prepared', 'committed')
            or not isinstance(value.get('actions'), list) or len(value['actions']) > 4096):
        raise ValueError('Invalid removal journal')
    seen = set()
    for action in value['actions']:
        if not isinstance(action, dict) or set(action) != {'path', 'before', 'replacement'}:
            raise ValueError('Invalid removal action')
        path, before, replacement = action['path'], action['before'], action['replacement']
        if not isinstance(path, str) or not allowed(path, replacement) or path in seen:
            raise ValueError('Unsafe removal journal path')
        seen.add(path)
        if replacement is not None:
            if not isinstance(replacement, str) or len(replacement) > 4 * 1024 * 1024:
                raise ValueError('Invalid replacement data')
            bytes.fromhex(replacement)
        if not isinstance(before, dict):
            raise ValueError('Invalid removal fingerprint')
        if 'link' in before:
            if (set(before) != {'link'} or Path(path).name not in ('current', 'previous', '.current-next', '.previous-next')
                    or not re.fullmatch(r'generations/[0-9a-f]{64}', before['link'])):
                raise ValueError('Invalid removal pointer')
        elif (set(before) != {'sha256', 'mode'} or not isinstance(before['sha256'], str)
              or HEX.fullmatch(before['sha256']) is None or type(before['mode']) is not int
              or before['mode'] & ~0o755):
            raise ValueError('Invalid removal file fingerprint')
    return value


def sync(directory):
    fd = os.open(directory, os.O_RDONLY | os.O_DIRECTORY)
    try:
        os.fsync(fd)
    finally:
        os.close(fd)


def restore(home, transaction, journal):
    for index, action in reversed(list(enumerate(journal['actions']))):
        saved = transaction / str(index)
        if not exists(saved):
            continue
        path = home / action['path']
        # Validate saved files without treating numbered pointer backups as files.
        if 'link' in action['before']:
            if not saved.is_symlink() or os.readlink(saved) != action['before']['link']:
                raise ValueError('Modified removal pointer backup')
        elif fingerprint(saved) != action['before']:
            raise ValueError('Modified removal backup')
        current = fingerprint(path)
        replacement = action['replacement']
        after = None if replacement is None else {'sha256': hashlib.sha256(bytes.fromhex(replacement)).hexdigest(),
                                                   'mode': action['before']['mode']}
        if current is not None and current not in (after, action['before']):
            raise ValueError('Later edit prevents removal recovery: ' + str(path))
        os.replace(saved, path)
        sync(path.parent)
    cleanup(transaction, journal, committed=False)


def cleanup(transaction, journal, committed):
    # Only explicit numbered entries and fixed recovery helpers may be removed.
    expected = {str(i) for i in range(len(journal['actions']))} | {'journal.json'} | set(HELPERS)
    if any(p.name not in expected for p in transaction.iterdir()):
        raise ValueError('Unexpected file in removal journal; preserving recovery directory')
    for index, action in enumerate(journal['actions']):
        saved = transaction / str(index)
        if exists(saved):
            if not committed:
                raise ValueError('Unrestored removal backup')
            if 'link' in action['before']:
                if not saved.is_symlink() or os.readlink(saved) != action['before']['link']:
                    raise ValueError('Modified removal pointer backup')
            elif fingerprint(saved) != action['before']:
                raise ValueError('Modified removal backup')
            saved.unlink()
    for name in HELPERS:
        path = transaction / name
        safe(path)
        path.unlink(missing_ok=True)
    (transaction / 'journal.json').unlink()
    transaction.rmdir()


def execute(home, transaction, actions):
    journal = validate_journal({'state': 'prepared', 'actions': actions})
    make_directories(transaction, 0o700)
    atomic(transaction / 'journal.json', json.dumps(journal).encode(), 0o600)
    deferred = {BASE / name for name in ('install.py', 'uninstall.py', 'installed.json')}
    try:
        # Keep the normal removal entry and its import available until journal
        # cleanup is complete, including interruptions during committed cleanup.
        source_directory = helper_directory(home)
        for name in HELPERS:
            source = source_directory / name
            if source.exists():
                atomic(transaction / name, read_file(source), 0o600)
        for index, action in enumerate(actions):
            path = home / action['path']
            if fingerprint(path) != action['before']:
                raise ValueError('File changed before removal: ' + str(path))
            if Path(action['path']) in deferred:
                continue
            os.replace(path, transaction / str(index))
            sync(path.parent)
            sync(transaction)
            if action['replacement'] is not None:
                atomic(path, bytes.fromhex(action['replacement']), action['before']['mode'])
        journal['state'] = 'committed'
        atomic(transaction / 'journal.json', json.dumps(journal).encode(), 0o600)
    except BaseException:
        restore(home, transaction, journal)
        raise
    cleanup(transaction, journal, committed=True)
    # Receipt is last, so an interrupted cleanup can still identify the helpers.
    for name in ('install.py', 'uninstall.py', 'installed.json'):
        for action in actions:
            if Path(action['path']) == BASE / name:
                path = home / action['path']
                if fingerprint(path) != action['before']:
                    raise ValueError('Later edit prevents helper cleanup: ' + str(path))
                path.unlink()
                sync(path.parent)


def uninstall(home, dry_run=False, purge=False):
    home = home.absolute()
    target = home / BASE
    safe(target / '.path-check')
    if not target.exists():
        print('Vitrallis is already absent.')
        return
    stage = target / '.vitrallis-update'
    safe(stage / 'lock')
    if not stage.exists():
        raise ValueError('Missing update lock directory; reinstall to reconcile ownership')
    if stage.stat().st_uid != os.getuid() or stage.stat().st_mode & 0o077:
        raise ValueError('Update staging must be private to this user')
    # Keep the lock inode until all removal work ends. Never replace a live lock.
    fd = os.open(stage / 'lock', os.O_RDWR | os.O_NOFOLLOW | os.O_NONBLOCK)
    with os.fdopen(fd, 'r+') as lock:
        if not stat.S_ISREG(os.fstat(lock.fileno()).st_mode) or os.fstat(lock.fileno()).st_nlink != 1:
            raise ValueError('Unsafe update lock')
        fcntl.flock(lock, fcntl.LOCK_EX | fcntl.LOCK_NB)
        transaction = stage / 'removal'
        safe(transaction / 'journal.json')
        if transaction.exists():
            journal = validate_journal(json.loads(read_file(transaction / 'journal.json', 8 * 1024 * 1024)))
            if dry_run:
                print('Would recover pending removal:', transaction)
                return
            if journal['state'] == 'prepared':
                restore(home, transaction, journal)
            else:
                cleanup(transaction, journal, committed=True)
        actions = plan(home, purge)
        for action in actions:
            print(('Would update: ' if action['replacement'] is not None else 'Would remove: ') + str(home / action['path']))
        if not actions:
            print('No managed installation files remain.')
            return
        print('Warning: stopping Vitrallis closes its owned apps. Save unsaved work first.')
        if dry_run:
            session(home, True)
            return
        if any(Path(a['path']) not in {BASE / name for name in ('install.py', 'uninstall.py', 'installed.json')}
               for a in actions):
            session(home, False)
        execute(home, transaction, actions)
        # Empty managed directories only; unknown files are deliberately retained.
        staging_parents = set()
        for action in actions:
            path = home / action['path']
            if path.parent == stage / 'generation':
                staging_parents.add(path.parent)
            elif path.parent.parent.parent == stage and INSTALL_STAGE.fullmatch(path.parent.parent.name):
                staging_parents.update((path.parent, path.parent.parent))
        for directory in sorted(staging_parents, key=lambda p: len(p.parts), reverse=True):
            if directory.is_dir() and not directory.is_symlink() and not any(directory.iterdir()):
                directory.rmdir()
        generations = target / 'generations'
        if generations.is_dir() and not generations.is_symlink():
            for path in generations.iterdir():
                if path.is_dir() and not path.is_symlink() and HEX.fullmatch(path.name):
                    if not any(path.iterdir()):
                        path.rmdir()
            if not any(generations.iterdir()):
                generations.rmdir()
    # Retaining this tiny lock prevents a waiting process from using an orphaned
    # inode concurrently with a new installation. It contains no personal data.
    print('Removed managed Vitrallis files. Marshmallow remains available.')
    print('Retained: apps/, app-center/ transactions, user documents, installation backups,')
    print('unrecognized or edited files, custom XDG locations, and .vitrallis-update/lock.')
    if not purge:
        print('Preferences and session logs are retained; --purge removes only the four documented data files.')


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--dry-run', action='store_true', help='Inspect without stopping sessions or changing files')
    parser.add_argument('--purge', action='store_true', help='Also remove Vitrallis preferences and session logs; asks for confirmation')
    args = parser.parse_args()
    if os.geteuid() == 0:
        raise ValueError('Run as the normal desktop user, without sudo')
    if args.purge and not args.dry_run:
        print('Purge deletes the default Vitrallis preferences and session logs, in addition to uninstalling.')
        if input('Type PURGE to confirm (anything else cancels): ') != 'PURGE':
            print('Cancelled.')
            return
    uninstall(Path.home(), args.dry_run, args.purge)


if __name__ == '__main__':
    try:
        main()
    except (OSError, ValueError, TypeError, KeyError, RuntimeError, EOFError, subprocess.SubprocessError) as error:
        print('Vitrallis removal failed: ' + str(error), file=sys.stderr)
        sys.exit(1)
