#!/usr/bin/env python3
"""Package a native shell executable and SHA-256 sidecar; never package apps."""
import argparse
import hashlib
import json
from pathlib import Path
import subprocess
import tempfile


TARGETS = {
    'x86_64-unknown-linux-gnu': (2, 62),
    'aarch64-unknown-linux-gnu': (2, 183),
    'armv7-unknown-linux-gnueabihf': (1, 40),
}
ROOT = Path(__file__).resolve().parents[1]


def package(binary, target, output, tag, runner=None):
    metadata = json.loads(subprocess.check_output(
        ['cargo', 'metadata', '--no-deps', '--locked', '--format-version', '1'], cwd=ROOT))
    version = next(p['version'] for p in metadata['packages'] if p['name'] == 'vitrallis-shell')
    if tag != 'v' + version:
        raise ValueError('Release tag must equal v plus the Cargo package version')
    if binary.is_symlink() or not binary.is_file() or not 64 <= binary.stat().st_size <= 64 * 1024 * 1024:
        raise ValueError('Expected a regular shell executable between 64 bytes and 64 MiB')
    data = binary.read_bytes()
    elf_class, machine = TARGETS[target]
    if (data[:4] != b'\x7fELF' or data[4:7] != bytes([elf_class, 1, 1])
            or int.from_bytes(data[16:18], 'little') not in (2, 3)
            or int.from_bytes(data[18:20], 'little') != machine):
        raise ValueError('Executable does not match the release target')
    # Run only a maintainer-supplied local build, never remote release commands.
    command = ([str(runner)] if runner is not None else []) + [str(binary.resolve()), '--version']
    actual = subprocess.check_output(command, timeout=5, text=True).strip()
    if actual != 'vitrallis ' + version:
        raise ValueError('Executable version does not match Cargo metadata')
    name = 'vitrallis-' + target + '-glibc2.36'
    output = output.absolute()
    output.parent.mkdir(parents=True, exist_ok=True)
    if output.exists() or output.is_symlink():
        raise ValueError('Output directory already exists; refusing to overwrite release artifacts')
    with tempfile.TemporaryDirectory(prefix='.shell-release-', dir=output.parent) as temporary:
        stage = Path(temporary) / 'artifacts'
        stage.mkdir()
        artifact = stage / name
        artifact.write_bytes(data)
        artifact.chmod(0o755)
        (stage / (name + '.sha256')).write_text(hashlib.sha256(data).hexdigest() + '  ' + name + '\n')
        stage.rename(output)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--binary', required=True, type=Path)
    parser.add_argument('--target', required=True, choices=TARGETS)
    parser.add_argument('--output', required=True, type=Path)
    parser.add_argument('--tag', required=True)
    parser.add_argument('--runner', type=Path, help='Local emulator executable for a cross-built binary')
    args = parser.parse_args()
    try:
        package(args.binary, args.target, args.output, args.tag, args.runner)
    except (OSError, ValueError, subprocess.SubprocessError) as error:
        parser.exit(1, f'Shell release packaging failed: {error}\n')


if __name__ == '__main__':
    main()
