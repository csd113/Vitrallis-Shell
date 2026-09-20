"""One-paste prerequisites and post-install startup without host package changes."""
import contextlib
import io
import os
from pathlib import Path
import re
import stat
import subprocess
import tempfile
import unittest
from unittest.mock import Mock, patch

from bootstrap_fixture import ShellFixture
from test_bootstrap import b, DEVICE, ROOT


class Prerequisites(unittest.TestCase):
    def setUp(self):
        self.temp = tempfile.TemporaryDirectory(prefix='vtr-', dir='/tmp')
        self.addCleanup(self.temp.cleanup)
        source = (DEVICE / 'bootstrap.sh').read_text()
        self.fixture = ShellFixture(Path(self.temp.name), source)
        self.addCleanup(self.fixture.bus.close)

    def run_script(self):
        return subprocess.run(['/bin/sh', '-c', self.fixture.script], env=self.fixture.env,
                              text=True, capture_output=True, timeout=15)

    def test_guide_block_is_the_canonical_entry_point(self):
        guide = (ROOT / 'docs/devices/pocketchip.md').read_text()
        block = re.search(r'<!-- pocketchip-bootstrap -->\n```sh\n(.*?)\n```', guide, re.S).group(1)
        self.assertEqual(block, (DEVICE / 'bootstrap.sh').read_text().split('\n', 2)[2].rstrip())

    def test_missing_packages_are_planned_pinned_verified_then_downloaded(self):
        result = self.run_script()
        self.assertEqual(result.returncode, 0, result.stderr)
        calls = self.fixture.calls()
        self.assertEqual(len(calls), 4)
        self.assertEqual(calls[0], ['-v'])
        self.assertIn('--simulate', calls[2])
        self.assertEqual(calls[2][-1], 'picom')
        self.assertEqual(calls[3][-1], 'picom=12.5-1')
        self.assertIn('--no-remove', calls[3])
        self.assertIn('--no-upgrade', calls[3])
        self.assertTrue(any('DPkg::Pre-Install-Pkgs::=' in arg for arg in calls[3]))
        self.assertTrue((self.fixture.root / 'bootstrap-executed').exists())
        self.assertFalse(Path((self.fixture.root / 'download-path').read_text()).exists())

    def test_repeat_with_complete_packages_never_runs_apt(self):
        self.fixture.marker.touch()
        result = self.run_script()
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertEqual(self.fixture.calls(), [])

    def test_upgrade_removal_unrelated_repair_and_injection_fail_before_install(self):
        for plan in ('Inst libc6 [2.36] (2.41 Debian [armhf])\n', 'Remv awesome [4.3]\n',
                     'Conf broken-package (1.0 Debian [armhf])\n',
                     'Inst picom (1;touch-pwned Debian [armhf])\n', ''):
            with self.subTest(plan=plan):
                self.fixture.plan.write_text(plan)
                self.fixture.log.unlink(missing_ok=True)
                result = self.run_script()
                self.assertNotEqual(result.returncode, 0)
                self.assertFalse(self.fixture.marker.exists())
                self.assertFalse((self.fixture.root / 'bootstrap-executed').exists())
                self.assertEqual(len(self.fixture.calls()), 3)

    def test_root_old_os_wrong_board_and_running_session_never_mutate(self):
        for name, code in [('id', 'print("0")'), ('uname', 'print("Darwin")'),
                           ('systemctl', 'print("active")')]:
            original = (self.fixture.commands / name).read_text()
            self.fixture.write(name, code)
            result = self.run_script()
            self.assertNotEqual(result.returncode, 0)
            self.assertEqual(self.fixture.calls(), [])
            (self.fixture.commands / name).write_text(original)
        self.fixture.os_release.write_text('ID=debian\nVERSION_ID=8\n')
        self.assertNotEqual(self.run_script().returncode, 0)
        self.assertEqual(self.fixture.calls(), [])

    def test_no_user_bus_or_disk_space_fails_before_packages(self):
        self.fixture.write('df', 'print("fixture 100 99 1 99% /")')
        result = self.run_script()
        self.assertNotEqual(result.returncode, 0)
        self.assertIn('128 MiB', result.stderr)
        self.assertEqual(self.fixture.calls(), [])
        (self.fixture.runtime / 'bus').unlink()
        result = self.run_script()
        self.assertNotEqual(result.returncode, 0)
        self.assertIn('desktop user bus', result.stderr)
        self.assertEqual(self.fixture.calls(), [])

    def test_network_package_and_download_failure_stop_and_clean(self):
        for flag in ('FIXTURE_UPDATE_FAIL', 'FIXTURE_INSTALL_FAIL', 'FIXTURE_DOWNLOAD_FAIL'):
            with self.subTest(flag=flag):
                self.fixture.env[flag] = '1'
                result = self.run_script()
                self.assertNotEqual(result.returncode, 0)
                self.assertFalse((self.fixture.root / 'bootstrap-executed').exists())
                download = self.fixture.root / 'download-path'
                if download.exists():
                    self.assertFalse(Path(download.read_text()).exists())
                self.fixture.env.pop(flag)

    def test_root_archive_guard_rejects_replacement_and_unplanned_archives(self):
        self.assertEqual(self.run_script().returncode, 0)
        option = next(a for a in self.fixture.calls()[-1] if a.startswith('DPkg::Pre-Install-Pkgs::='))
        guard = option.split('=', 1)[1]
        self.fixture.write('dpkg-deb', 'print({"Package": "picom", "Version": "12.5-1", "Architecture": "armhf"}[sys.argv[-1]])')
        self.fixture.write('dpkg-query', 'sys.exit(1)')
        guard = guard.replace('/usr/bin/dpkg-deb', str(self.fixture.commands / 'dpkg-deb'))
        guard = guard.replace('/usr/bin/dpkg-query', str(self.fixture.commands / 'dpkg-query'))
        guard = guard.replace('/usr/bin/dpkg ', str(self.fixture.commands / 'dpkg') + ' ')
        def execute():
            return subprocess.run(['/bin/sh', '-c', guard], input='/tmp/example.deb\n',
                                  text=True, capture_output=True)
        self.assertEqual(execute().returncode, 0, execute().stderr)
        self.fixture.write('dpkg-query', 'assert sys.argv[2] == "-f=${Status}"; print("install ok installed")')
        self.assertNotEqual(execute().returncode, 0)
        self.fixture.write('dpkg-query', 'sys.exit(2)')
        self.assertNotEqual(execute().returncode, 0)
        self.fixture.write('dpkg-query', 'sys.exit(1)')
        self.fixture.write('dpkg-deb', 'print({"Package": "unexpected", "Version": "12.5-1", "Architecture": "armhf"}[sys.argv[-1]])')
        self.assertNotEqual(execute().returncode, 0)


