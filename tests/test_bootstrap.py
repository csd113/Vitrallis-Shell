"""Bounded release selection and literal device-guide command tests with local fixtures."""
import copy
import hashlib
import importlib.machinery
import importlib.util
import json
import os
from pathlib import Path
import re
import shlex
import shutil
import subprocess
import sys
import tempfile
import unittest
from unittest.mock import patch

import test_installer as fixture
from bootstrap_fixture import ShellFixture

ROOT = fixture.ROOT
DEVICE = fixture.DEVICE


def load(name, path):
    loader = importlib.machinery.SourceFileLoader(name, str(path))
    spec = importlib.util.spec_from_loader(name, loader)
    module = importlib.util.module_from_spec(spec)
    loader.exec_module(module)
    return module


b = load('bootstrap', DEVICE / 'bootstrap.py')


def setUpModule():
    # See test_installer: fixtures must not inherit the caller's umask.
    global _module_umask
    _module_umask = os.umask(0o022)


def tearDownModule():
    os.umask(_module_umask)


def release(tag='v1.2.3-beta.2', payload=b'fixture'):
    data = {b.BUNDLE: payload}
    data.update({name: (DEVICE / name).read_bytes() for name in b.HELPERS})
    # Published releases also carry the project license and third-party
    # notices; the installer must download only its own bundle and helpers.
    for name in ('LICENSE', 'THIRD_PARTY_NOTICES.md', 'THIRD_PARTY_LICENSES.txt'):
        data[name] = b'fixture notice ' + name.encode()
    for name, content in list(data.items()):
        data[name + '.sha256'] = (hashlib.sha256(content).hexdigest() + '  ' + name + '\n').encode()
    assets = [{'name': name, 'size': len(content), 'state': 'uploaded',
               'browser_download_url': b.DOWNLOAD + tag + '/' + name}
              for name, content in data.items()]
    return {'tag_name': tag, 'prerelease': '-' in tag, 'draft': False,
            'published_at': '2026-09-12T00:00:00Z', 'assets': assets}, data


