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
DEVICE = ROOT / 'integrations/pocketchip'
spec = importlib.util.spec_from_file_location('installer', DEVICE / 'install-session.py')
m = importlib.util.module_from_spec(spec)
spec.loader.exec_module(m)


def setUpModule():
    # Fixtures must not inherit the caller's umask. The production guards
    # correctly reject group-writable directories, so a shared umask 002 would
    # make every fixture look unsafe. Dedicated tests set their own umask.
    global _module_umask
    _module_umask = os.umask(0o022)


def tearDownModule():
    os.umask(_module_umask)


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
        for name in m.HELPERS:
            shutil.copyfile(DEVICE / name, self.source / name)
        self.addCleanup(patch.stopall)
        patch.object(m, 'preflight').start()
        patch.object(m, 'require_stopped_session').start()
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

    def test_valid_bundle_runs_platform_setup(self):
        with patch.object(m, 'setup_platform') as setup:
            self.install()
        setup.assert_called_once_with(self.source)

    def test_invalid_bundle_never_runs_privileged_platform_setup(self):
        self.binary.write_bytes(b'invalid')
        with patch.object(m, 'setup_platform') as setup:
            with self.assertRaises(ValueError): self.install()
        setup.assert_not_called()

    def test_preserves_original_session_menu_bytes_and_idempotent_install(self):
        self.config.chmod(0o600)
        self.install()
        self.install()
        config = json.loads(self.config.read_bytes())
        self.assertEqual(config['custom'], 19)
        self.assertEqual(config['pages'][0]['items'][0], self.original['pages'][0]['items'][0])
        self.assertEqual(len(config['pages'][0]['items']), 1)
        self.assertEqual(self.config.read_text(), json.dumps(self.original))
        self.assertFalse((self.target / '.installation-pending').exists())
        self.assertTrue((self.target / 'current/vitrallis').stat().st_mode & 0o111)
        self.assertEqual(self.config.stat().st_mode & 0o777, 0o600)

    def test_tor_defaults_are_private_idempotent_and_preserve_startup_selection(self):
        self.install()
        root = self.target / 'tor'
        config = self.home / '.config/vitrallis/tor.json'
        self.assertEqual(json.loads(config.read_text()), {'startup': 'on-demand'})
        self.assertEqual(config.stat().st_mode & 0o777, 0o600)
        for path in (root, root / 'cache', root / 'state'):
            self.assertEqual(path.stat().st_mode & 0o777, 0o700)
        config.write_text('{"startup":"disabled"}')
        self.install()
        self.assertEqual(json.loads(config.read_text()), {'startup': 'disabled'})
        config.write_text('{"startup":"sometimes"}')
        with self.assertRaisesRegex(ValueError, 'Malformed Tor'):
            self.install()
        self.assertEqual(config.read_text(), '{"startup":"sometimes"}')

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

    def test_obsolete_helper_receipt_is_rejected_without_adopting_its_layout(self):
        self.install()
        path = self.target / 'installed.json'
        receipt = json.loads(path.read_bytes())
        receipt['install.py'] = receipt.pop('install-session.py')
        path.write_text(json.dumps(receipt))
        (self.target / 'install-session.py').rename(self.target / 'install.py')
        before = {p.name: p.read_bytes() for p in self.target.iterdir() if p.is_file()}
        current = os.readlink(self.target / 'current')
        with self.assertRaisesRegex(ValueError, 'Invalid receipt digest: install-session.py'):
            self.install()
        self.assertEqual(os.readlink(self.target / 'current'), current)
        self.assertEqual({p.name: p.read_bytes() for p in self.target.iterdir() if p.is_file()}, before)
        self.assertFalse((self.target / 'install-session.py').exists())

    def test_user_edit_blocks_replacement(self):
        self.install()
        (self.target / 'vitrallis-session.py').write_text('# user edit')
        with self.assertRaisesRegex(ValueError, 'edited'):
            self.install()
        self.assertEqual((self.target / 'vitrallis-session.py').read_text(), '# user edit')

    def test_shortcut_write_failure_rolls_back(self):
        original = self.config.read_bytes()
        real = m.atomic
        failed = []
        def fail(path, *args):
            if path == self.home / '.local/share/applications/vitrallis.desktop' and not failed:
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

    def test_no_user_launcher_config_is_required(self):
        self.config.unlink()
        self.config.parent.rmdir()
        self.install()
        self.assertFalse(self.config.exists())
        self.assertTrue((self.target / 'current/vitrallis-files').is_file())

    def test_unrelated_config_is_never_read_or_written(self):
        for content in ('broken', '[]', '{"pages":false}'):
            self.config.write_text(content)
            self.install()
            self.assertEqual(self.config.read_text(), content)
        outside = self.home / 'outside'
        self.config.rename(outside)
        self.config.symlink_to(outside)
        self.install()
        self.assertEqual(outside.read_text(), '{"pages":false}')
        self.config.unlink()
        os.link(outside, self.config)
        self.install()
        self.assertEqual(outside.read_text(), '{"pages":false}')

    def test_malformed_recovery_marker_fails(self):
        self.install()
        (self.target / '.installation-pending').write_text('{"vitrallis": 42}')
        with self.assertRaisesRegex(ValueError, 'recovery marker'):
            self.install()

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
            version = '2.6.0' if name == 'arti' else '0.1.0-test'
            path.write_text('#!/bin/sh\nprintf "' + ('Arti' if name == 'arti' else name) + ' ' + version + '\\n"\n')
            path.chmod(0o755)
        m.verify_versions(generation)
        (generation / 'vitrallis-files').write_text('#!/bin/sh\nprintf "vitrallis-files 0.2.0\\n"\n')
        with self.assertRaisesRegex(ValueError, 'version mismatch'):
            m.verify_versions(generation)
        (generation / 'vitrallis-files').write_text('#!/bin/sh\nwhile :; do printf "output flood"; done\n')
        with self.assertRaises(subprocess.SubprocessError):
            m.verify_versions(generation)

    def test_menu_edit_during_bundle_staging_is_not_overwritten(self):
        def edit(*args):
            config = json.loads(self.config.read_bytes())
            config['late edit'] = 'preserve'
            self.config.write_text(json.dumps(config))
        with patch.object(m, 'verify_versions', side_effect=edit):
            m.install(self.binary, self.source, self.home)
        self.assertEqual(json.loads(self.config.read_bytes())['late edit'], 'preserve')
        self.assertTrue((self.target / 'current').exists())

    def test_partial_desktop_commit_is_repairable_from_pending_receipt(self):
        self.install()
        receipt = json.loads((self.target / 'installed.json').read_bytes())
        desktop = self.home / '.local/share/applications/vitrallis.desktop'
        desktop.write_text('new staged shortcut')
        pending = dict(receipt, desktop_sha256=hashlib.sha256(desktop.read_bytes()).hexdigest())
        (self.target / '.installation-pending').write_text(json.dumps({'schema': 1, 'receipt': pending, 'hashes': {}}))
        self.install()
        self.assertTrue(desktop.read_text().startswith('[Desktop Entry]'))

    def run_installer(self, script, *args):
        # Exercise the actual CLI, but only against this fixture HOME. Mock the
        # normal-user check so the same staging test also works in root-run CI.
        runner = (
            'import importlib.util, sys\n'
            'from unittest.mock import patch\n'
            'from pathlib import Path\n'
            'script, bundle = sys.argv[1:3]\n'
            "spec = importlib.util.spec_from_file_location('device_install', script)\n"
            'module = importlib.util.module_from_spec(spec)\n'
            'spec.loader.exec_module(module)\n'
            "with patch.object(module, 'preflight'), patch.object(module, 'require_stopped_session'), patch.object(module, 'verify_versions'):\n"
            '    module.install(Path(bundle), Path(script).parent, Path.home())\n'
        )
        return subprocess.run(
            [sys.executable, '-c', runner, str(script), *args],
            cwd=self.home, env=dict(os.environ, HOME=str(self.home)),
            capture_output=True, text=True, check=False,
        )

    def stage_and_install(self, layout):
        stage = self.home / 'package with spaces'
        stage.mkdir()
        canonical = stage / 'integrations/pocketchip' if layout == 'checkout' else stage
        canonical.mkdir(parents=True, exist_ok=True)
        for name in m.HELPERS:
            shutil.copyfile(DEVICE / name, canonical / name)
        entry = canonical / 'install-session.py'
        result = self.run_installer(entry, str(self.binary.relative_to(self.home)))
        self.assertEqual(result.returncode, 0, result.stderr)
        for name in m.BINARIES:
            self.assertEqual((self.target / 'current' / name).read_bytes(), self.payload)
        self.assertEqual((self.target / 'vitrallis-session.py').read_bytes(), (DEVICE / 'vitrallis-session.py').read_bytes())
        self.assertIn(str(self.target), result.stdout)
        self.assertFalse((self.target / '.installation-pending').exists())

    def test_checkout_installer_stages_from_unrelated_cwd_with_spaces(self):
        self.stage_and_install('checkout')

    def test_canonical_helpers_stage_from_unrelated_cwd_with_spaces(self):
        self.stage_and_install('canonical')

    def test_missing_adjacent_session_fails_before_installing_files(self):
        entry = self.source / 'install-session.py'
        shutil.copyfile(DEVICE / 'install-session.py', entry)
        (self.source / 'vitrallis-session.py').unlink()
        before = self.config.read_bytes()
        result = self.run_installer(entry, str(self.binary))
        self.assertEqual(result.returncode, 1)
        self.assertIn('vitrallis-session.py', result.stderr)
        self.assertEqual(self.config.read_bytes(), before)
        self.assertFalse(self.target.exists())


