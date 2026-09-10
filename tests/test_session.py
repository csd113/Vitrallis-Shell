import importlib.util
import os
from pathlib import Path
import shutil
import subprocess
import tempfile
import unittest
from unittest.mock import patch

spec = importlib.util.spec_from_file_location('session', Path(__file__).resolve().parents[1] / 'devices/pocketchip/vitrallis-session.py')
s = importlib.util.module_from_spec(spec)
spec.loader.exec_module(s)


class Session(unittest.TestCase):
    def test_explicit_argv_and_cleanup_policy(self):
        env = dict(DISPLAY=':0', XAUTHORITY='/home/chip/.Xauthority', DBUS_SESSION_BUS_ADDRESS='unix:path=/run/user/1000/bus')
        args = s.launch_command(Path('/space here/100%/session.py'), env)
        self.assertIn('--property=KillMode=control-group', args)
        self.assertIn('--property=Restart=no', args)
        self.assertIn('--property=TimeoutStopSec=5', args)
        self.assertEqual(args[-2:], ['/space here/100%/session.py', 'run'])
        self.assertIn('--property=ExecStopPost=/usr/bin/python3 "/space here/100%%/session.py" restore', args)
        with self.assertRaises(ValueError):
            s.launch_command(Path('/session.py'), {})

    def test_log_growth_bounded_and_symlinks_rejected(self):
        with tempfile.TemporaryDirectory() as temp:
            root = Path(temp).resolve()
            log = root / 'session.log'
            for _ in range(50):
                s.log_chunk(log, b'x' * 8192)
            self.assertLessEqual(log.stat().st_size, s.LIMIT)
            self.assertLessEqual(log.with_suffix('.log.1').stat().st_size, s.LIMIT)
            log.unlink()
            log.symlink_to(root / 'outside')
            with self.assertRaises(ValueError):
                s.log_chunk(log, b'bad')
            self.assertFalse((root / 'outside').exists())

    def test_missing_binary_never_changes_home_routing(self):
        with tempfile.TemporaryDirectory() as temp, patch.object(s, 'awesome') as awesome:
            with self.assertRaises(RuntimeError):
                s.supervise(Path(temp).resolve())
            awesome.assert_not_called()

    def test_hardlinked_log_and_dangling_marker_are_rejected(self):
        with tempfile.TemporaryDirectory() as temp:
            root = Path(temp).resolve()
            outside = root / 'outside'
            outside.write_bytes(b'preserve')
            log = root / 'session.log'
            os.link(outside, log)
            with self.assertRaisesRegex(ValueError, 'hardlink'):
                s.log_chunk(log, b'bad')
            self.assertEqual(outside.read_bytes(), b'preserve')
            (root / '.installation-pending').symlink_to(root / 'missing')
            with patch.object(s, 'awesome') as awesome:
                with self.assertRaisesRegex(ValueError, 'symlink'):
                    s.supervise(root)
                awesome.assert_not_called()

    def test_run_resolves_binary_beside_the_session_script(self):
        with tempfile.TemporaryDirectory() as temp:
            script = Path(temp).resolve() / 'installed session with spaces/vitrallis-session.py'
            with patch.object(s, '__file__', str(script)), \
                    patch.object(s.sys, 'argv', [str(script), 'run']), \
                    patch.object(s, 'supervise', return_value=17) as supervise:
                self.assertEqual(s.main(), 17)
                supervise.assert_called_once_with(script.parent)

    def test_run_wrapper_preserves_environment_arguments_cwd_and_exit_status(self):
        with tempfile.TemporaryDirectory() as temp:
            root = Path(temp).resolve()
            home = root / 'user home'
            launcher = home / '.local/share/vitrallis/launch'
            launcher.parent.mkdir(parents=True)
            launcher.write_text(
                '#!/bin/sh\n'
                'printf "%s\\n" "$HOME" "$PWD" "$DISPLAY" "$XAUTHORITY" "$@"\n'
                'exit 17\n'
            )
            launcher.chmod(0o755)
            package = root / 'package with spaces'
            package.mkdir()
            wrapper = package / 'run-pocketchip.sh'
            shutil.copyfile(Path(s.__file__).parent / 'run-pocketchip.sh', wrapper)
            cwd = root / 'unrelated directory'
            cwd.mkdir()
            args = ['argument with spaces', 'literal $HOME']
            result = subprocess.run(
                ['sh', str(wrapper), *args], cwd=cwd,
                env=dict(os.environ, HOME=str(home), DISPLAY=':91', XAUTHORITY='fixture auth'),
                capture_output=True, text=True, check=False,
            )
            self.assertEqual(result.returncode, 17, result.stderr)
            self.assertEqual(result.stdout.splitlines(), [str(home), str(cwd), ':91', 'fixture auth', *args])


if __name__ == '__main__':
    unittest.main()
