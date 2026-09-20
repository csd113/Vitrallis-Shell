#!/usr/bin/env python3
"""Download a complete, compatible ARMv7 Linux release and its matching helpers."""
import argparse
from functools import cmp_to_key
import hashlib
import importlib.util
import json
import os
from pathlib import Path
import pwd
import re
import resource
import shutil
import stat
import subprocess
import sys
import tempfile
import time

REPOSITORY = 'csd113/Vitrallis-Shell'
API = 'https://api.github.com/repos/' + REPOSITORY + '/releases'
DOWNLOAD = 'https://github.com/' + REPOSITORY + '/releases/download/'
BUNDLE = 'vitrallis-armv7-unknown-linux-gnueabihf-glibc2.36-v2.vtrbundle'
HELPERS = ('install-session.py', 'uninstall.py', 'vitrallis-session.py', 'platform-setup.py', 'media-setup.py')
MAX_BUNDLE = 320 * 1024 * 1024 + 216
VERSION = re.compile(r'v(0|[1-9][0-9]*)\.(0|[1-9][0-9]*)\.(0|[1-9][0-9]*)(?:-([0-9A-Za-z-]+(?:\.[0-9A-Za-z-]+)*))?(?:\+([0-9A-Za-z-]+(?:\.[0-9A-Za-z-]+)*))?')


def command_output(args, env, timeout=5):
    """Bound diagnostics from session commands, including a broken desktop."""
    def limits():
        resource.setrlimit(resource.RLIMIT_FSIZE, (65536, 65536))
        resource.setrlimit(resource.RLIMIT_CORE, (0, 0))
    with tempfile.TemporaryFile() as output:
        result = subprocess.run(args, stdin=subprocess.DEVNULL, stdout=output,
                                stderr=output, timeout=timeout, env=env,
                                preexec_fn=limits, check=False)
        output.seek(0)
        message = output.read(65537).decode('utf-8', errors='replace')
    if result.returncode or len(message) > 65536:
        raise ValueError('Command failed: ' + args[0] + ': ' + message[-2048:].strip())
    return message.strip()


def installation_environment():
    """Use the desktop account's existing manager even from an SSH login."""
    uid = os.getuid()
    if uid == 0 or os.geteuid() != uid:
        raise ValueError('Run as your normal desktop user, without sudo')
    if os.uname().sysname != 'Linux' or os.uname().machine not in ('armv7l', 'armv8l'):
        raise ValueError('Requires a PocketCHIP running 32-bit ARMv7 Debian')
    if b'nextthing,pocketchip' not in Path('/sys/firmware/devicetree/base/compatible').read_bytes().split(b'\0'):
        raise ValueError('Requires a PocketCHIP')
    if Path.home().resolve() != Path(pwd.getpwuid(uid).pw_dir).resolve():
        raise ValueError('HOME must be your normal desktop account home directory')
    runtime = Path('/run/user') / str(uid)
    try:
        directory = runtime.lstat()
        bus = (runtime / 'bus').lstat()
    except FileNotFoundError as error:
        raise ValueError('Log into the PocketCHIP desktop first; its user manager is unavailable') from error
    if (not stat.S_ISDIR(directory.st_mode) or directory.st_uid != uid
            or stat.S_IMODE(directory.st_mode) != 0o700
            or not stat.S_ISSOCK(bus.st_mode) or bus.st_uid != uid):
        raise ValueError('No safe desktop user bus; log into the PocketCHIP desktop first')
    env = dict(os.environ, XDG_RUNTIME_DIR=str(runtime),
               DBUS_SESSION_BUS_ADDRESS='unix:path=' + str(runtime / 'bus'))
    state = command_output(['/usr/bin/systemctl', '--user', 'show',
                            'vitrallis-session.service', '--property=ActiveState', '--value'], env)
    if state not in ('inactive', 'failed'):
        raise ValueError('Save your work and exit the existing Vitrallis session before installing')
    return env


