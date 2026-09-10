import hashlib
import importlib.util
import json
from pathlib import Path
import tempfile
import unittest
from unittest.mock import patch

spec = importlib.util.spec_from_file_location('store_patch', Path(__file__).parents[1] / 'scripts/apply-store-patch.py')
m = importlib.util.module_from_spec(spec)
spec.loader.exec_module(m)


class StorePatch(unittest.TestCase):
    def setUp(self):
        self.temp = tempfile.TemporaryDirectory()
        self.addCleanup(self.temp.cleanup)
        self.root = Path(self.temp.name).resolve()
        self.source, self.target = self.root / 'source', self.root / 'target'
        self.source.mkdir()
        self.target.mkdir()
        self.names = ['update_apps.py', 'deployment.py', 'test_deployment.py', 'test_update_apps.py', 'test_self_update.py']
        self.old, self.new = b'# previous\n', b'# reviewed\n'
        self.manifest = self.root / 'manifest.json'
        self.manifest.write_text(json.dumps({'files': {name: dict(before=hashlib.sha256(self.old).hexdigest(), after=hashlib.sha256(self.new).hexdigest()) for name in self.names}}))
        for name in self.names:
            (self.source / name).write_bytes(self.new)
            (self.target / name).write_bytes(self.old)

    def apply(self):
        m.apply(self.source, self.manifest, self.target)

    def test_exact_bundle_and_idempotence(self):
        self.apply()
        self.apply()
        self.assertEqual(len(list(self.target.glob('before-vitrallis-*'))), 1)
        self.assertFalse((self.target / '.installation-pending').exists())
        for name in self.names:
            self.assertEqual((self.target / name).read_bytes(), self.new)

    def test_unknown_edit_and_source_tampering_rejected(self):
        (self.target / 'deployment.py').write_bytes(b'# user edit')
        with self.assertRaisesRegex(ValueError, 'differs'):
            self.apply()
        self.assertEqual((self.target / 'update_apps.py').read_bytes(), self.old)
        (self.target / 'deployment.py').write_bytes(self.old)
        (self.source / 'deployment.py').write_bytes(b'# unreviewed')
        with self.assertRaisesRegex(ValueError, 'match manifest'):
            self.apply()

    def test_failure_rolls_back_and_retry_repairs_marker(self):
        real = m.atomic
        failed = []
        def fail(path, data):
            if path == self.target / 'test_deployment.py' and not failed:
                failed.append(True)
                raise OSError('read-only filesystem')
            real(path, data)
        with patch.object(m, 'atomic', side_effect=fail):
            with self.assertRaises(OSError):
                self.apply()
        self.assertTrue((self.target / '.installation-pending').exists())
        for name in self.names:
            self.assertEqual((self.target / name).read_bytes(), self.old)
        self.apply()
        self.assertFalse((self.target / '.installation-pending').exists())

    def test_symlink_never_mutates_target(self):
        outside = self.root / 'outside'
        outside.write_bytes(self.old)
        path = self.target / 'update_apps.py'
        path.unlink()
        path.symlink_to(outside)
        with self.assertRaises(ValueError):
            self.apply()
        self.assertEqual(outside.read_bytes(), self.old)


if __name__ == '__main__':
    unittest.main()
