"""Process-isolated native SDL checks; set VITRALLIS_RENDERER_BIN_DIR after build.

Set VITRALLIS_TEST_ACCELERATED=1 in a graphical session for real GPU readback.
"""
import os
from pathlib import Path
import struct
import subprocess
import tempfile
import unittest


@unittest.skipUnless(os.environ.get('VITRALLIS_RENDERER_BIN_DIR'), 'requires built native binaries')
class NativeRenderer(unittest.TestCase):
    def setUp(self):
        self.scratch = tempfile.TemporaryDirectory(prefix='vitrallis-renderer-')
        self.addCleanup(self.scratch.cleanup)
        self.root = Path(self.scratch.name)
        self.folder = self.root / 'files'
        self.folder.mkdir()
        (self.folder / 'sample.txt').write_text('Native renderer equivalence\n')
        self.binaries = Path(os.environ['VITRALLIS_RENDERER_BIN_DIR']).resolve()

    def run_app(self, app, mode, *args, dummy=True):
        env = dict(os.environ, HOME=str(self.root), XDG_CONFIG_HOME=str(self.root / 'config'),
                   XDG_DATA_HOME=str(self.root / 'data'))
        for key in ('VITRALLIS_SESSION', 'VITRALLIS_NATIVE_BROKER'):
            env.pop(key, None)
        if dummy:
            env['SDL_VIDEODRIVER'] = 'dummy'
        command = [str(self.binaries / ('vitrallis-' + app)), '--renderer', mode, *args]
        if app == 'files':
            command += ['--', str(self.folder)]
        elif app == 'notepad':
            command += ['--', str(self.folder / 'sample.txt')]
        return subprocess.run(command, env=env, cwd=self.root, capture_output=True, timeout=20)

    def test_dummy_policy_and_readback(self):
        for app in ('terminal', 'notepad', 'files'):
            baseline = None
            for mode in ('software', 'auto'):
                with self.subTest(app=app, mode=mode):
                    path = self.root / f'{app}-{mode}.bmp'
                    result = self.run_app(app, mode, '--size', '480x272', '--screenshot', str(path))
                    self.assertEqual(result.returncode, 0, result.stderr.decode())
                    log = result.stderr.decode()
                    self.assertIn(f'requested={mode} mode=software', log)
                    self.assertIn('fallback=' + ('true' if mode == 'auto' else 'false'), log)
                    data = path.read_bytes()
                    self.assertEqual(struct.unpack_from('<ii', data, 18), (480, 272))
                    if baseline is not None:
                        self.assertEqual(data, baseline)
                    baseline = data
                    repeat = self.run_app(app, mode, '--screenshot', str(path))
                    self.assertNotEqual(repeat.returncode, 0)
                    self.assertEqual(path.read_bytes(), data)
            result = self.run_app(app, 'hardware', '--smoke-test')
            self.assertNotEqual(result.returncode, 0)
            self.assertIn(b'hardware renderer unavailable', result.stderr)
            self.assertNotIn(b'event=renderer_initialized', result.stderr)
            for args in (['--renderer'], ['--renderer', 'invalid']):
                result = self.run_app(app, 'auto', *args)
                self.assertNotEqual(result.returncode, 0)

    @unittest.skipUnless(os.environ.get('VITRALLIS_TEST_ACCELERATED') == '1', 'requires accelerated SDL')
    def test_accelerated_visual_equivalence_and_readback(self):
        for app in ('terminal', 'notepad', 'files'):
            for size in ('480x272', '800x480', '1280x720'):
                baseline = None
                for mode in ('software', 'hardware', 'auto'):
                    with self.subTest(app=app, size=size, mode=mode):
                        path = self.root / f'{app}-{size}-{mode}.bmp'
                        result = self.run_app(app, mode, '--size', size, '--screenshot', str(path), dummy=False)
                        self.assertEqual(result.returncode, 0, result.stderr.decode())
                        actual = 'software' if mode == 'software' else 'hardware'
                        self.assertIn(f'requested={mode} mode={actual}', result.stderr.decode())
                        self.assertIn(b'fallback=false', result.stderr)
                        data = path.read_bytes()
                        self.assertEqual(struct.unpack_from('<ii', data, 18), tuple(map(int, size.split('x'))))
                        if baseline is not None:
                            self.assertTrue(data == baseline, 'Native pixels must match exactly')
                        baseline = data


if __name__ == '__main__':
    unittest.main()