class FirstLaunch(unittest.TestCase):
    def setUp(self):
        self.env = dict(DISPLAY=':0', XAUTHORITY='/home/chip/.Xauthority',
                        DBUS_SESSION_BUS_ADDRESS='unix:path=/run/user/1000/bus')

    def test_ssh_including_forwarded_display_never_contacts_desktop(self):
        for key in ('SSH_CONNECTION', 'SSH_CLIENT', 'SSH_TTY'):
            with patch.object(b, 'command_output') as command:
                self.assertFalse(b.desktop_available(dict(self.env, **{key: 'present'})))
                command.assert_not_called()
        with patch.object(b, 'command_output') as command:
            self.assertFalse(b.desktop_available(dict(self.env, DISPLAY='localhost:10.0')))
            self.assertFalse(b.desktop_available({}))
            command.assert_not_called()

    def test_local_desktop_requires_successful_awesome_connection(self):
        with patch.object(b, 'command_output', return_value='string "vitrallis-desktop-available"'):
            self.assertTrue(b.desktop_available(self.env))
        with patch.object(b, 'command_output', side_effect=ValueError('no bus')):
            self.assertFalse(b.desktop_available(self.env))

    def test_remote_completion_preserves_install_without_launch(self):
        output = io.StringIO()
        with patch.dict(os.environ, {'SSH_CONNECTION': 'present'}), \
                patch.object(b, 'load_session') as load, contextlib.redirect_stdout(output):
            b.finish_install(Path('/verified'))
        load.assert_not_called()
        self.assertIn('installed successfully', output.getvalue())
        self.assertIn('~/.local/share/vitrallis/launch', output.getvalue())

    def test_success_waits_for_window_and_failed_start_is_reported_as_installed(self):
        session = Mock()
        session.owned_session.side_effect = [None, ('123', 'start'), ('123', 'start')]
        with patch.object(b, 'desktop_available', return_value=True), patch.object(b, 'load_session', return_value=session), \
                patch.object(b, 'command_output') as launch, patch.object(b, 'wait_for_desktop') as wait:
            b.finish_install(Path('/verified'))
            launch.assert_called_once()
            wait.assert_called_once()
            session.stop_owned.assert_not_called()
        session.owned_session.side_effect = [None, ('123', 'start'), ('123', 'start')]
        with patch.object(b, 'desktop_available', return_value=True), patch.object(b, 'load_session', return_value=session), \
                patch.object(b, 'command_output'), patch.object(b, 'wait_for_desktop', side_effect=ValueError('timeout')):
            with self.assertRaisesRegex(ValueError, 'Installation succeeded, but launch failed'):
                b.finish_install(Path('/verified'))
        session.stop_owned.assert_called_once()

    def test_concurrent_session_is_never_stopped_or_launched_over(self):
        session = Mock()
        session.owned_session.return_value = ('123', 'start')
        with patch.object(b, 'desktop_available', return_value=True), patch.object(b, 'load_session', return_value=session), \
                patch.object(b, 'command_output') as launch:
            with self.assertRaisesRegex(ValueError, 'already started'):
                b.finish_install(Path('/verified'))
        launch.assert_not_called()
        session.stop_owned.assert_not_called()

    def test_changed_session_is_preserved_after_startup_failure(self):
        session = Mock()
        session.owned_session.side_effect = [None, ('123', 'start'), ('456', 'new')]
        with patch.object(b, 'desktop_available', return_value=True), patch.object(b, 'load_session', return_value=session), \
                patch.object(b, 'command_output'), patch.object(b, 'wait_for_desktop', side_effect=ValueError('changed')):
            with self.assertRaisesRegex(ValueError, 'session changed; it was left running'):
                b.finish_install(Path('/verified'))
        session.stop_owned.assert_not_called()

    def test_visible_window_requires_matching_process_generation_and_supervisor(self):
        session = Mock()
        session.owned_session.return_value = ('123', 'start')
        with patch.object(b.Path, 'resolve', return_value=Path('/generation/vitrallis')), \
                patch.object(b.Path, 'read_text', side_effect=['999 (other window) S 987 0', '456 (vitrallis) S 123 0']), \
                patch.object(b.Path, 'stat', return_value=Mock(st_uid=os.getuid())), \
                patch.object(b, 'command_output', return_value='string "vitrallis-ready:999,vitrallis-ready:456"'):
            b.wait_for_desktop(session, Path('/installed/vitrallis-session.py'), self.env, ('123', 'start'))
        session.owned_session.return_value = None
        with patch.object(b.Path, 'resolve', return_value=Path('/generation/vitrallis')):
            with self.assertRaisesRegex(ValueError, 'exited or changed'):
                b.wait_for_desktop(session, Path('/installed/vitrallis-session.py'), self.env, ('123', 'start'))


    def test_invisible_window_times_out(self):
        session = Mock()
        session.owned_session.return_value = ('123', 'start')
        with patch.object(b.Path, 'resolve', return_value=Path('/generation/vitrallis')), \
                patch.object(b.time, 'monotonic', side_effect=[0, 1, 2, 21]), \
                patch.object(b.time, 'sleep'), patch.object(b, 'command_output', return_value='string "vitrallis-starting"'):
            with self.assertRaisesRegex(ValueError, '20 seconds'):
                b.wait_for_desktop(session, Path('/installed/vitrallis-session.py'), self.env, ('123', 'start'))