def check_download_space(release, directory):
    sizes = [asset.get('size') for asset in release['assets'] if asset.get('name') == BUNDLE]
    if len(sizes) != 1 or type(sizes[0]) is not int or not 0 < sizes[0] <= MAX_BUNDLE:
        raise ValueError('Invalid bundle size')
    # Account for both the download and staged native generation on shared disks.
    required = 2 * sizes[0] + 16 * 1024 * 1024
    for path in (directory, Path.home()):
        if shutil.disk_usage(path).free < required:
            raise ValueError('Insufficient free space at {}: keep at least {} MiB free'.format(
                path, (required + 1048575) // 1048576))


def load_session(directory):
    # Called only after every downloaded helper's size and digest were verified.
    spec = importlib.util.spec_from_file_location('vitrallis_release_session', directory / 'vitrallis-session.py')
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module


def desktop_available(env):
    if any(env.get(key) for key in ('SSH_CONNECTION', 'SSH_CLIENT', 'SSH_TTY')):
        return False
    if not re.fullmatch(r':[0-9]+(?:\.[0-9]+)?', env.get('DISPLAY', '')):
        return False
    if any(not env.get(key) or any(c in env[key] for c in '\r\n\0')
           for key in ('XAUTHORITY', 'DBUS_SESSION_BUS_ADDRESS')):
        return False
    try:
        return 'vitrallis-desktop-available' in command_output(
            ['/usr/bin/awesome-client', 'return "vitrallis-desktop-available"'], env)
    except (OSError, ValueError, subprocess.SubprocessError):
        return False


def wait_for_desktop(session, script, env, identity):
    """Require a visible shell window belonging to this supervised generation."""
    deadline = time.monotonic() + 20
    binary = (script.parent / 'current/vitrallis').resolve(strict=True)
    query = '''local windows = {}
    for _, c in ipairs(client.get()) do
        if c.name == "Vitrallis" and not c.hidden and not c.minimized and c:isvisible() then
            table.insert(windows, "vitrallis-ready:" .. tostring(c.pid))
        end
    end
    return table.concat(windows, ",")'''
    while time.monotonic() < deadline:
        owner = session.owned_session(script)
        if owner is None or owner != identity:
            raise ValueError('The Vitrallis session exited or changed during startup')
        output = command_output(['/usr/bin/awesome-client', query], env,
                                timeout=min(2, max(0.1, deadline - time.monotonic())))
        for pid in re.findall(r'vitrallis-ready:([1-9][0-9]*)', output):
            proc = Path('/proc') / pid
            try:
                parent = (proc / 'stat').read_text().rsplit(')', 1)[1].split()[1]
                if (proc.stat().st_uid == os.getuid() and parent == owner[0]
                        and (proc / 'exe').resolve(strict=True) == binary
                        and session.owned_session(script) == owner):
                    return
            except FileNotFoundError:
                pass
        time.sleep(0.2)
    raise ValueError('No visible Vitrallis window appeared within 20 seconds')


def finish_install(directory):
    target = Path.home() / '.local/share/vitrallis'
    launch = target / 'launch'
    print('Vitrallis installed successfully.', flush=True)
    if not desktop_available(os.environ):
        print('To open it on your PocketCHIP, open Terminal on the device and run:\n'
              '~/.local/share/vitrallis/launch\n'
              'Automatic startup at boot has not been enabled.', flush=True)
        return
    session = load_session(directory)
    script = target / 'vitrallis-session.py'
    print('Opening Vitrallis on the PocketCHIP display...', flush=True)
    # Do not stop a session another process started after installation completed.
    if session.owned_session(script) is not None:
        raise ValueError('A session has already started; installation succeeded, but automatic launch was skipped')
    identity = None
    try:
        command_output([str(launch)], os.environ, timeout=10)
        identity = session.owned_session(script)
        if identity is None:
            raise ValueError('The Vitrallis session exited during startup')
        wait_for_desktop(session, script, os.environ, identity)
    except (OSError, ValueError, subprocess.SubprocessError) as error:
        cleanup = ''
        if identity is not None:
            try:
                if session.owned_session(script) == identity:
                    session.stop_owned(script)
                else:
                    cleanup = '\nThe session changed; it was left running.'
            except (OSError, ValueError, RuntimeError, subprocess.SubprocessError) as stop_error:
                cleanup = '\nSession cleanup also failed: ' + str(stop_error)
        raise ValueError('Installation succeeded, but launch failed: {}.\n'
                         'Inspect {} and retry {} from the device Terminal.{}'.format(
                             error, target / 'session.log', launch, cleanup)) from error
    print('Vitrallis is open. Home returns from an app; Exit Vitrallis returns to your original desktop.\n'
          'Automatic startup at boot has not been enabled. Any GPU reboot notice above still applies.', flush=True)


def fetch(url, destination, limit, timeout=30):
    """Curl also bounds chunked responses through the child's file-size limit."""
    if not url.startswith('https://'):
        raise ValueError('Download must use HTTPS')
    def limits():
        resource.setrlimit(resource.RLIMIT_CORE, (0, 0))
        resource.setrlimit(resource.RLIMIT_FSIZE, (limit, limit))
    # The installer rejects group-writable inputs. Create private downloads
    # even when the desktop user's umask is 002, retaining exclusive creation.
    fd = os.open(destination, os.O_WRONLY | os.O_CREAT | os.O_EXCL, 0o600)
    with os.fdopen(fd, 'wb') as output, tempfile.TemporaryFile() as errors:
        result = subprocess.run([
            '/usr/bin/curl', '-q', '--fail', '--silent', '--show-error', '--location',
            '--proto', '=https', '--proto-redir', '=https', '--max-redirs', '5',
            '--connect-timeout', '10', '--max-time', str(timeout),
            '--speed-limit', '1024', '--speed-time', '15', '--max-filesize', str(limit),
            '--header', 'Accept: application/vnd.github+json', url,
        ], stdin=subprocess.DEVNULL, stdout=output, stderr=errors,
            timeout=timeout + 5, preexec_fn=limits, check=False)
        if result.returncode:
            errors.seek(0)
            detail = errors.read(2048).decode('utf-8', errors='replace').strip()
            raise ValueError('HTTPS download failed: ' + url + ': ' + detail)
    if not 0 < destination.stat().st_size <= limit:
        raise ValueError('Empty or oversized download: ' + url)


def version(tag):
    match = VERSION.fullmatch(tag) if isinstance(tag, str) else None
    if match is None or len(tag) > 128:
        raise ValueError('Invalid release version')
    major, minor, patch, pre, _ = match.groups()
    identifiers = pre.split('.') if pre else []
    if any(v.isdigit() and len(v) > 1 and v.startswith('0') for v in identifiers):
        raise ValueError('Invalid numeric prerelease version')
    return tuple(map(int, (major, minor, patch))), identifiers


def compare(left, right):
    a, ap = version(left['tag_name'])
    b, bp = version(right['tag_name'])
    if a != b:
        return (a > b) - (a < b)
    if not ap or not bp:
        return (not ap) - (not bp)
    for a, b in zip(ap, bp):
        if a == b:
            continue
        if a.isdigit() and b.isdigit():
            return (int(a) > int(b)) - (int(a) < int(b))
        if a.isdigit() != b.isdigit():
            return -1 if a.isdigit() else 1
        return (a > b) - (a < b)
    return (len(ap) > len(bp)) - (len(ap) < len(bp))


def releases(directory):
    found = []
    for page in range(1, 6):
        path = directory / ('releases-' + str(page) + '.json')
        fetch(API + '?per_page=30&page=' + str(page), path, 2 * 1024 * 1024)
        values = json.loads(path.read_bytes())
        if not isinstance(values, list) or len(values) > 30 or any(not isinstance(v, dict) for v in values):
            raise ValueError('Malformed GitHub release listing')
        found.extend(values)
        if len(values) < 30:
            return found
    raise ValueError('Release listing exceeds 150 entries; use a reviewed release staging directory')


def select(values, stable=False, tag=None):
    candidates = []
    for value in values:
        if value.get('draft') is not False or not value.get('published_at'):
            continue
        if not isinstance(value.get('prerelease'), bool):
            raise ValueError('Malformed release channel')
        try:
            _, pre = version(value.get('tag_name'))
        except ValueError:
            continue
        if stable and (value['prerelease'] or pre):
            continue
        if tag is not None and value['tag_name'] != tag:
            continue
        assets = value.get('assets')
        if not isinstance(assets, list) or any(not isinstance(a, dict) for a in assets):
            raise ValueError('Malformed release assets')
        # This installer artifact identifies the current session contract.
        # A bundle alone does not establish compatible installation helpers.
        names = {a.get('name') for a in assets if isinstance(a.get('name'), str)}
        if BUNDLE in names and 'install-session.py' in names:
            candidates.append(value)
    if not candidates:
        raise ValueError('No compatible published ARMv7 Linux bundle release. A maintainer must publish the complete bundle with install-session.py and its matching helpers.')
    return sorted(candidates, key=cmp_to_key(compare), reverse=True)[0]


def download_release(release, directory):
    tag = release['tag_name']
    version(tag)
    prefix = DOWNLOAD + tag + '/'
    planned = []
    for name in (BUNDLE,) + HELPERS:
        for filename, limit in ((name, MAX_BUNDLE if name == BUNDLE else 256 * 1024),
                                (name + '.sha256', 256)):
            matches = [a for a in release['assets'] if a.get('name') == filename]
            if len(matches) != 1:
                raise ValueError('Missing or duplicate asset in ' + tag + ': ' + filename)
            asset = matches[0]
            if (asset.get('state') != 'uploaded'
                    or type(asset.get('size')) is not int or not 0 < asset['size'] <= limit
                    or asset.get('browser_download_url') != prefix + filename):
                raise ValueError('Invalid release asset: ' + filename)
            planned.append((filename, asset, limit))
    # Validate the complete inventory before downloading any executable helper.
    for filename, asset, limit in planned:
        path = directory / filename
        fetch(asset['browser_download_url'], path, limit, 180 if filename == BUNDLE else 30)
        if path.stat().st_size != asset['size']:
            raise ValueError('Release asset length mismatch: ' + filename)
    for name in (BUNDLE,) + HELPERS:
        sidecar = (directory / (name + '.sha256')).read_text(encoding='ascii')
        match = re.fullmatch(r'([0-9a-f]{64})  ' + re.escape(name) + r'\n?', sidecar)
        if match is None:
            raise ValueError('Invalid SHA-256 sidecar: ' + name)
        digest = hashlib.sha256()
        with (directory / name).open('rb') as stream:
            for chunk in iter(lambda: stream.read(65536), b''):
                digest.update(chunk)
        if digest.hexdigest() != match.group(1):
            raise ValueError('SHA-256 mismatch: ' + name)
        asset = next(a for a in release['assets'] if a['name'] == name)
        if asset.get('digest') is not None and asset['digest'] != 'sha256:' + digest.hexdigest():
            raise ValueError('GitHub digest disagrees with sidecar: ' + name)
    return directory / BUNDLE


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--stable', action='store_true', help='Exclude beta/prerelease releases')
    parser.add_argument('--release', help='Select one published v-prefixed version')
    args = parser.parse_args()
    if sys.version_info < (3, 8) or os.geteuid() == 0:
        raise ValueError('Use Python 3.8+ as the normal desktop user, without sudo')
    if args.release:
        version(args.release)
    env = installation_environment()
    with tempfile.TemporaryDirectory(prefix='vitrallis-download-') as temporary:
        directory = Path(temporary).resolve()
        release = select(releases(directory), args.stable, args.release)
        print('Selected release:', release['tag_name'], flush=True)
        check_download_space(release, directory)
        bundle = download_release(release, directory)
        subprocess.run([sys.executable, '-I', str(directory / 'install-session.py'), str(bundle),
                        '--expected-version', release['tag_name'][1:]], check=True, env=env)
        finish_install(directory)


if __name__ == '__main__':
    try:
        main()
    except (OSError, ValueError, TypeError, KeyError, subprocess.SubprocessError) as error:
        print('Vitrallis setup failed: ' + str(error), file=sys.stderr)
        sys.exit(1)
