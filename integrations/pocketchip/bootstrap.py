#!/usr/bin/env python3
"""Download a complete, compatible ARMv7 Linux release and its matching helpers."""
import argparse
from functools import cmp_to_key
import hashlib
import json
import os
from pathlib import Path
import re
import resource
import subprocess
import sys
import tempfile

REPOSITORY = 'csd113/Vitrallis-Shell'
API = 'https://api.github.com/repos/' + REPOSITORY + '/releases'
DOWNLOAD = 'https://github.com/' + REPOSITORY + '/releases/download/'
BUNDLE = 'vitrallis-armv7-unknown-linux-gnueabihf-glibc2.36.vtrbundle'
HELPERS = ('install-session.py', 'uninstall.py', 'vitrallis-session.py')
MAX_BUNDLE = 256 * 1024 * 1024 + 176
VERSION = re.compile(r'v(0|[1-9][0-9]*)\.(0|[1-9][0-9]*)\.(0|[1-9][0-9]*)(?:-([0-9A-Za-z-]+(?:\.[0-9A-Za-z-]+)*))?(?:\+([0-9A-Za-z-]+(?:\.[0-9A-Za-z-]+)*))?')


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
    with tempfile.TemporaryDirectory(prefix='vitrallis-download-') as temporary:
        directory = Path(temporary).resolve()
        release = select(releases(directory), args.stable, args.release)
        print('Selected release:', release['tag_name'], flush=True)
        bundle = download_release(release, directory)
        subprocess.run([sys.executable, str(directory / 'install-session.py'), str(bundle),
                        '--expected-version', release['tag_name'][1:]], check=True)


if __name__ == '__main__':
    try:
        main()
    except (OSError, ValueError, TypeError, KeyError, subprocess.SubprocessError) as error:
        print('Vitrallis installation failed: ' + str(error), file=sys.stderr)
        sys.exit(1)
