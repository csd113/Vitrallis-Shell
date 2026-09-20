#!/usr/bin/python3
"""Stage Vitrallis in the user's home and add an opt-in desktop shortcut."""
import fcntl
import argparse
import ctypes
import hashlib
import json
import os
import platform
import pwd
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


# The self-contained remover owns the shared filesystem guards so it remains
# runnable after the installer helper has been removed during final cleanup.
sys.dont_write_bytecode = True
sys.path.insert(0, str(Path(__file__).resolve().parent))
try:
    from uninstall import BINARIES, HELPERS, MAGIC, atomic, file_digest, make_directories, pointer, read_file, safe, validate_receipt
except ImportError as error:
    raise SystemExit('Keep uninstall.py beside install-session.py: ' + str(error)) from error


def preflight():
    if os.geteuid() == 0:
        raise ValueError('Run as your normal desktop user, without sudo')
    if platform.system() != 'Linux' or platform.machine() not in ('armv7l', 'armv8l'):
        raise ValueError('This bundle requires 32-bit ARMv7 Linux (armhf)')
    release = Path('/etc/os-release').read_text()
    fields = dict(line.split('=', 1) for line in release.splitlines() if '=' in line)
    if (fields.get('ID', '').strip('"') != 'debian'
            or not re.fullmatch(r'[0-9]+', fields.get('VERSION_ID', '').strip('"'))
            or int(fields['VERSION_ID'].strip('"')) < 12):
        raise ValueError('Requires Debian 12 or newer; original Jessie images are unsupported')
    if subprocess.check_output(['/usr/bin/dpkg', '--print-architecture'], timeout=5, text=True).strip() != 'armhf':
        raise ValueError('Requires the Debian ARM hard-float ABI (armhf)')
    libc = subprocess.check_output(['/usr/bin/getconf', 'GNU_LIBC_VERSION'], timeout=5, text=True)
    match = re.fullmatch(r'glibc ([0-9]+)\.([0-9]+)\s*', libc)
    if match is None or tuple(map(int, match.groups())) < (2, 36):
        raise ValueError('Requires glibc 2.36 or newer')
    class Version(ctypes.Structure):
        _fields_ = [('major', ctypes.c_uint8), ('minor', ctypes.c_uint8), ('patch', ctypes.c_uint8)]
    sdl = ctypes.CDLL('libSDL2-2.0.so.0')
    sdl.SDL_GetVersion.argtypes = [ctypes.POINTER(Version)]
    sdl.SDL_GetVersion.restype = None
    version = Version()
    sdl.SDL_GetVersion(ctypes.byref(version))
    if (version.major, version.minor, version.patch) < (2, 26, 5):
        raise ValueError('Requires SDL2 2.26.5 or newer')
    for name in ('python3', 'curl', 'systemctl', 'systemd-run', 'awesome', 'awesome-client', 'picom', 'bwrap'):
        if not os.access('/usr/bin/' + name, os.X_OK):
            raise ValueError('Missing runtime prerequisite: /usr/bin/' + name)
    awesome = subprocess.check_output(['/usr/bin/awesome', '--version'], timeout=5, text=True)
    if re.search(r'awesome v4\.', awesome) is None:
        raise ValueError('Requires Awesome 4.x')
    require_stopped_session()
    graphics_advice()


def graphics_advice():
    """Optional upstream graphics components must never block software installs."""
    packages = (('libEGL.so.1', 'libegl1 and libegl-mesa0'),
                ('libGLESv2.so.2', 'libgles2'), ('libdrm.so.2', 'libdrm2'))
    for library, package in packages:
        try:
            ctypes.CDLL(library)
        except OSError as error:
            print('Graphics warning: {} unavailable ({}); Debian packages: {}. '
                  'Mesa DRI drivers are provided by libgl1-mesa-dri. '
                  'Software rendering remains available.'.format(library, error, package), file=sys.stderr)
    if not Path('/dev/dri').is_dir():
        print('Graphics warning: /dev/dri unavailable; installation can continue with software rendering.', file=sys.stderr)
    print('Optional graphics check from the desktop session after installation: '
          '~/.local/share/vitrallis/current/vitrallis --graphics-test --renderer auto')


def require_stopped_session():
    state = subprocess.check_output(['/usr/bin/systemctl', '--user', 'show',
                                     'vitrallis-session.service', '--property=ActiveState', '--value'],
                                    timeout=5, text=True).strip()
    if state not in ('inactive', 'failed'):
        raise ValueError('Close the existing Vitrallis session before installing')


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