class Bootstrap(unittest.TestCase):
    def setUp(self):
        self.temp = tempfile.TemporaryDirectory()
        self.addCleanup(self.temp.cleanup)
        self.root = Path(self.temp.name).resolve()
        self.value, self.data = release()

    def fetch(self, url, path, limit, timeout=30):
        content = self.data[url.rsplit('/', 1)[-1]]
        if len(content) > limit:
            raise ValueError('oversized fixture')
        path.write_bytes(content)

    def download(self):
        with patch.object(b, 'fetch', side_effect=self.fetch):
            return b.download_release(self.value, self.root)

    def test_same_release_helpers_and_complete_checksums(self):
        result = self.download()
        self.assertEqual(result.read_bytes(), self.data[b.BUNDLE])
        for name in b.HELPERS:
            self.assertEqual((self.root / name).read_bytes(), (DEVICE / name).read_bytes())
        for name in ('LICENSE', 'THIRD_PARTY_NOTICES.md', 'THIRD_PARTY_LICENSES.txt'):
            self.assertFalse((self.root / name).exists())

    def test_semver_beta_stable_drafts_and_obsolete_releases(self):
        values = [release(v)[0] for v in ('v1.2.3-beta.10', 'v1.2.3-beta.2', 'v1.2.3', 'v2.0.0-beta.1')]
        self.assertEqual(b.select(values)['tag_name'], 'v2.0.0-beta.1')
        self.assertEqual(b.select(values, stable=True)['tag_name'], 'v1.2.3')
        self.assertEqual(b.select(values[:2])['tag_name'], 'v1.2.3-beta.10')
        values[-1]['draft'] = True
        self.assertEqual(b.select(values)['tag_name'], 'v1.2.3')
        self.assertEqual(b.select(values, tag='v1.2.3-beta.2')['tag_name'], 'v1.2.3-beta.2')
        values[0]['assets'] = [{'name': b.BUNDLE[:-len('.vtrbundle')]}]
        with self.assertRaisesRegex(ValueError, 'No compatible'):
            b.select(values[:1])

    def test_bundle_with_obsolete_installer_is_not_a_supported_session_release(self):
        value, _ = release()
        for asset in value['assets']:
            asset['name'] = asset['name'].replace('install-session.py', 'install.py')
        with self.assertRaisesRegex(ValueError, 'No compatible'):
            b.select([value])

    def test_missing_duplicate_foreign_and_oversized_assets_fail_before_download(self):
        original = copy.deepcopy(self.value)

        def drop_bundle_sidecar(value):
            value['assets'] = [asset for asset in value['assets']
                               if asset['name'] != b.BUNDLE + '.sha256']

        def drop_bundle_asset(value):
            value['assets'] = [asset for asset in value['assets']
                               if asset['name'] != b.BUNDLE]

        cases = [drop_bundle_asset, drop_bundle_sidecar,
                 lambda v: v['assets'].append(v['assets'][0]),
                 lambda v: v['assets'][0].update(browser_download_url='https://example.com/evil'),
                 lambda v: v['assets'][0].update(size=b.MAX_BUNDLE + 1),
                 lambda v: v['assets'][1].update(state='new')]
        for mutate in cases:
            self.value = copy.deepcopy(original)
            mutate(self.value)
            with patch.object(b, 'fetch') as fetch, self.assertRaises(ValueError):
                b.download_release(self.value, self.root)
            fetch.assert_not_called()

    def test_corrupt_missing_checksum_and_disagreeing_digest(self):
        self.data[b.BUNDLE] = b'corrupt'
        with self.assertRaisesRegex(ValueError, 'SHA-256'):
            self.download()
        self.value, self.data = release()
        self.value['assets'][0]['digest'] = 'sha256:' + '0' * 64
        with self.assertRaisesRegex(ValueError, 'disagrees'):
            self.download()
        self.value, self.data = release()
        self.data[b.BUNDLE + '.sha256'] = b'x' * len(self.data[b.BUNDLE + '.sha256'])
        with self.assertRaisesRegex(ValueError, 'sidecar'):
            self.download()

    def test_truncated_response_and_transport_failure(self):
        self.data[b.BUNDLE] = b'tiny'
        with self.assertRaisesRegex(ValueError, 'length'):
            self.download()
        with patch.object(b, 'fetch', side_effect=OSError('network failure')), self.assertRaises(OSError):
            b.download_release(self.value, self.root)

    def test_listing_is_bounded_and_rejects_objects(self):
        with patch.object(b, 'fetch', side_effect=lambda url, path, limit: path.write_text('[{}]' .replace('{}', ','.join(['{}'] * 30)))) as fetch:
            with self.assertRaisesRegex(ValueError, '150'):
                b.releases(self.root)
            self.assertEqual(fetch.call_count, 5)
        with patch.object(b, 'fetch', side_effect=lambda url, path, limit: path.write_text('{}')):
            with self.assertRaisesRegex(ValueError, 'listing'):
                b.releases(self.root)

    def test_transport_disables_curl_config_and_limits_bytes_redirects_and_time(self):
        def run(args, **kwargs):
            self.assertEqual(args[:2], ['/usr/bin/curl', '-q'])
            self.assertEqual(args[args.index('--proto') + 1], '=https')
            self.assertEqual(args[args.index('--proto-redir') + 1], '=https')
            self.assertIn('--max-filesize', args)
            self.assertEqual(kwargs['timeout'], 35)
            self.assertIsNotNone(kwargs['preexec_fn'])
            kwargs['stdout'].write(b'complete')
            return subprocess.CompletedProcess(args, 0)
        with patch.object(b.subprocess, 'run', side_effect=run):
            b.fetch('https://github.com/example', self.root / 'file', 100)
        with self.assertRaisesRegex(ValueError, 'HTTPS'):
            b.fetch('http://github.com/example', self.root / 'bad', 100)

    def test_download_permissions_are_accepted_by_installer_with_shared_umask(self):
        def run(args, **kwargs):
            kwargs['stdout'].write(b'verified download')
            return subprocess.CompletedProcess(args, 0)
        for mask in (0o002, 0o000, 0o077):
            with self.subTest(umask=oct(mask)):
                path = self.root / ('download-' + str(mask))
                previous = os.umask(mask)
                try:
                    with patch.object(b.subprocess, 'run', side_effect=run):
                        b.fetch('https://github.com/example', path, 100)
                finally:
                    os.umask(previous)
                self.assertEqual(path.stat().st_mode & 0o777, 0o600)
                self.assertEqual(fixture.m.read_file(path), b'verified download')
                with patch.object(b.subprocess, 'run') as transport:
                    with self.assertRaises(FileExistsError):
                        b.fetch('https://github.com/example', path, 100)
                    transport.assert_not_called()
                self.assertEqual(path.read_bytes(), b'verified download')


