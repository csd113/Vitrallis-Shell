import importlib.util
from pathlib import Path
import tempfile
import unittest
from unittest.mock import patch

spec = importlib.util.spec_from_file_location('session', Path(__file__).parents[1] / 'scripts/vitrallis-session.py')
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


if __name__ == '__main__':
    unittest.main()
