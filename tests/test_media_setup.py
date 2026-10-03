"""Fixed multimedia-helper provisioning under the public installer's umask."""
import importlib.util
import os
from pathlib import Path
import stat
import tempfile
from types import SimpleNamespace
import unittest
from unittest.mock import patch

ROOT = Path(__file__).resolve().parents[1]
spec = importlib.util.spec_from_file_location('media_setup', ROOT / 'integrations/pocketchip/media-setup.py')
m = importlib.util.module_from_spec(spec)
spec.loader.exec_module(m)


class Provisioning(unittest.TestCase):
    def test_private_install_umask_leaves_the_fixed_helper_executable_by_the_desktop_user(self):
        with tempfile.TemporaryDirectory() as temp:
            root = Path(temp)
            local, etc = root / 'usr/local', root / 'etc'
            local.mkdir(parents=True)
            etc.mkdir()
            (root / 'debian_version').write_text('13.7\n')
            paths = {
                '/etc/debian_version': root / 'debian_version',
                '/usr/local/libexec/vitrallis-carousel-install-media': local / 'libexec/vitrallis-carousel-install-media',
                '/etc/sudoers.d': etc / 'sudoers.d',
            }
            previous = os.umask(0o077)
            try:
                # Only root identity and host absolute paths are replaced. The
                # production creation, validation, writes and modes run unchanged.
                with patch.object(m, 'Path', side_effect=lambda value: paths[value]), \
                        patch.object(m, 'secure'), patch.object(m.os, 'geteuid', return_value=0), \
                        patch.object(m.pwd, 'getpwnam', return_value=SimpleNamespace(pw_uid=1000)), \
                        patch.object(m.subprocess, 'run') as visudo:
                    m.install('chip')
                    m.install('chip')
            finally:
                os.umask(previous)
            helper = paths['/usr/local/libexec/vitrallis-carousel-install-media']
            rule = paths['/etc/sudoers.d'] / 'vitrallis-carousel-media-chip'
            self.assertEqual(stat.S_IMODE(helper.parent.stat().st_mode), 0o755)
            self.assertEqual(stat.S_IMODE(helper.stat().st_mode), 0o755)
            self.assertEqual(stat.S_IMODE(rule.stat().st_mode), 0o440)
            self.assertEqual(helper.read_bytes(), m.HELPER)
            self.assertEqual(rule.read_bytes(), m.policy('chip'))
            self.assertEqual(visudo.call_count, 2)

    def test_unsafe_parent_prevents_creating_a_helper_directory(self):
        with tempfile.TemporaryDirectory() as temp:
            root = Path(temp)
            (root / 'debian_version').write_text('13.7\n')
            helper = root / 'libexec/vitrallis-carousel-install-media'
            paths = {'/etc/debian_version': root / 'debian_version',
                     '/usr/local/libexec/vitrallis-carousel-install-media': helper,
                     '/etc/sudoers.d': root / 'sudoers.d'}
            with patch.object(m, 'Path', side_effect=lambda value: paths[value]), \
                    patch.object(m, 'secure', side_effect=ValueError('unsafe root path')), \
                    patch.object(m.os, 'geteuid', return_value=0), \
                    patch.object(m.pwd, 'getpwnam', return_value=SimpleNamespace(pw_uid=1000)):
                with self.assertRaisesRegex(ValueError, 'unsafe'):
                    m.install('chip')
            self.assertFalse(helper.parent.exists())
            self.assertFalse(paths['/etc/sudoers.d'].exists())
