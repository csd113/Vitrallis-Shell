#!/usr/bin/env python3
"""Create a complete, reproducible source archive of the Cargo workspace."""
import argparse
import gzip
import json
import os
from pathlib import Path
import subprocess
import tarfile
import tempfile

ROOT = Path(__file__).resolve().parents[1]


def sources():
    metadata = json.loads(subprocess.check_output(
        ['cargo', 'metadata', '--no-deps', '--locked', '--format-version', '1'], cwd=ROOT))
    files = set()
    for package in metadata['packages']:
        if package['id'] not in metadata['workspace_members']:
            continue
        directory = Path(package['manifest_path']).parent
        listing = subprocess.check_output([
            'cargo', 'package', '--list', '--allow-dirty', '--locked', '--offline',
            '--manifest-path', str(directory / 'Cargo.toml')], cwd=ROOT, text=True)
        for name in listing.splitlines():
            if name in ('.cargo_vcs_info.json', 'Cargo.toml.orig'):
                continue
            path = directory / name
            # Cargo describes generated lockfiles for member packages; the workspace
            # archive retains the actual root lockfile and original manifests.
            if name == 'Cargo.lock' and not path.exists():
                continue
            if path.is_symlink() or not path.is_file() or not path.resolve().is_relative_to(ROOT):
                raise ValueError('Unsafe source archive input: ' + str(path))
            files.add(path.relative_to(ROOT))
    return sorted(files)


def package(output):
    files = sources()
    output = output.absolute()
    if output.exists() or output.is_symlink():
        raise ValueError('Output already exists')
    output.parent.mkdir(parents=True, exist_ok=True)
    with tempfile.TemporaryDirectory(prefix='.vitrallis-source-', dir=output.parent) as temporary:
        archive = Path(temporary) / 'source.tar.gz'
        with archive.open('xb') as raw, gzip.GzipFile(fileobj=raw, mode='wb', filename='', mtime=0) as compressed, tarfile.open(fileobj=compressed, mode='w') as tar:
            for path in files:
                source = ROOT / path
                info = tar.gettarinfo(str(source), arcname='Vitrallis-Shell/' + path.as_posix())
                info.uid = info.gid = info.mtime = 0
                info.uname = info.gname = ''
                with source.open('rb') as stream:
                    tar.addfile(info, stream)
        # Publish exclusively; the temporary link is removed with the staging directory.
        os.link(archive, output)


if __name__ == '__main__':
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--output', type=Path, required=True)
    args = parser.parse_args()
    try:
        package(args.output)
    except (OSError, ValueError, subprocess.SubprocessError) as error:
        parser.exit(1, f'Source packaging failed: {error}\n')
