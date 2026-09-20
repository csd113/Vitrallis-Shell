"""Offline regression tests for the exact Python helpers embedded by Rust."""
import importlib.util
import os
from pathlib import Path
import subprocess
import sys
import tempfile
import unittest
from unittest.mock import patch
import venv
import zipfile

ROOT = Path(__file__).resolve().parents[1]
SOURCE = (ROOT / 'src/app_center/runtime.rs').read_text()
PROVISION = SOURCE.split('const PROVISION: &str = r"', 1)[1].split('\n";', 1)[0]
VALIDATE = (ROOT / 'src/app_center/python_requirements.py').read_text()
spec = importlib.util.spec_from_file_location('requirements', ROOT / 'src/app_center/python_requirements.py')
requirements = importlib.util.module_from_spec(spec)
spec.loader.exec_module(requirements)


class RuntimeTests(unittest.TestCase):
    def test_policy_rejects_unsupported_and_inactive_extras(self):
        for line in ['--target=/tmp/unsafe', 'Pillow @ https://example.com/a.whl',
                     './local', 'Pillow[extra]', 'Pillow[extra]; python_version < "1"',
                     'Pillow>=10 --target=x', 'Pillow>=', 'Pillow==1.*.*']:
            with self.subTest(line=line), self.assertRaises((RuntimeError, ValueError)):
                requirements.check('validate', [line])

    def test_versions_markers_and_missing_distributions(self):
        with patch.object(requirements.metadata, 'version', return_value='11.1.0'):
            requirements.check('plain', ['Pillow>=10.4,<13'])
            with self.assertRaises(RuntimeError):
                requirements.check('plain', ['Pillow>=12,<13'])
        with patch.object(requirements.metadata, 'version', side_effect=requirements.metadata.PackageNotFoundError):
            requirements.check('plain', ['missing; python_version < "1"'])
            with self.assertRaises(requirements.metadata.PackageNotFoundError):
                requirements.check('plain', ['missing>=1'])

    def test_all_failure_stages_clean_up_and_preserve_existing_environment(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory) / 'environment'
            previous = Path(directory) / 'previous'
            previous.mkdir()
            (previous / 'user-data').write_text('keep')
            for fail in range(4):
                with patch('venv.EnvBuilder') as builder, patch('subprocess.run') as run:
                    if fail == 0:
                        builder.return_value.create.side_effect = RuntimeError('venv unavailable')
                    else:
                        run.side_effect = [None] * (fail - 1) + [subprocess.CalledProcessError(1, 'fixture')]
                    with patch.dict(os.environ), patch.object(sys, 'argv', ['installer', 'tk', str(root), 'Pillow>=10.4,<13', VALIDATE]):
                        with self.assertRaises((RuntimeError, subprocess.CalledProcessError)):
                            exec(PROVISION, {})
                self.assertFalse(root.exists())
                self.assertEqual(list(Path(directory).iterdir()), [previous])
                self.assertEqual((previous / 'user-data').read_text(), 'keep')

    def test_real_pip_reuses_compatible_distribution_and_installs_missing_locally(self):
        # No network or machine-wide writes. A fixture .pth models the system
        # site visible to a --system-site-packages venv. A newer heavy wheel
        # must not displace an already-compatible system installation.
        with tempfile.TemporaryDirectory() as directory:
            directory = Path(directory)
            system = directory / 'system'
            system.mkdir()
            self.distribution(system, 'heavy', '11.1.0')
            wheelhouse = directory / 'wheels'
            wheelhouse.mkdir()
            for name, version in [('light', '8.2'), ('packaging', '25.0'), ('heavy', '12.0')]:
                payload = directory / name
                payload.mkdir()
                self.distribution(payload, name, version)
                with zipfile.ZipFile(wheelhouse / f'{name}-{version}-py3-none-any.whl', 'w') as wheel:
                    for path in payload.rglob('*'):
                        if path.is_file():
                            wheel.write(path, path.relative_to(payload))
            before = {str(p): p.read_bytes() for p in system.rglob('*') if p.is_file()}
            original_create, original_run = venv.EnvBuilder.create, subprocess.run
            root = directory / 'environment'
            calls = []

            def create(builder, staging):
                self.assertTrue(builder.system_site_packages)
                original_create(builder, staging)
                site = next((Path(staging) / 'lib').glob('python*/site-packages'))
                (site / 'system-fixture.pth').write_text(str(system) + '\n')

            def run(command, **kwargs):
                if 'ensurepip' in command:
                    return original_run(command, **kwargs)
                calls.append(command)
                self.assertEqual(kwargs['env']['PIP_CONFIG_FILE'], os.devnull)
                self.assertNotIn('PIP_TARGET', kwargs['env'])
                self.assertNotIn('PYTHONPATH', kwargs['env'])
                if 'pip' in command:
                    command = command + ['--no-index', '--find-links', str(wheelhouse)]
                return original_run(command, **kwargs)

            with patch.object(venv.EnvBuilder, 'create', create), patch('subprocess.run', run), patch.dict(os.environ, {'PIP_TARGET': str(directory / 'escaped'), 'PYTHONPATH': str(directory / 'poison')}), patch.object(sys, 'argv', ['installer', 'plain', str(root), 'heavy>=10.4,<13\nlight>=7.4,<9', VALIDATE]):
                exec(PROVISION, {})
            result = subprocess.run([str(root / 'bin/python3'), '-I', '-c', 'import heavy,light,site; print(heavy.__file__); print(light.__file__); assert not site.ENABLE_USER_SITE'], check=True, capture_output=True, text=True)
            self.assertIn(str(system / 'heavy.py'), result.stdout)
            self.assertIn(str(root), result.stdout)
            self.assertEqual(before, {str(p): p.read_bytes() for p in system.rglob('*') if p.is_file() and '__pycache__' not in str(p)})
            self.assertFalse((directory / 'escaped').exists())
            self.assertFalse(list(directory.glob('.pending-*')))
            self.assertEqual(calls[-1][4], 'plain')
            # An incompatible system version must instead be shadowed locally.
            upgraded = directory / 'upgraded'
            with patch.object(venv.EnvBuilder, 'create', create), patch('subprocess.run', run), patch.dict(os.environ), patch.object(sys, 'argv', ['installer', 'plain', str(upgraded), 'heavy>=12,<13', VALIDATE]):
                exec(PROVISION, {})
            result = subprocess.run([str(upgraded / 'bin/python3'), '-I', '-c', 'import heavy; assert heavy.VERSION == "12.0"; print(heavy.__file__)'], check=True, capture_output=True, text=True)
            self.assertIn(str(upgraded), result.stdout)
            self.assertEqual(before, {str(p): p.read_bytes() for p in system.rglob('*') if p.is_file() and '__pycache__' not in str(p)})

    @staticmethod
    def distribution(root, name, version):
        (root / f'{name}.py').write_text(f'VERSION = "{version}"\n')
        info = root / f'{name}-{version}.dist-info'
        info.mkdir()
        (info / 'METADATA').write_text(f'Metadata-Version: 2.1\nName: {name}\nVersion: {version}\n')
        (info / 'WHEEL').write_text('Wheel-Version: 1.0\nGenerator: fixture\nRoot-Is-Purelib: true\nTag: py3-none-any\n')
        (info / 'RECORD').write_text('')


if __name__ == '__main__':
    unittest.main()