class ReadmeCommands(unittest.TestCase):
    def setUp(self):
        self.fixture = fixture.Installer()
        self.fixture.setUp()
        self.addCleanup(self.fixture.doCleanups)
        self.home = self.fixture.home
        shell_temp = tempfile.TemporaryDirectory(prefix='vtr-', dir='/tmp')
        self.addCleanup(shell_temp.cleanup)
        self.shell = ShellFixture(Path(shell_temp.name), (DEVICE / 'bootstrap.sh').read_text())
        self.addCleanup(self.shell.bus.close)
        self.shell.marker.touch()
        self.commands = self.shell.commands
        self.network = self.home / 'network'
        self.network.mkdir()
        value, data = release(payload=self.fixture.binary.read_bytes())
        (self.network / 'releases.json').write_text(json.dumps([value]))
        for name, content in data.items():
            (self.network / name).write_bytes(content)
        self.env = dict(self.shell.env, HOME=str(self.home),
                        PATH=str(self.commands) + os.pathsep + os.environ['PATH'],
                        VITRALLIS_TEST_NETWORK=str(self.network), VITRALLIS_TEST_LOG=str(self.home / 'outer-download'))
        self.write_command('curl', '''#!/bin/sh
while [ "$#" -gt 0 ]; do
    if [ "$1" = -o ]; then shift; output=$1; fi
    shift
done
printf '%s' "$output" > "$VITRALLIS_TEST_LOG"
if [ "${VITRALLIS_TEST_FAIL-}" = yes ]; then
    printf 'raise SystemExit("partial payload must never execute")' > "$output"
    exit 22
fi
cp ''' + shlex.quote(str(DEVICE / 'bootstrap.py')) + ''' "$output"
''')
        self.write_command('python3', '#!/bin/sh\nif [ "$2" = -c ]; then exit 0; fi\nexec ' + shlex.quote(sys.executable) + ' ' +
                           shlex.quote(str(Path(__file__).resolve())) + ' --readme-driver "$@"\n')
        readme = (ROOT / 'docs/devices/pocketchip.md').read_text()
        block = re.search(r'<!-- pocketchip-bootstrap -->\n```sh\n(.*?)\n```', readme, re.S).group(1)
        self.assertEqual(block, (DEVICE / 'bootstrap.sh').read_text().split('\n', 2)[2].rstrip())
        self.install_line = self.shell.script
        self.uninstall_line = re.search(r'## Uninstall\n.*?```sh\n([^\n]+)\n```', (ROOT / 'README.md').read_text(), re.S).group(1)

    def write_command(self, name, data):
        path = self.commands / name
        path.write_text(data)
        path.chmod(0o755)

    def run_line(self, line):
        return subprocess.run(['sh', '-c', line], cwd=self.home, env=self.env,
                              capture_output=True, text=True, check=False)

    def test_literal_readme_install_reinstall_dry_run_uninstall(self):
        for _ in range(2):
            result = self.run_line(self.install_line)
            self.assertEqual(result.returncode, 0, result.stderr)
            self.assertFalse(Path((self.home / 'outer-download').read_text()).exists())
            for name in fixture.m.BINARIES:
                self.assertTrue((self.fixture.target / 'current' / name).is_file())
        result = self.run_line(self.uninstall_line + ' --dry-run')
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertTrue((self.fixture.target / 'current').is_symlink())
        self.assertFalse((self.fixture.target / '__pycache__').exists())
        result = self.run_line(self.uninstall_line)
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertFalse((self.fixture.target / 'current').exists())
        self.assertFalse((self.fixture.target / 'uninstall.py').exists())
        again = self.run_line(self.uninstall_line)
        self.assertNotEqual(again.returncode, 0)

    def test_failed_literal_download_cleans_temp_and_never_executes_python(self):
        self.env['VITRALLIS_TEST_FAIL'] = 'yes'
        self.write_command('python3', '#!/bin/sh\nif [ "$2" = -c ]; then exit 0; fi\ntouch "$HOME/incorrectly-executed"\n')
        result = self.run_line(self.install_line)
        self.assertEqual(result.returncode, 22)
        self.assertFalse((self.home / 'incorrectly-executed').exists())
        self.assertFalse(Path((self.home / 'outer-download').read_text()).exists())
        self.assertFalse(self.fixture.target.exists())

    def test_corrupt_release_never_installs_and_cleans_bootstrap(self):
        path = self.network / b.BUNDLE
        data = path.read_bytes()
        path.write_bytes(data[:-1] + bytes([data[-1] ^ 1]))
        result = self.run_line(self.install_line)
        self.assertNotEqual(result.returncode, 0)
        self.assertIn('SHA-256', result.stderr)
        self.assertFalse(self.fixture.target.exists())
        self.assertFalse(Path((self.home / 'outer-download').read_text()).exists())


