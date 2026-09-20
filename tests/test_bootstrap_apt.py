"""Opt-in, offline APT integration in a disposable Debian container only."""
import os
from pathlib import Path
import subprocess
import tempfile
import unittest

ROOT = Path(__file__).resolve().parents[1]
ENABLED = (os.environ.get('VITRALLIS_APT_FIXTURE') == '1'
           and Path('/.dockerenv').exists() and os.geteuid() == 0)


@unittest.skipUnless(ENABLED, 'requires explicit disposable Debian container APT fixture')
class AptArchiveGuard(unittest.TestCase):
    def test_real_apt_accepts_planned_new_package_and_rejects_changed_transaction(self):
        source = (ROOT / 'integrations/pocketchip/bootstrap.sh').read_text()
        guard_source = source[source.index("    vitrallis_guard='"):source.index('\n    sudo --', source.index("    vitrallis_guard='"))]
        def guard(plan):
            return subprocess.check_output(['/bin/sh', '-c', 'vitrallis_plan=$1\n' + guard_source
                                            + '\nprintf "%s" "$vitrallis_guard"', 'guard', plan], text=True)
        with tempfile.TemporaryDirectory(prefix='vitrallis-apt-') as temporary:
            root = Path(temporary)
            def package(name, version):
                stage = root / (name + '-' + version)
                (stage / 'DEBIAN').mkdir(parents=True)
                (stage / 'DEBIAN/control').write_text(
                    'Package: ' + name + '\nVersion: ' + version + '\nArchitecture: all\n'
                    'Maintainer: Fixture <fixture@example.invalid>\nDescription: disposable test data\n')
                (stage / 'usr/share' / name).mkdir(parents=True)
                (stage / 'usr/share' / name / 'version').write_text(version)
                output = stage.with_suffix('.deb')
                subprocess.run(['dpkg-deb', '--build', str(stage), str(output)], check=True, capture_output=True)
                return output
            def install(archive, plan):
                return subprocess.run(['/usr/bin/apt-get', '--no-install-recommends', '--no-remove', '-y',
                                       '-o', 'DPkg::Pre-Install-Pkgs::=' + guard(plan), 'install', str(archive)],
                                      text=True, capture_output=True, timeout=60)
            name = 'vitrallis-bootstrap-test'
            first = install(package(name, '1.0'), name + '=1.0 ')
            self.assertEqual(first.returncode, 0, first.stdout + first.stderr)
            replacement = install(package(name, '2.0'), name + '=2.0 ')
            self.assertNotEqual(replacement.returncode, 0)
            self.assertIn('refused to replace', replacement.stderr)
            self.assertEqual(subprocess.check_output(['dpkg-query', '-W', '-f=${Version}', name], text=True), '1.0')
            unexpected = install(package(name + '-unexpected', '1.0'), name + '=1.0 ')
            self.assertNotEqual(unexpected.returncode, 0)
            self.assertIn('unplanned package', unexpected.stderr)
            self.assertFalse(Path('/usr/share/' + name + '-unexpected').exists())


if __name__ == '__main__':
    unittest.main()