class InstallationEnvironment(unittest.TestCase):
    def setUp(self):
        self.addCleanup(patch.stopall)
        patch.object(b.os, 'getuid', return_value=1000).start()
        patch.object(b.os, 'geteuid', return_value=1000).start()
        patch.object(b.os, 'uname', return_value=Mock(sysname='Linux', machine='armv7l')).start()
        patch.object(b.Path, 'read_bytes', return_value=b'nextthing,pocketchip\0').start()
        patch.object(b.Path, 'home', return_value=Path('/home/chip')).start()
        patch.object(b.Path, 'resolve', lambda path: path).start()
        patch.object(b.pwd, 'getpwuid', return_value=Mock(pw_dir='/home/chip')).start()
        self.lstat = patch.object(b.Path, 'lstat', side_effect=[
            Mock(st_mode=stat.S_IFDIR | 0o700, st_uid=1000),
            Mock(st_mode=stat.S_IFSOCK | 0o660, st_uid=1000)]).start()
        self.command = patch.object(b, 'command_output', return_value='inactive').start()

    def test_ssh_uses_validated_user_bus_without_changing_display_credentials(self):
        with patch.dict(os.environ, {'SSH_CONNECTION': 'present', 'DISPLAY': 'localhost:10.0',
                                    'DBUS_SESSION_BUS_ADDRESS': 'untrusted-value'}):
            env = b.installation_environment()
            self.assertEqual(env['DBUS_SESSION_BUS_ADDRESS'], 'unix:path=/run/user/1000/bus')
            self.assertEqual(env['DISPLAY'], 'localhost:10.0')
            self.assertEqual(os.environ['DBUS_SESSION_BUS_ADDRESS'], 'untrusted-value')
            self.assertIs(self.command.call_args.args[1], env)

    def test_unsafe_runtime_or_missing_manager_is_rejected_before_query(self):
        self.lstat.side_effect = [Mock(st_mode=stat.S_IFLNK | 0o777, st_uid=1000),
                                 Mock(st_mode=stat.S_IFSOCK | 0o660, st_uid=1000)]
        with self.assertRaisesRegex(ValueError, 'safe desktop user bus'):
            b.installation_environment()
        self.command.assert_not_called()
        self.lstat.side_effect = FileNotFoundError()
        with self.assertRaisesRegex(ValueError, 'Log into'):
            b.installation_environment()

    def test_running_session_and_insufficient_space_block_download(self):
        self.command.return_value = 'active'
        with self.assertRaisesRegex(ValueError, 'Save your work'):
            b.installation_environment()
        release = {'assets': [{'name': b.BUNDLE, 'size': 1000}]}
        with patch.object(b.shutil, 'disk_usage', return_value=Mock(free=1)):
            with self.assertRaisesRegex(ValueError, 'Insufficient free space'):
                b.check_download_space(release, Path('/tmp'))



if __name__ == '__main__':
    unittest.main()
