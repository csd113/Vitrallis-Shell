"""Offline lifecycle tests confined to temporary user homes."""
import contextlib
import fcntl
import importlib.util
import io
import json
import os
from pathlib import Path
import sys
import subprocess
import unittest
from unittest.mock import patch

import test_installer as fixture
DEVICE = fixture.DEVICE

spec = importlib.util.spec_from_file_location('uninstaller', DEVICE / 'uninstall.py')
u = importlib.util.module_from_spec(spec)
spec.loader.exec_module(u)


class Uninstaller(unittest.TestCase):
    install = fixture.Installer.install
    # Reuse fixture construction, without inheriting the installer test cases.
    def setUp(self):
        fixture.Installer.setUp(self)
        self.output = io.StringIO()
        self.addCleanup(patch.stopall)
        patch.object(u, 'session', return_value=False).start()
        self.install()
        # Match the installed CLI's location so recovery copies fixture-owned
        # helpers, including in containers whose checkout belongs to the host.
        patch.object(u, '__file__', str(self.target / 'uninstall.py')).start()

    def remove(self, **kwargs):
        with contextlib.redirect_stdout(self.output):
            u.uninstall(self.home, **kwargs)

    def test_round_trip_and_repeat_preserve_data_and_later_menu_edits(self):
        config = json.loads(self.config.read_bytes())
        config['later'] = 'keep'
        config['pages'][0]['items'].append({'name': 'Later', 'shell': 'later'})
        self.config.write_text(json.dumps(config))
        save = self.target / 'apps/third-party/save.txt'
        save.parent.mkdir(parents=True)
        save.write_text('personal save')
        journal = self.target / 'app-center/transactions/keep'
        journal.parent.mkdir(parents=True)
        journal.write_text('app backup')
        log = self.target / 'session.log'
        log.write_text('log')
        backups = self.home / '.local/share/vitrallis-backups'
        prior_backups = sorted(backups.rglob('*'))
        self.remove()
        self.remove()
        result = json.loads(self.config.read_bytes())
        self.assertEqual(result['later'], 'keep')
        self.assertEqual([i['name'] for i in result['pages'][0]['items']], ['Keep', 'Later'])
        for name in u.HELPERS + ('launch', 'installed.json', 'generations', 'current', 'previous'):
            self.assertFalse(u.exists(self.target / name), name)
        self.assertEqual(save.read_text(), 'personal save')
        self.assertEqual(journal.read_text(), 'app backup')
        self.assertEqual(log.read_text(), 'log')
        self.assertEqual(sorted(backups.rglob('*')), prior_backups)
        self.install()
        self.assertTrue((self.target / 'current/vitrallis-files').is_file())

    def test_dry_run_has_no_filesystem_or_session_mutations(self):
        def snapshot():
            return {str(p): os.readlink(p) if p.is_symlink() else p.read_bytes()
                    for p in self.home.rglob('*') if p.is_file() or p.is_symlink()}
        before = snapshot()
        self.remove(dry_run=True)
        self.assertEqual(snapshot(), before)
        u.session.assert_called_once_with(self.home, True)

    def test_edited_helpers_shortcuts_and_matching_autostart(self):
        helper = self.target / 'launch'
        helper.write_text('# user launcher')
        desktop = self.home / u.DESKTOP
        autostart = self.home / u.AUTOSTART
        autostart.parent.mkdir(parents=True)
        autostart.write_bytes(desktop.read_bytes())
        desktop.write_text('edited shortcut')
        self.remove()
        self.assertEqual(helper.read_text(), '# user launcher')
        self.assertEqual(desktop.read_text(), 'edited shortcut')
        self.assertFalse(autostart.exists())

    def test_optional_startup_removes_only_exact_block(self):
        path = self.home / u.AWESOME
        path.parent.mkdir(parents=True)
        path.write_text('before\n' + u.STARTUP + '\nafter user edits\n')
        self.remove()
        self.assertEqual(path.read_text(), 'before\n\nafter user edits\n')

    def test_edited_startup_and_menu_entries_are_preserved(self):
        path = self.home / u.AWESOME
        path.parent.mkdir(parents=True)
        edited = u.STARTUP.replace('start_new(5', 'start_new(9')
        path.write_text(edited)
        config = json.loads(self.config.read_bytes())
        config['pages'][0]['items'][-1]['custom'] = 1
        self.config.write_text(json.dumps(config))
        self.remove()
        self.assertEqual(path.read_text(), edited)
        self.assertEqual(json.loads(self.config.read_bytes()), config)

    def test_arbitrary_pointers_and_file_symlinks_fail_before_stopping(self):
        pointer = self.target / 'current'
        pointer.unlink()
        outside = self.home / 'personal'
        outside.write_text('preserve')
        pointer.symlink_to(outside)
        with self.assertRaisesRegex(ValueError, 'pointer'):
            self.remove()
        self.assertEqual(outside.read_text(), 'preserve')
        u.session.assert_not_called()
        pointer.unlink()
        helper = self.target / 'launch'
        helper.unlink()
        helper.symlink_to(outside)
        with self.assertRaisesRegex(ValueError, 'symlink'):
            self.remove()
        self.assertEqual(outside.read_text(), 'preserve')

    def test_linked_generation_directory_fails_closed(self):
        generation = self.target / os.readlink(self.target / 'current')
        outside = self.home / 'outside'
        generation.rename(outside)
        generation.symlink_to(outside, target_is_directory=True)
        with self.assertRaisesRegex(ValueError, 'generation'):
            self.remove()
        self.assertTrue((outside / 'vitrallis').is_file())
        u.session.assert_not_called()

    def test_purge_only_fixed_preferences_and_logs(self):
        for relative in u.PURGE:
            path = self.home / relative
            path.parent.mkdir(parents=True, exist_ok=True)
            path.write_text('owned data')
        personal = self.home / '.config/vitrallis/my-notes'
        personal.write_text('keep')
        self.remove(purge=True)
        self.assertTrue(all(not (self.home / p).exists() for p in u.PURGE))
        self.assertEqual(personal.read_text(), 'keep')

    def test_purge_requires_literal_confirmation(self):
        with patch.object(sys, 'argv', ['uninstall.py', '--purge']), patch.object(os, 'geteuid', return_value=1000), \
                patch('builtins.input', return_value='yes'), patch.object(u, 'uninstall') as remove:
            u.main()
            remove.assert_not_called()
        with patch.object(sys, 'argv', ['uninstall.py', '--purge']), patch.object(os, 'geteuid', return_value=1000), \
                patch('builtins.input', return_value='PURGE'), patch.object(u, 'uninstall') as remove:
            u.main()
            self.assertTrue(remove.call_args.args[2])

    def test_write_failure_rolls_back_owned_files_and_startup_block(self):
        owned_config = self.home / u.AWESOME
        owned_config.parent.mkdir(parents=True)
        owned_config.write_text('before\n' + u.STARTUP + '\nafter\n')
        before = owned_config.read_bytes()
        real = u.atomic
        failed = []
        def fail(path, *args):
            if path == owned_config and not failed:
                failed.append(True)
                raise OSError('injected full disk')
            return real(path, *args)
        with patch.object(u, 'atomic', side_effect=fail):
            with self.assertRaisesRegex(OSError, 'full disk'):
                self.remove()
        self.assertEqual(owned_config.read_bytes(), before)
        self.assertTrue((self.target / 'current/vitrallis').is_file())
        self.assertTrue((self.target / 'uninstall.py').is_file())
        self.remove()

    def test_recovery_after_interruption_and_rejects_traversal_journal(self):
        actions = u.plan(self.home)
        tx = self.target / '.vitrallis-update/removal'
        tx.mkdir()
        journal = {'state': 'prepared', 'actions': actions}
        (tx / 'journal.json').write_text(json.dumps(journal))
        first = self.home / actions[0]['path']
        first.rename(tx / '0')
        self.remove()
        self.assertFalse((self.target / 'current').exists())
        self.install()
        tx.mkdir()
        actions[0]['path'] = '../outside'
        (tx / 'journal.json').write_text(json.dumps(journal))
        with self.assertRaisesRegex(ValueError, 'journal path'):
            self.remove()
        self.assertTrue((self.target / 'current').exists())

    def test_lock_and_session_failure_prevent_removal(self):
        with (self.target / '.vitrallis-update/lock').open('r+') as lock:
            fcntl.flock(lock, fcntl.LOCK_EX | fcntl.LOCK_NB)
            with self.assertRaises(BlockingIOError):
                self.remove()
        with patch.object(u, 'session', side_effect=ValueError('unrelated session')):
            with self.assertRaisesRegex(ValueError, 'unrelated session'):
                self.remove()
        self.assertTrue((self.target / 'current').exists())

    def test_stale_update_download_and_partial_stages_are_removed(self):
        stage = self.target / '.vitrallis-update'
        (stage / 'download').write_bytes(b'partial network download')
        generation = stage / 'install-1234abcd/generation'
        generation.mkdir(parents=True)
        (generation / 'vitrallis').write_bytes(b'partial binary')
        self.remove()
        self.assertFalse((stage / 'download').exists())
        self.assertFalse((stage / 'install-1234abcd').exists())
        self.assertTrue((stage / 'lock').exists())

    def test_modified_generation_and_unrelated_generation_contents_survive(self):
        generation = self.target / os.readlink(self.target / 'current')
        path = generation / 'vitrallis'
        path.write_bytes(path.read_bytes() + b'user edit')
        self.remove()
        self.assertTrue(path.read_bytes().endswith(b'user edit'))
        self.assertFalse((self.target / 'current').exists())

    def test_no_launcher_config_is_required_for_removal(self):
        self.config.unlink()
        self.config.parent.rmdir()
        self.remove()
        self.assertFalse(self.config.exists())
        self.assertFalse((self.target / 'current').exists())

    def test_unrelated_broken_config_is_not_read(self):
        self.config.write_text('broken user config')
        self.remove()
        self.assertEqual(self.config.read_text(), 'broken user config')

    def test_recovery_preserves_later_edits_and_leaves_journal(self):
        owned_config = self.home / u.AWESOME
        owned_config.parent.mkdir(parents=True)
        owned_config.write_text('before\n' + u.STARTUP + '\nafter\n')
        real = u.atomic
        def fail(path, *args):
            if path == owned_config:
                path.write_text('later user edit')
                raise OSError('injected error')
            return real(path, *args)
        with patch.object(u, 'atomic', side_effect=fail), self.assertRaisesRegex(ValueError, 'Later edit'):
            self.remove()
        self.assertEqual(owned_config.read_text(), 'later user edit')
        self.assertTrue((self.target / '.vitrallis-update/removal/journal.json').is_file())
        self.assertTrue((self.target / 'uninstall.py').is_file())

    def test_interrupted_committed_cleanup_is_repeatable(self):
        real = u.cleanup
        def interrupt(transaction, journal, committed):
            if committed:
                raise OSError('interrupted cleanup')
            return real(transaction, journal, committed)
        with patch.object(u, 'cleanup', side_effect=interrupt), self.assertRaisesRegex(OSError, 'interrupted cleanup'):
            self.remove()
        self.assertTrue((self.target / 'uninstall.py').is_file())
        self.assertTrue((self.target / 'install-session.py').is_file())
        self.remove()
        self.assertFalse((self.target / 'uninstall.py').exists())
        self.assertFalse((self.target / 'current').exists())

    def test_local_cli_survives_interruption_after_installer_helper_removal(self):
        entry = self.target / 'uninstall.py'
        original = Path.unlink
        failed = []
        def unlink(path, *args, **kwargs):
            if path == entry and not failed:
                failed.append(True)
                raise OSError('interrupted final helper cleanup')
            return original(path, *args, **kwargs)
        with patch.object(Path, 'unlink', side_effect=None, autospec=True) as remove:
            remove.side_effect = unlink
            with self.assertRaisesRegex(OSError, 'final helper cleanup'):
                self.remove()
        self.assertTrue(entry.is_file())
        self.assertFalse((self.target / 'install-session.py').exists())
        code = "import runpy, sys; from unittest.mock import patch; " + \
               "sys.argv = sys.argv[1:]; " + \
               "guard = patch('os.geteuid', return_value=1000); guard.start(); " + \
               "runpy.run_path(sys.argv[0], run_name='__main__')"
        result = subprocess.run([sys.executable, '-c', code, str(entry)],
                                env=dict(os.environ, HOME=str(self.home)),
                                capture_output=True, text=True, check=False)
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertFalse(entry.exists())
        self.assertFalse((self.target / 'installed.json').exists())

    def test_partial_install_uses_pending_receipt(self):
        receipt = json.loads((self.target / 'installed.json').read_bytes())
        (self.target / '.installation-pending').write_text(json.dumps({'schema': 1, 'receipt': receipt, 'hashes': {}}))
        (self.target / 'installed.json').unlink()
        (self.target / 'vitrallis-session.py').unlink()
        self.remove()
        self.assertFalse((self.target / 'current').exists())


if __name__ == '__main__':
    unittest.main()
