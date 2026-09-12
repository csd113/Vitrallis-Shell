import importlib.util
import hashlib
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
        self.payload = bytes(header)
        self.binary.write_bytes(m.MAGIC + b''.join(struct.pack('<Q', len(header)) + hashlib.sha256(header).digest() + header for _ in m.BINARIES))
        self.target = self.home / '.local/share/vitrallis'

    def install(self):
        # Tiny ARM header fixtures cannot execute on the host; real bounded
        # version probes are tested separately with owned executable fixtures.
        with patch.object(m, 'verify_versions'):
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
        self.assertTrue((self.target / 'current/vitrallis').stat().st_mode & 0o111)
        self.assertEqual(self.config.stat().st_mode & 0o777, 0o600)

    def test_group_writable_umask_does_not_make_new_ota_directories_unsafe(self):
        previous = os.umask(0o002)
        try:
            self.install()
        finally:
            os.umask(previous)
        for path in (self.home / '.local', self.home / '.local/share', self.target):
            self.assertEqual(path.stat().st_mode & 0o777, 0o755)
        self.assertEqual((self.target / 'current/vitrallis').stat().st_mode & 0o777, 0o755)

    def test_existing_directory_permissions_are_preserved(self):
        self.target.mkdir(parents=True, mode=0o700)
        self.install()
        self.assertEqual(self.target.stat().st_mode & 0o777, 0o700)

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
        self.assertFalse((self.target / 'current/vitrallis').exists())
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
            if path == self.target / 'vitrallis-session.py' and not failed:
                failed.append(True)
                raise OSError('directory fsync failure')
        with patch.object(m, 'atomic', side_effect=write):
            with self.assertRaises(OSError):
                self.install()
        self.assertFalse((self.target / 'current/vitrallis').exists())
        self.assertTrue((self.target / '.installation-pending').exists())
        self.install()

    def test_concurrent_installer_is_rejected(self):
        stage = self.target / '.vitrallis-update'
        stage.mkdir(parents=True, mode=0o700)
        with (stage / 'lock').open('a+') as lock:
            fcntl.flock(lock, fcntl.LOCK_EX | fcntl.LOCK_NB)
            with self.assertRaises(BlockingIOError):
                self.install()
        self.assertFalse((self.target / 'current').exists())

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

    def test_incomplete_corrupt_or_foreign_companion_never_changes_current(self):
        self.install()
        current = os.readlink(self.target / 'current')
        original = self.binary.read_bytes()
        for bad in (original[:-1], original + b'extra', original[:-1] + bytes([original[-1] ^ 1])):
            self.binary.write_bytes(bad)
            with self.assertRaises(ValueError):
                self.install()
            self.assertEqual(os.readlink(self.target / 'current'), current)
            for name in m.BINARIES:
                self.assertEqual((self.target / 'current' / name).read_bytes(), self.payload)
        self.binary.write_bytes(original)

    def test_failed_pointer_sync_rolls_back_the_whole_generation(self):
        self.install()
        old = os.readlink(self.target / 'current')
        changed = self.payload + b'new generation'
        self.binary.write_bytes(m.MAGIC + b''.join(struct.pack('<Q', len(changed)) + hashlib.sha256(changed).digest() + changed for _ in m.BINARIES))
        real = m.atomic_pointer
        failed = []
        def publish(path, value):
            real(path, value)
            if path == self.target / 'current' and not failed:
                failed.append(True)
                raise OSError('injected pointer durability error')
        with patch.object(m, 'atomic_pointer', side_effect=publish):
            with self.assertRaises(OSError):
                self.install()
        self.assertEqual(os.readlink(self.target / 'current'), old)
        for name in m.BINARIES:
            self.assertEqual((self.target / 'current' / name).read_bytes(), self.payload)
        self.assertTrue((self.target / '.installation-pending').exists())
        self.install()
        self.assertNotEqual(os.readlink(self.target / 'current'), old)

    def test_bounded_version_probes_reject_mixed_versions_and_output_floods(self):
        generation = self.source / 'probes'
        generation.mkdir()
        for name in m.BINARIES:
            path = generation / name
            path.write_text('#!/bin/sh\nprintf "' + name + ' 0.1.0-test\\n"\n')
            path.chmod(0o755)
        m.verify_versions(generation)
        (generation / 'vitrallis-files').write_text('#!/bin/sh\nprintf "vitrallis-files 0.2.0\\n"\n')
        with self.assertRaisesRegex(ValueError, 'version mismatch'):
            m.verify_versions(generation)
        (generation / 'vitrallis-files').write_text('#!/bin/sh\nwhile :; do printf "output flood"; done\n')
        with self.assertRaises(subprocess.SubprocessError):
            m.verify_versions(generation)

    def run_installer(self, script, *args):
        # Exercise the actual CLI, but only against this fixture HOME. Mock the
        # normal-user check so the same staging test also works in root-run CI.
        runner = (
            'import runpy, sys\n'
            'from unittest.mock import patch\n'
            'sys.argv = sys.argv[1:]\n'
            'def probe(args, **kwargs):\n'
            '    from pathlib import Path\n'
            '    kwargs["stdout"].write((Path(args[0]).name + " 0.1.0-test\\n").encode())\n'

            "with patch('os.geteuid', return_value=1000), patch('subprocess.run', side_effect=probe):\n"
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
        entry = canonical / 'install.py'
        result = self.run_installer(entry, str(self.binary.relative_to(self.home)))
        self.assertEqual(result.returncode, 0, result.stderr)
        for name in m.BINARIES:
            self.assertEqual((self.target / 'current' / name).read_bytes(), self.payload)
        self.assertEqual((self.target / 'vitrallis-session.py').read_bytes(), (DEVICE / 'vitrallis-session.py').read_bytes())
        self.assertIn(str(self.target), result.stdout)
        self.assertFalse((self.target / '.installation-pending').exists())

    def test_checkout_installer_stages_from_unrelated_cwd_with_spaces(self):
        self.stage_and_install('checkout')

    def test_canonical_pair_stages_from_unrelated_cwd_with_spaces(self):
        self.stage_and_install('canonical')

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