def readme_driver():
    # This driver changes only transport, runtime probes and session boundaries.
    # Production scripts have no fixture-mode flags or environment bypasses.
    sys.argv = sys.argv[2:]
    if sys.argv[0] == '-I':
        sys.argv = sys.argv[1:]
    script = Path(sys.argv[0])
    sys.dont_write_bytecode = True
    module = load('readme_script', script)
    if script.name == 'uninstall.py':
        with patch.object(os, 'geteuid', return_value=1000), patch.object(module, 'session', return_value=False):
            module.main()
        return
    network = Path(os.environ['VITRALLIS_TEST_NETWORK'])
    def fetch(url, path, limit, timeout=30):
        source = network / ('releases.json' if url.startswith(b.API) else url.rsplit('/', 1)[-1])
        content = source.read_bytes()
        if len(content) > limit:
            raise ValueError('oversized mock response')
        path.write_bytes(content)
    def install(args, **kwargs):
        if args[1] != '-I':
            raise AssertionError('installer must use isolated Python')
        args = [args[0]] + args[2:]
        installer = load('readme_installer', Path(args[1]))
        with patch.object(installer, 'preflight'), patch.object(installer, 'require_stopped_session'), patch.object(installer, 'verify_versions') as probe:
            installer.install(Path(args[2]), Path(args[1]).parent, Path.home(), args[4])
            self_expected = probe.call_args.args[1]
            if self_expected != '1.2.3-beta.2':
                raise AssertionError('release tag was not passed to the installer')
        return subprocess.CompletedProcess(args, 0)
    with patch.object(os, 'geteuid', return_value=1000), patch.object(module, 'fetch', side_effect=fetch), \
            patch.object(module, 'installation_environment', return_value=dict(os.environ)), \
            patch.object(module, 'desktop_available', return_value=False), \
            patch.object(module.subprocess, 'run', side_effect=install):
        module.main()


if __name__ == '__main__':
    if len(sys.argv) > 1 and sys.argv[1] == '--readme-driver':
        readme_driver()
    else:
        unittest.main()