class Preflight(unittest.TestCase):
    def setUp(self):
        from unittest.mock import Mock
        self.addCleanup(patch.stopall)
        patch.object(m.os, 'geteuid', return_value=1000).start()
        patch.object(m.platform, 'system', return_value='Linux').start()
        patch.object(m.platform, 'machine', return_value='armv7l').start()
        patch.object(m.Path, 'read_text', return_value='ID=debian\nVERSION_ID="13"\n').start()
        patch.object(m.os, 'access', return_value=True).start()
        self.answers = {'dpkg': 'armhf\n', 'getconf': 'glibc 2.36\n',
                        'awesome': 'awesome v4.3\n', 'systemctl': 'inactive\n'}
        patch.object(m.subprocess, 'check_output', side_effect=lambda args, **kwargs: self.answers[Path(args[0]).name]).start()
        self.sdl = Mock()
        def version(pointer):
            pointer._obj.major, pointer._obj.minor, pointer._obj.patch = (2, 26, 5)
        self.sdl.SDL_GetVersion.side_effect = version
        patch.object(m.ctypes, 'CDLL', return_value=self.sdl).start()

    def test_optional_graphics_never_blocks_software_installation(self):
        import io
        output = io.StringIO()
        with patch.object(m.ctypes, 'CDLL', side_effect=OSError('missing optional library')), \
                patch.object(m.Path, 'is_dir', return_value=False), \
                patch.object(m.sys, 'stderr', output):
            m.graphics_advice()
        self.assertIn('libegl-mesa0', output.getvalue())
        self.assertIn('libgles2', output.getvalue())
        self.assertIn('libdrm2', output.getvalue())
        self.assertIn('Software rendering remains available', output.getvalue())
        self.assertIn('/dev/dri unavailable', output.getvalue())

    def test_supported_runtime(self):
        m.preflight()

    def test_wrong_os_architecture_abi_or_runtime_is_rejected(self):
        cases = [('dpkg', 'armel'), ('getconf', 'glibc 2.35'), ('awesome', 'awesome v3.5'), ('systemctl', 'active')]
        for command, bad in cases:
            original = self.answers[command]
            self.answers[command] = bad
            with self.assertRaises(ValueError):
                m.preflight()
            self.answers[command] = original
        with patch.object(m.platform, 'machine', return_value='aarch64'), self.assertRaisesRegex(ValueError, '32-bit'):
            m.preflight()
        with patch.object(m.Path, 'read_text', return_value='ID=debian\nVERSION_ID=8'), self.assertRaisesRegex(ValueError, 'Jessie'):
            m.preflight()
        with patch.object(m.os, 'geteuid', return_value=0), self.assertRaisesRegex(ValueError, 'normal desktop user'):
            m.preflight()
        with patch.object(m.ctypes, 'CDLL', side_effect=OSError('missing SDL2')), self.assertRaises(OSError):
            m.preflight()
        with patch.object(m.os, 'access', return_value=False), self.assertRaisesRegex(ValueError, 'prerequisite'):
            m.preflight()


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
