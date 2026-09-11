"""Shell release packaging contract, without network or executable fixtures."""
import hashlib
import importlib.util
import json
from pathlib import Path
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
        self.binary = self.root / 'vitrallis'
        self.data = bytearray(64)
        self.data[:7] = b'\x7fELF\x02\x01\x01'
        self.data[16:20] = b'\x03\x00\x3e\x00'
        self.binary.write_bytes(self.data)
        self.output = self.root / 'release'
        self.check = mock.patch.object(RELEASE.subprocess, 'check_output', side_effect=[
            json.dumps({'packages': [{'name': 'vitrallis-shell', 'version': '1.2.3'}]}).encode(),
            'vitrallis 1.2.3\n',
        ]).start()
        self.addCleanup(mock.patch.stopall)

    def package(self, target='x86_64-unknown-linux-gnu', tag='v1.2.3'):
        RELEASE.package(self.binary, target, self.output, tag)

    def test_packages_only_native_shell_and_exact_named_checksum(self):
        apps = self.root / 'apps'
        apps.mkdir()
        (apps / 'sentinel').write_text('untouched')
        self.package()
        name = 'vitrallis-x86_64-unknown-linux-gnu-glibc2.36'
        self.assertEqual(sorted(p.name for p in self.output.iterdir()), [name, name + '.sha256'])
        self.assertEqual((self.output / name).read_bytes(), self.data)
        self.assertEqual((self.output / (name + '.sha256')).read_text(),
                         hashlib.sha256(self.data).hexdigest() + '  ' + name + '\n')
        self.assertEqual((apps / 'sentinel').read_text(), 'untouched')
        self.assertEqual(self.check.call_args_list[1].args[0], [str(self.binary.resolve()), '--version'])

    def test_rejects_tag_version_mismatch_before_creating_output(self):
        with self.assertRaisesRegex(ValueError, 'tag'):
            self.package(tag='v9.0.0')
        self.assertFalse(self.output.exists())
        self.assertEqual(self.check.call_count, 1)

    def test_rejects_foreign_architecture_before_executing_binary(self):
        with self.assertRaisesRegex(ValueError, 'target'):
            self.package(target='aarch64-unknown-linux-gnu')
        self.assertEqual(self.check.call_count, 1)
        self.assertFalse(self.output.exists())

    def test_packages_arm_beta_using_an_explicit_emulator_without_shell_parsing(self):
        self.data[:7] = b'\x7fELF\x01\x01\x01'
        self.data[18:20] = b'\x28\x00'
        self.binary.write_bytes(self.data)
        self.check.side_effect = [json.dumps({'packages': [
            {'name': 'vitrallis-shell', 'version': '0.1.0-beta.2'}]}).encode(),
            'vitrallis 0.1.0-beta.2\n']
        runner = Path('/local emulator/qemu-arm')
        RELEASE.package(self.binary, 'armv7-unknown-linux-gnueabihf', self.output,
                        'v0.1.0-beta.2', runner)
        name = 'vitrallis-armv7-unknown-linux-gnueabihf-glibc2.36'
        self.assertEqual((self.output / name).read_bytes(), self.data)
        self.assertEqual((self.output / (name + '.sha256')).read_text(),
                         hashlib.sha256(self.data).hexdigest() + '  ' + name + '\n')
        self.assertEqual(self.check.call_args_list[1].args[0],
                         [str(runner), str(self.binary.resolve()), '--version'])

    def test_rejects_wrong_executable_version_and_existing_output(self):
        self.check.side_effect = [json.dumps({'packages': [{'name': 'vitrallis-shell', 'version': '1.2.3'}]}).encode(), 'vitrallis 0.9.0\n']
        with self.assertRaisesRegex(ValueError, 'version'):
            self.package()
        self.assertFalse(self.output.exists())
        self.output.mkdir()
        sentinel = self.output / 'keep'
        sentinel.write_text('previous release')
        self.check.side_effect = [json.dumps({'packages': [{'name': 'vitrallis-shell', 'version': '1.2.3'}]}).encode(), 'vitrallis 1.2.3\n']
        with self.assertRaisesRegex(ValueError, 'already exists'):
            self.package()
        self.assertEqual(sentinel.read_text(), 'previous release')


if __name__ == '__main__':
    unittest.main()
