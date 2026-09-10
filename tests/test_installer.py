import importlib.util
import json
import fcntl
import os
import shutil
import struct
import subprocess
import sys
from pathlib import Path
import tempfile
import unittest
from unittest.mock import patch

ROOT = Path(__file__).resolve().parents[1]
DEVICE = ROOT / 'devices/pocketchip'
spec = importlib.util.spec_from_file_location('installer', DEVICE / 'install.py')
m = importlib.util.module_from_spec(spec)
spec.loader.exec_module(m)


class Installer(unittest.TestCase):
    def setUp(self):
        self.temp = tempfile.TemporaryDirectory()
        self.addCleanup(self.temp.cleanup)
        self.home = Path(self.temp.name).resolve() / 'user home'
        self.home.mkdir()
        self.config = self.home / '.pocket-home/config.json'
        self.config.parent.mkdir()
        self.original = {'defaultPage': 'Apps', 'pages': [{'name': 'Apps', 'items': [{'name': 'Keep', 'shell': 'keep'}]}], 'custom': 19}
        self.config.write_text(json.dumps(self.original))
        self.source = self.home / 'source with spaces'
        self.source.mkdir()
        (self.source / 'vitrallis-session.py').write_text('# reviewed script')
        self.binary = self.source / 'binary'
        header = bytearray(84)
        header[:7] = b'\x7fELF\x01\x01\x01'
        struct.pack_into('<HHIIIIIHHH', header, 16, 2, 40, 1, 0, 52, 0, 0x05000400, 52, 32, 1)
        self.binary.write_bytes(header)
        self.target = self.home / '.local/share/vitrallis'

    def install(self):
        m.install(self.binary, self.source, self.home)

    def test_preserves_menu_and_idempotent_install(self):
        self.config.chmod(0o600)
        self.install()
        self.install()
        config = json.loads(self.config.read_bytes())
        self.assertEqual(config['custom'], 19)
        self.assertEqual(config['pages'][0]['items'][0], self.original['pages'][0]['items'][0])
        self.assertEqual(len(config['pages'][0]['items']), 2)
        self.assertFalse((self.target / '.installation-pending').exists())
        self.assertTrue((self.target / 'vitrallis').stat().st_mode & 0o111)
        self.assertEqual(self.config.stat().st_mode & 0o777, 0o600)

    def test_edited_desktop_shortcut_is_preserved(self):
        self.install()
        desktop = self.home / '.local/share/applications/vitrallis.desktop'
        desktop.write_text('user shortcut')
        before = self.config.read_bytes()
        with self.assertRaisesRegex(ValueError, 'edited desktop'):
            self.install()
        self.assertEqual(desktop.read_text(), 'user shortcut')
        self.assertEqual(self.config.read_bytes(), before)
        self.assertFalse((self.target / '.installation-pending').exists())

    def test_symlinked_backup_root_is_rejected_before_mutation(self):
        outside = self.home / 'outside'
        outside.mkdir()
        backups = self.home / '.local/share/vitrallis-backups'
        backups.parent.mkdir(parents=True)
        backups.symlink_to(outside, target_is_directory=True)
        before = self.config.read_bytes()
        with self.assertRaisesRegex(ValueError, 'symlink'):
            self.install()
        self.assertEqual(list(outside.iterdir()), [])
        self.assertEqual(self.config.read_bytes(), before)
        self.assertFalse(self.target.exists())

    def test_user_edit_blocks_replacement(self):
        self.install()
        (self.target / 'vitrallis-session.py').write_text('# user edit')
        with self.assertRaisesRegex(ValueError, 'edited'):
            self.install()
        self.assertEqual((self.target / 'vitrallis-session.py').read_text(), '# user edit')

    def test_menu_write_failure_rolls_back(self):
        original = self.config.read_bytes()
        real = m.atomic
        failed = []
        def fail(path, *args):
            if path == self.config and not failed:
                failed.append(True)
                raise OSError('full filesystem')
            real(path, *args)
        with patch.object(m, 'atomic', side_effect=fail):
            with self.assertRaises(OSError):
                self.install()
        self.assertEqual(self.config.read_bytes(), original)
        self.assertFalse((self.target / 'vitrallis').exists())
        self.install()
        self.assertFalse((self.target / '.installation-pending').exists())

    def test_rejects_bad_binary_and_symlink_before_menu_changes(self):
        self.binary.write_bytes(b'wrong architecture')
        with self.assertRaises(ValueError):
            self.install()
        self.assertEqual(json.loads(self.config.read_bytes()), self.original)
        self.config.unlink()
        self.config.symlink_to(self.binary)
        with self.assertRaises(ValueError):
            self.install()
        self.assertEqual(self.binary.read_bytes(), b'wrong architecture')

    def test_failure_after_rename_is_rolled_back(self):
        real = m.atomic
        failed = []
        def write(path, *args):
            real(path, *args)
            if path == self.target / 'vitrallis' and not failed:
                failed.append(True)
                raise OSError('directory fsync failure')
        with patch.object(m, 'atomic', side_effect=write):
            with self.assertRaises(OSError):
                self.install()
        self.assertFalse((self.target / 'vitrallis').exists())
        self.assertTrue((self.target / '.installation-pending').exists())
        self.install()

    def test_concurrent_installer_is_rejected(self):
        with (self.config.parent / 'vitrallis-install.lock').open('a+') as lock:
            fcntl.flock(lock, fcntl.LOCK_EX | fcntl.LOCK_NB)
            with self.assertRaises(BlockingIOError):
                self.install()
        self.assertFalse(self.target.exists())

    def test_malformed_config_and_marker_fail_before_installation(self):
        for value in ([], None, 'text', 42):
            self.config.write_text(json.dumps(value))
            with self.assertRaises(ValueError):
                self.install()
            self.assertFalse(self.target.exists())
        self.config.write_text(json.dumps(self.original))
        self.install()
        (self.target / '.installation-pending').write_text('{"vitrallis": 42}')
        with self.assertRaisesRegex(ValueError, 'recovery marker'):
            self.install()

    def test_hardlinked_file_is_rejected(self):
        outside = self.home / 'outside'
        os.link(self.config, outside)
        original = outside.read_bytes()
        with self.assertRaisesRegex(ValueError, 'hardlink'):
            self.install()
        self.assertEqual(outside.read_bytes(), original)
        self.assertFalse(self.target.exists())

    def test_concurrent_menu_edit_is_preserved_during_rollback(self):
        real = m.atomic
        def edit(path, *args):
            if path == self.target / 'installed.json':
                self.config.write_text('concurrent user edit')
                raise OSError('injected receipt failure')
            real(path, *args)
        with patch.object(m, 'atomic', side_effect=edit):
            with self.assertRaises(OSError):
                self.install()
        self.assertEqual(self.config.read_text(), 'concurrent user edit')
        self.assertTrue((self.target / '.installation-pending').exists())

    def run_installer(self, script, *args):
        # Exercise the actual CLI, but only against this fixture HOME. Mock the
        # normal-user check so the same staging test also works in root-run CI.
        runner = (
            'import runpy, sys\n'
            'from unittest.mock import patch\n'
            'sys.argv = sys.argv[1:]\n'
            "with patch('os.geteuid', return_value=1000):\n"
            "    runpy.run_path(sys.argv[0], run_name='__main__')\n"
        )
        return subprocess.run(
            [sys.executable, '-c', runner, str(script), *args],
            cwd=self.home, env=dict(os.environ, HOME=str(self.home)),
            capture_output=True, text=True, check=False,
        )

    def stage_and_install(self, layout):
        stage = self.home / 'package with spaces'
        stage.mkdir()
        canonical = stage / 'devices/pocketchip' if layout == 'checkout' else stage
        canonical.mkdir(parents=True, exist_ok=True)
        for name in ('install.py', 'vitrallis-session.py'):
            shutil.copyfile(DEVICE / name, canonical / name)
        if layout == 'canonical':
            entry = canonical / 'install.py'
        else:
            entry = stage / 'scripts/install-pocketchip.py' if layout == 'checkout' else stage / 'install-pocketchip.py'
            entry.parent.mkdir(exist_ok=True)
            shutil.copyfile(ROOT / 'scripts/install-pocketchip.py', entry)
        result = self.run_installer(entry, str(self.binary.relative_to(self.home)))
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertEqual((self.target / 'vitrallis').read_bytes(), self.binary.read_bytes())
        self.assertEqual((self.target / 'vitrallis-session.py').read_bytes(), (DEVICE / 'vitrallis-session.py').read_bytes())
        self.assertIn(str(self.target), result.stdout)
        self.assertFalse((self.target / '.installation-pending').exists())

    def test_checkout_shim_stages_from_unrelated_cwd_with_spaces(self):
        self.stage_and_install('checkout')

    def test_canonical_pair_stages_from_unrelated_cwd_with_spaces(self):
        self.stage_and_install('canonical')

    def test_standalone_legacy_bundle_stages_without_checkout(self):
        self.stage_and_install('legacy')

    def test_missing_canonical_fails_without_mutation(self):
        entry = self.source / 'install-pocketchip.py'
        shutil.copyfile(ROOT / 'scripts/install-pocketchip.py', entry)
        before = self.config.read_bytes()
        result = self.run_installer(entry, str(self.binary))
        self.assertEqual(result.returncode, 1)
        self.assertIn('place install.py and vitrallis-session.py beside', result.stderr)
        self.assertEqual(self.config.read_bytes(), before)
        self.assertFalse((self.config.parent / 'vitrallis-install.lock').exists())
        self.assertFalse(self.target.exists())

    def test_shim_preserves_arguments_and_exit_status(self):
        entry = self.source / 'install-pocketchip.py'
        shutil.copyfile(ROOT / 'scripts/install-pocketchip.py', entry)
        (self.source / 'install.py').write_text(
            'import json, sys\n'
            'print(json.dumps(sys.argv[1:]))\n'
            'raise SystemExit(23)\n'
        )
        args = ['binary with spaces', '--unknown-option', 'literal $HOME']
        result = self.run_installer(entry, *args)
        self.assertEqual(result.returncode, 23, result.stderr)
        self.assertEqual(json.loads(result.stdout), args)
        self.assertFalse(self.target.exists())

    def test_missing_adjacent_session_fails_before_installing_files(self):
        entry = self.source / 'install.py'
        shutil.copyfile(DEVICE / 'install.py', entry)
        (self.source / 'vitrallis-session.py').unlink()
        before = self.config.read_bytes()
        result = self.run_installer(entry, str(self.binary))
        self.assertEqual(result.returncode, 1)
        self.assertIn('vitrallis-session.py', result.stderr)
        self.assertEqual(self.config.read_bytes(), before)
        self.assertFalse(self.target.exists())


class BoundedInputs(unittest.TestCase):
    def test_read_limit_is_enforced_without_truncating_input(self):
        with tempfile.TemporaryDirectory() as temp:
            path = Path(temp).resolve() / 'source'
            path.write_bytes(b'abcd')
            with self.assertRaisesRegex(ValueError, 'size limit'):
                m.read_file(path, limit=3)
            self.assertEqual(m.read_file(path, limit=4), b'abcd')
            self.assertEqual(path.read_bytes(), b'abcd')


if __name__ == '__main__':
    unittest.main()
