"""Opt-in real Awesome/X11 readiness check in the disposable simulator."""
import importlib.util
import os
from pathlib import Path
import subprocess
import tempfile
import time
from types import SimpleNamespace
import unittest

ROOT = Path(__file__).resolve().parents[1]
spec = importlib.util.spec_from_file_location('bootstrap_desktop', ROOT / 'integrations/pocketchip/bootstrap.py')
b = importlib.util.module_from_spec(spec)
spec.loader.exec_module(b)
ENABLED = os.environ.get('VITRALLIS_DESKTOP_FIXTURE') == '1' and Path('/.dockerenv').exists()


@unittest.skipUnless(ENABLED, 'requires explicit disposable Awesome/X11 simulator')
class DesktopReadiness(unittest.TestCase):
    def test_real_visible_window_pid_and_parent_are_verified(self):
        with tempfile.TemporaryDirectory(prefix='vitrallis-desktop-') as temporary:
            root = Path(temporary)
            env = dict(os.environ, DISPLAY=':97', XAUTHORITY=str(root / 'Xauthority'), HOME=str(root))
            for key in ('SSH_CONNECTION', 'SSH_CLIENT', 'SSH_TTY'):
                env.pop(key, None)
            processes = []
            try:
                for command in (['Xvfb', ':97', '-screen', '0', '480x272x24', '-nolisten', 'tcp'],
                                ['awesome', '--config', '/etc/xdg/awesome/rc.lua']):
                    processes.append(subprocess.Popen(command, env=env, stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL))
                    time.sleep(0.5)
                for _ in range(30):
                    if b.desktop_available(env):
                        break
                    time.sleep(0.1)
                self.assertTrue(b.desktop_available(env))
                (root / 'current').mkdir()
                (root / 'current/vitrallis').symlink_to('/usr/bin/xterm')
                processes.append(subprocess.Popen(['/usr/bin/xterm', '-T', 'Vitrallis'], env=env,
                                                   stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL))
                identity = (str(os.getpid()), 'fixture-supervisor')
                # Container has no systemd user manager. The graphical connection,
                # window visibility, PID, executable and parent are all real.
                session = SimpleNamespace(owned_session=lambda script: identity)
                b.wait_for_desktop(session, root / 'vitrallis-session.py', env, identity)
            finally:
                for process in reversed(processes):
                    process.terminate()
                    try:
                        process.wait(timeout=5)
                    except subprocess.TimeoutExpired:
                        process.kill()
                        process.wait(timeout=5)


if __name__ == '__main__':
    unittest.main()
