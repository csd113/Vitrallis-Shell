import importlib.util
import json
from pathlib import Path
import tempfile
import unittest
from unittest.mock import patch

spec = importlib.util.spec_from_file_location('installer', Path(__file__).parents[1] / 'scripts/install-pocketchip.py')
m = importlib.util.module_from_spec(spec)
spec.loader.exec_module(m)


class Installer(unittest.TestCase):
    def setUp(self):
        self.temp = tempfile.TemporaryDirectory()
        self.addCleanup(self.temp.cleanup)
        self.home = Path(self.temp.name).resolve()
        self.config = self.home / '.pocket-home/config.json'
        self.config.parent.mkdir()
        self.original = {'defaultPage': 'Apps', 'pages': [{'name': 'Apps', 'items': [{'name': 'Keep', 'shell': 'keep'}]}], 'custom': 19}
        self.config.write_text(json.dumps(self.original))
        self.source = self.home / 'source'
        self.source.mkdir()
        (self.source / 'vitrallis-session.py').write_text('# reviewed script')
        self.binary = self.source / 'binary'
        self.binary.write_bytes(b'\x7fELF\x01\x01\x01' + b'\0' * 11 + b'\x28\x00')
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


if __name__ == '__main__':
    unittest.main()
