"""Host-only regression checks for build paths and the shipped source archive."""
import json
import os
from pathlib import Path
import shutil
import subprocess
import tarfile
import tempfile
import unittest


ROOT = Path(__file__).resolve().parents[1]


class BuildPaths(unittest.TestCase):
    def setUp(self):
        self.temp = tempfile.TemporaryDirectory(prefix='vitrallis build paths ')
        self.addCleanup(self.temp.cleanup)
        self.root = Path(self.temp.name).resolve()
        self.checkout = self.root / 'source checkout'
        (self.checkout / 'scripts').mkdir(parents=True)
        self.script = self.checkout / 'scripts/build-pocketchip.sh'
        shutil.copyfile(ROOT / 'scripts/build-pocketchip.sh', self.script)
        self.caller = self.root / 'unrelated working directory'
        self.caller.mkdir()
        self.pkgconfig = self.root / 'private ARM libraries/pkgconfig'
        self.pkgconfig.mkdir(parents=True)
        (self.pkgconfig / 'sdl2.pc').write_text('# test fixture only\n')
        commands = self.root / 'mock commands'
        commands.mkdir()
        cargo = commands / 'cargo'
        cargo.write_text(
            '#!/bin/sh\n'
            'printf "%s\\n" "$PWD" "$PKG_CONFIG_LIBDIR" '
            '"$PKG_CONFIG_ALLOW_CROSS" "${PKG_CONFIG_PATH-unset}" "$@"\n'
        )
        cargo.chmod(0o755)
        self.env = dict(os.environ, PATH=str(commands) + os.pathsep + os.environ['PATH'],
                        PKG_CONFIG_LIBDIR=str(self.pkgconfig),
                        PKG_CONFIG_PATH='/unwanted/host/libraries')
        self.env.pop('VITRALLIS_ARM_GLIBC', None)

    def run_build(self):
        return subprocess.run(['sh', str(self.script)], cwd=self.caller, env=self.env,
                              capture_output=True, text=True, check=False)

    def test_cross_build_uses_script_root_and_explicit_target_libraries(self):
        result = self.run_build()
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertEqual(result.stdout.splitlines(), [
            str(self.checkout), str(self.pkgconfig), '1', 'unset',
            'zigbuild', '--locked', '--workspace', '--all-features', '--release',
            '--target', 'armv7-unknown-linux-gnueabihf.2.36',
        ])

    def test_missing_target_library_fails_before_cargo(self):
        (self.pkgconfig / 'sdl2.pc').unlink()
        result = self.run_build()
        self.assertEqual(result.returncode, 1)
        self.assertIn('Missing target sdl2.pc', result.stderr)
        self.assertEqual(result.stdout, '')

    def test_invalid_glibc_fails_before_cargo(self):
        self.env['VITRALLIS_ARM_GLIBC'] = 'host'
        result = self.run_build()
        self.assertEqual(result.returncode, 1)
        self.assertIn('VITRALLIS_ARM_GLIBC must be', result.stderr)
        self.assertEqual(result.stdout, '')


class SourcePackage(unittest.TestCase):
    def test_archive_contains_installation_pair_patch_bundle_and_embedded_assets(self):
        with tempfile.TemporaryDirectory(prefix='vitrallis source package ') as temp:
            output = Path(temp).resolve() / 'package output'
            result = subprocess.run([
                'cargo', 'package', '--allow-dirty', '--no-verify', '--locked', '--offline',
                '--manifest-path', str(ROOT / 'Cargo.toml'), '--target-dir', str(output),
            ], cwd=ROOT, capture_output=True, text=True, check=False)
            self.assertEqual(result.returncode, 0, result.stderr)
            archives = list((output / 'package').glob('*.crate'))
            self.assertEqual(len(archives), 1)
            required = {
                'Cargo.toml', 'Cargo.lock', 'rust-toolchain.toml', 'README.md',
                'scripts/install-pocketchip.py', 'scripts/build-pocketchip.sh',
                'scripts/validate.sh', 'devices/pocketchip/install.py',
                'devices/pocketchip/vitrallis-session.py',
                'devices/pocketchip/run-pocketchip.sh',
                'devices/pocketchip/apply-store-patch.py',
                'devices/pocketchip/integration/pocketchip-store.patch',
                'devices/pocketchip/integration/store-patch-manifest.json',
                'docs/devices/pocketchip.md', 'docs/repository-layout.md',
                'src/layout.rs', 'src/renderer.rs', 'src/renderer/system.rs',
                'src/discovery/catalog.rs', 'src/discovery/executable.rs',
                'src/discovery/pockethome.rs', 'src/platform/pocketchip/store.rs',
            }
            required.update('assets/system/' + name + '.png'
                            for name in ('gear', 'wifi', 'sun', 'speaker', 'power', 'restart'))
            with tarfile.open(archives[0], 'r:gz') as archive:
                members = {member.name.split('/', 1)[1]: member
                           for member in archive.getmembers()}
                self.assertTrue(required <= members.keys(), required - members.keys())
                for name, member in members.items():
                    self.assertTrue(member.isfile(), name)
                    self.assertNotIn('target', Path(name).parts)
                    self.assertNotIn('__pycache__', Path(name).parts)
                    self.assertNotIn(name, {
                        'scripts/vitrallis-session.py', 'scripts/run-pocketchip.sh',
                        'scripts/apply-store-patch.py', 'integration/pocketchip-store.patch',
                        'src/discovery/marshmallow.rs', 'src/discovery/store.rs',
                    })
                for name in required - {'Cargo.toml'}:
                    with archive.extractfile(members[name]) as stream:
                        self.assertEqual(stream.read(), (ROOT / name).read_bytes(), name)
                manifest_name = 'devices/pocketchip/integration/store-patch-manifest.json'
                with archive.extractfile(members[manifest_name]) as stream:
                    self.assertEqual(len(json.load(stream)['files']), 5)


if __name__ == '__main__':
    unittest.main()
