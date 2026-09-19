#!/usr/bin/env python3
"""Package the complete native Vitrallis build and a SHA-256 sidecar."""
import argparse
import hashlib
import json
from pathlib import Path
import struct
import subprocess
import tempfile

TARGETS = {'x86_64-unknown-linux-gnu': (2, 62),
           'aarch64-unknown-linux-gnu': (2, 183),
           'armv7-unknown-linux-gnueabihf': (1, 40)}
BINARIES = ('vitrallis', 'vitrallis-terminal', 'vitrallis-notepad', 'vitrallis-files')
MAGIC = b'VITRALLIS-BUNDLE'
ROOT = Path(__file__).resolve().parents[1]
SESSION_HELPERS = ('bootstrap.py', 'install-session.py', 'uninstall.py', 'vitrallis-session.py', 'platform-setup.py', 'media-setup.py')


def inventory(directory, target, version, runner):
    entries = []
    elf_class, machine = TARGETS[target]
    for name in BINARIES:
        binary = directory / name
        if binary.is_symlink() or not binary.is_file() or not 64 <= binary.stat().st_size <= 64 * 1024 * 1024:
            raise ValueError('Missing or invalid bundled executable: ' + name)
        with binary.open('rb') as stream:
            header = stream.read(64)
            if (header[:4] != b'\x7fELF' or header[4:7] != bytes([elf_class, 1, 1])
                    or int.from_bytes(header[16:18], 'little') not in (2, 3)
                    or int.from_bytes(header[18:20], 'little') != machine):
                raise ValueError('Executable does not match the release target: ' + name)
            if target == 'armv7-unknown-linux-gnueabihf':
                _, _, elf_version, _, phoff, _, flags, ehsize, phsize, phcount = struct.unpack_from('<HHIIIIIHHH', header, 16)
                if (elf_version != 1 or flags & 0xff000000 != 0x05000000 or flags & 0x600 != 0x400
                        or ehsize != 52 or phsize != 32 or not phcount or phoff < 52
                        or phoff + phsize * phcount > binary.stat().st_size):
                    raise ValueError('Executable does not have the ARM EABI5 hard-float ABI: ' + name)
            stream.seek(0)
            digest = hashlib.file_digest(stream, 'sha256').digest()
        command = ([str(runner)] if runner is not None else []) + [str(binary.resolve()), '--version']
        if subprocess.check_output(command, timeout=5, text=True).strip() != name + ' ' + version:
            raise ValueError('Executable version does not match Cargo metadata: ' + name)
        entries.append((binary, binary.stat().st_size, digest))
    return entries


def package(directory, target, output, tag, runner=None):
    metadata = json.loads(subprocess.check_output(
        ['cargo', 'metadata', '--no-deps', '--locked', '--format-version', '1'], cwd=ROOT))
    version = next(p['version'] for p in metadata['packages'] if p['name'] == 'vitrallis-shell')
    if tag != 'v' + version:
        raise ValueError('Release tag must equal v plus the Cargo workspace version')
    if output.exists() or output.is_symlink():
        raise ValueError('Output directory already exists; refusing to overwrite release artifacts')
    entries = inventory(directory, target, version, runner)
    name = 'vitrallis-' + target + '-glibc2.36.vtrbundle'
    output = output.absolute()
    output.parent.mkdir(parents=True, exist_ok=True)
    with tempfile.TemporaryDirectory(prefix='.vitrallis-release-', dir=output.parent) as temporary:
        stage = Path(temporary) / 'artifacts'
        stage.mkdir()
        artifact = stage / name
        with artifact.open('xb') as bundle:
            bundle.write(MAGIC)
            for binary, size, expected in entries:
                bundle.write(struct.pack('<Q', size) + expected)
                digest = hashlib.sha256()
                remaining = size
                with binary.open('rb') as stream:
                    while remaining:
                        chunk = stream.read(min(65536, remaining))
                        if not chunk:
                            raise ValueError('Executable changed during packaging: ' + binary.name)
                        digest.update(chunk)
                        bundle.write(chunk)
                        remaining -= len(chunk)
                    if stream.read(1) or digest.digest() != expected:
                        raise ValueError('Executable changed during packaging: ' + binary.name)
        with artifact.open('rb') as stream:
            digest = hashlib.file_digest(stream, 'sha256').hexdigest()
        (stage / (name + '.sha256')).write_text(digest + '  ' + name + '\n')
        if target == 'armv7-unknown-linux-gnueabihf':
            for helper in SESSION_HELPERS:
                source = ROOT / 'integrations/pocketchip' / helper
                if source.is_symlink() or not source.is_file() or not 0 < source.stat().st_size <= 256 * 1024:
                    raise ValueError('Missing or unsafe ARMv7 Linux helper: ' + helper)
                data = source.read_bytes()
                compile(data, str(source), 'exec')
                (stage / helper).write_bytes(data)
                (stage / (helper + '.sha256')).write_text(hashlib.sha256(data).hexdigest() + '  ' + helper + '\n')
        stage.rename(output)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--bin-dir', required=True, type=Path)
    parser.add_argument('--target', required=True, choices=TARGETS)
    parser.add_argument('--output', required=True, type=Path)
    parser.add_argument('--tag', required=True)
    parser.add_argument('--runner', type=Path, help='Local emulator for cross-built executables')
    args = parser.parse_args()
    try:
        package(args.bin_dir, args.target, args.output, args.tag, args.runner)
    except (OSError, ValueError, subprocess.SubprocessError) as error:
        parser.exit(1, f'Vitrallis release packaging failed: {error}\n')


if __name__ == '__main__':
    main()