def limit_probe_output():
    # Installer is single-threaded. Bound the child's regular stdout/stderr file
    # before exec, so a broken --version cannot fill memory or the staging disk.
    resource.setrlimit(resource.RLIMIT_CORE, (0, 0))
    resource.setrlimit(resource.RLIMIT_FSIZE, (4096, 4096))


def verify_versions(generation, expected=None):
    for name in BINARIES:
        with tempfile.TemporaryFile() as output:
            subprocess.run([str(generation / name), '--version'], stdin=subprocess.DEVNULL,
                           stdout=output, stderr=output, timeout=5, check=True,
                           preexec_fn=limit_probe_output)
            output.seek(0)
            text = output.read(4097).decode('ascii').strip()
        if name == 'arti':
            if text.splitlines()[:1] != ['Arti 2.6.0']:
                raise ValueError('Bundled executable version mismatch: arti')
            continue
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
            or flags & 0xff000000 != 0x05000000 or flags & 0x600 != 0x400):
        raise ValueError('Expected a complete ARM EABI5 hard-float executable')


def load_inputs(source, home):
    if any(c in str(home) for c in '\r\n\0"\\%'):
        raise ValueError('Unsupported home directory characters')
    helpers = {}
    for name in HELPERS:
        path = source / name
        helpers[name] = read_file(path)
        compile(helpers[name], str(path), 'exec')
    return helpers


def install(bundle, source, home, expected_version=None):
    preflight()
    inputs = load_inputs(source, home)
    target = home / '.local/share/vitrallis'
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
        require_stopped_session()
        if (stage / 'removal').exists() or (stage / 'removal').is_symlink():
            raise ValueError('Uninstall recovery is pending; rerun the local uninstaller first')
        for name in ('.current-next', '.previous-next'):
            stale = target / name
            if stale.exists() or stale.is_symlink():
                pointer(stale)  # Validate only our fixed relative generation format.
                stale.unlink()
        with tempfile.TemporaryDirectory(prefix='install-', dir=stage) as temporary:
            generation = Path(temporary) / 'generation'
            generation.mkdir(mode=0o755)
            digest = extract_bundle(bundle, generation)
            verify_versions(generation, expected_version)
            setup_platform(source)
            install_locked(generation, digest, home, inputs)


def setup_platform(source):
    """Privilege is confined to explicit platform provisioning, never the shell."""
    compatible = Path('/sys/firmware/devicetree/base/compatible')
    if not compatible.exists() or b'nextthing,pocketchip' not in compatible.read_bytes().split(b'\0'):
        return
    print('Configuring PocketCHIP GPU OPP and private utilization access. '
          'The system may request your administrator password.', flush=True)
    subprocess.run(['/usr/bin/sudo', '--', '/usr/bin/python3', '-I',
                    str(source / 'platform-setup.py'), '--install-user', pwd.getpwuid(os.getuid()).pw_name],
                   check=True)
    print('Enabling the fixed FFmpeg installation action for Media Carousel. '
          'No multimedia packages are installed until requested in the app.', flush=True)
    subprocess.run(['/usr/bin/sudo', '--', '/usr/bin/python3', '-I',
                    str(source / 'media-setup.py'), '--user', pwd.getpwuid(os.getuid()).pw_name],
                   check=True)
    state = Path('/var/lib/vitrallis-pocketchip/gpu-status.json')
    status = json.loads(state.read_bytes())
    if status.get('reboot_required') is True:
        print('REBOOT REQUIRED: restart PocketCHIP to activate GPU utilization support.', flush=True)
    elif status.get('trace_configured') is not True:
        print('GPU utilization is unavailable: ' + status.get('trace_error', 'inspect platform setup status'), flush=True)


def tor_defaults(home, writes):
    """Validate all owned service paths before the installation transaction."""
    root = home / '.local/share/vitrallis/tor'
    directories = [root, root / 'cache', root / 'state']
    for path in directories:
        safe(path / ".permission-check")
        if path.exists() and (not path.is_dir() or path.stat().st_uid != os.getuid()
                              or stat.S_IMODE(path.stat().st_mode) != 0o700):
            raise ValueError('Tor directories must be private (0700): ' + str(path))
    config = home / '.config/vitrallis/tor.json'
    safe(config)
    if config.exists():
        def unique(pairs):
            result = {}
            for key, value in pairs:
                if key in result: raise ValueError('Duplicate Tor setting')
                result[key] = value
            return result
        value = json.loads(read_file(config, 4096), object_pairs_hook=unique)
        if (not isinstance(value, dict) or set(value) != {'startup'}
                or value['startup'] not in ('on-demand', 'always-on', 'disabled')):
            raise ValueError('Malformed Tor startup configuration')
        if stat.S_IMODE(config.stat().st_mode) != 0o600:
            raise ValueError('Tor startup configuration must be private (0600)')
    else:
        writes[config] = (b'{"startup":"on-demand"}\n', 0o600)
    return directories


