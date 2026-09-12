"""Complete native release inventory, version, target, checksum and failure guards."""
import hashlib
import importlib.util
import json
from pathlib import Path
import struct
import tempfile
import unittest
from unittest import mock

ROOT = Path(__file__).resolve().parents[1]
SPEC = importlib.util.spec_from_file_location('shell_release', ROOT / 'scripts/package-shell-release.py')
RELEASE = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(RELEASE)


class ShellRelease(unittest.TestCase):
    def setUp(self):
        self.temp = tempfile.TemporaryDirectory()
        self.addCleanup(self.temp.cleanup)
        self.root = Path(self.temp.name)
        self.binaries = self.root / 'native binaries'
        self.binaries.mkdir()
        self.data = bytearray(64)
        self.data[:7] = b'\x7fELF\x02\x01\x01'
        self.data[16:20] = b'\x03\x00\x3e\x00'
        self.write_binaries()
        self.output = self.root / 'release'
        self.check = mock.patch.object(RELEASE.subprocess, 'check_output').start()
        self.addCleanup(mock.patch.stopall)
        self.version('1.2.3')

    def write_binaries(self):
        for name in RELEASE.BINARIES:
            (self.binaries / name).write_bytes(self.data)

    def version(self, version):
        self.check.side_effect = [json.dumps({'packages': [{'name': 'vitrallis-shell', 'version': version}]}).encode()] + [name + ' ' + version + '\n' for name in RELEASE.BINARIES]

    def package(self, target='x86_64-unknown-linux-gnu', tag='v1.2.3', runner=None):
        RELEASE.package(self.binaries, target, self.output, tag, runner)

    def contents(self, name):
        payload = (self.output / name).read_bytes()
        self.assertTrue(payload.startswith(RELEASE.MAGIC))
        offset = len(RELEASE.MAGIC)
        for binary in RELEASE.BINARIES:
            size = struct.unpack_from('<Q', payload, offset)[0]
            digest = payload[offset + 8:offset + 40]
            content = payload[offset + 40:offset + 40 + size]
            self.assertEqual(content, (self.binaries / binary).read_bytes())
            self.assertEqual(digest, hashlib.sha256(content).digest())
            offset += 40 + size
        self.assertEqual(offset, len(payload))
        self.assertEqual((self.output / (name + '.sha256')).read_text(), hashlib.sha256(payload).hexdigest() + '  ' + name + '\n')

    def test_packages_exactly_four_native_binaries_and_checksum(self):
        apps = self.root / 'apps'
        apps.mkdir()
        (apps / 'sentinel').write_text('untouched')
        self.package()
        name = 'vitrallis-x86_64-unknown-linux-gnu-glibc2.36.vtrbundle'
        self.assertEqual(sorted(p.name for p in self.output.iterdir()), [name, name + '.sha256'])
        self.contents(name)
        self.assertEqual((apps / 'sentinel').read_text(), 'untouched')
        self.assertEqual(self.check.call_count, 5)
        for i, binary in enumerate(RELEASE.BINARIES, 1):
            self.assertEqual(self.check.call_args_list[i].args[0], [str((self.binaries / binary).resolve()), '--version'])

    def test_missing_companion_prevents_artifact_publication(self):
        (self.binaries / 'vitrallis-files').unlink()
        with self.assertRaisesRegex(ValueError, 'vitrallis-files'):
            self.package()
        self.assertFalse(self.output.exists())

    def test_wrong_companion_version_prevents_mixed_builds(self):
        self.check.side_effect = [json.dumps({'packages': [{'name': 'vitrallis-shell', 'version': '1.2.3'}]}).encode(), 'vitrallis 1.2.3\n', 'vitrallis-terminal 1.2.2\n']
        with self.assertRaisesRegex(ValueError, 'version'):
            self.package()
        self.assertFalse(self.output.exists())

    def test_tag_mismatch_is_rejected_before_probing(self):
        with self.assertRaisesRegex(ValueError, 'tag'):
            self.package(tag='v9.0.0')
        self.assertEqual(self.check.call_count, 1)
        self.assertFalse(self.output.exists())

    def test_foreign_architecture_and_symlink_are_rejected(self):
        with self.assertRaisesRegex(ValueError, 'target'):
            self.package(target='aarch64-unknown-linux-gnu')
        self.assertEqual(self.check.call_count, 1)
        self.version('1.2.3')
        binary = self.binaries / 'vitrallis'
        binary.unlink()
        binary.symlink_to(self.binaries / 'vitrallis-files')
        with self.assertRaisesRegex(ValueError, 'invalid'):
            self.package()
        self.assertFalse(self.output.exists())

    def test_arm_beta_uses_explicit_emulator_for_every_executable(self):
        self.data[:7] = b'\x7fELF\x01\x01\x01'
        self.data[18:20] = b'\x28\x00'
        self.write_binaries()
        self.version('0.1.0-beta.2')
        runner = Path('/local emulator/qemu-arm')
        self.package('armv7-unknown-linux-gnueabihf', 'v0.1.0-beta.2', runner)
        self.contents('vitrallis-armv7-unknown-linux-gnueabihf-glibc2.36.vtrbundle')
        for i, binary in enumerate(RELEASE.BINARIES, 1):
            self.assertEqual(self.check.call_args_list[i].args[0], [str(runner), str((self.binaries / binary).resolve()), '--version'])

    def test_existing_output_is_never_overwritten(self):
        self.output.mkdir()
        sentinel = self.output / 'keep'
        sentinel.write_text('previous release')
        with self.assertRaisesRegex(ValueError, 'already exists'):
            self.package()
        self.assertEqual(sentinel.read_text(), 'previous release')


if __name__ == '__main__':
    unittest.main()