def install_locked(generation, digest, home, inputs):
    target = home / '.local/share/vitrallis'
    helpers = inputs
    generations = target / 'generations'
    make_directories(generations)
    destination = generations / digest
    if destination.exists() or destination.is_symlink():
        for name in BINARIES:
            if file_digest(destination / name) != file_digest(generation / name):
                raise ValueError('Existing generation has been modified')
    current_link = target / 'current'
    old_pointer = pointer(current_link)
    old_previous = pointer(target / 'previous')
    new_pointer = 'generations/' + digest
    launch = target / 'launch'
    desktop = home / '.local/share/applications/vitrallis.desktop'
    writes = {
        **{target / name: (data, 0o644) for name, data in helpers.items()},
        launch: (b'#!/bin/sh\nexec /usr/bin/python3 "$HOME/.local/share/vitrallis/vitrallis-session.py" "$@"\n', 0o755),
        desktop: ((
            '[Desktop Entry]\nType=Application\nName=Vitrallis\nExec="' + str(launch) +
            '"\nTerminal=false\nCategories=System;\n').encode(), 0o644),
    }
    tor_directories = tor_defaults(home, writes)
    receipt_path = target / 'installed.json'
    safe(receipt_path)
    receipt = json.loads(read_file(receipt_path)) if receipt_path.exists() else {}
    if not isinstance(receipt, dict):
        raise ValueError('Malformed installation receipt')
    if receipt:
        # Require the current helper inventory; do not silently adopt an obsolete layout.
        validate_receipt(receipt)
    marker = target / '.installation-pending'
    safe(marker)
    recovery = json.loads(read_file(marker)) if marker.exists() else {}
    if not isinstance(recovery, dict) or (recovery and recovery.get('schema') != 1):
        raise ValueError('Malformed installation recovery marker')
    recovery_receipt = recovery.get('receipt', {})
    if not isinstance(recovery_receipt, dict):
        raise ValueError('Malformed installation recovery receipt')
    if recovery_receipt:
        validate_receipt(recovery_receipt)
    safe(desktop)
    if desktop.exists():
        expected = [receipt.get('desktop_sha256'), recovery_receipt.get('desktop_sha256')]
        if hashlib.sha256(read_file(desktop)).hexdigest() not in expected:
            raise ValueError('Unmanaged or edited desktop shortcut: ' + str(desktop))
    pending = recovery.get('hashes', {})
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
    hashes['schema'] = 1
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
    atomic(marker, json.dumps({'schema': 1, 'receipt': hashes, 'hashes': {
        p.name: [hashlib.sha256(old[0]).hexdigest() if old else None,
                 hashlib.sha256(writes[p][0]).hexdigest()]
        for p, old in previous.items() if p.parent == target}}).encode(), 0o600)
    changed = []
    created_tor = []
    try:
        for directory in tor_directories:
            if not directory.exists():
                make_directories(directory, 0o700)
                created_tor.append(directory)
        if not destination.exists():
            generation.rename(destination)
            directory = os.open(generations, os.O_RDONLY | os.O_DIRECTORY)
            try:
                os.fsync(directory)
            finally:
                os.close(directory)
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
        if pointer(target / 'previous') != old_previous:
            if old_previous is None:
                (target / 'previous').unlink(missing_ok=True)
            else:
                atomic_pointer(target / 'previous', old_previous)
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
        for directory in reversed(created_tor):
            try: directory.rmdir()
            except OSError: pass  # Preserve any concurrent service data.
        raise
    print('Installed:', target)
    print('Backups:', backup)
    print('Launch:', launch)
    print('The original session and menu are unchanged. Run the launch command above.')
    print('Native bundle SHA256:', digest)
    print('Offline uninstall: python3 "' + str(target / 'uninstall.py') + '"')


if __name__ == '__main__':
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('bundle', type=Path)
    parser.add_argument('--expected-version')
    args = parser.parse_args()
    try:
        install(args.bundle, Path(__file__).resolve().parent, Path.home(), args.expected_version)
    except (OSError, ValueError, TypeError, SyntaxError, subprocess.SubprocessError) as error:
        print('Install failed: ' + str(error), file=sys.stderr)
        sys.exit(1)
