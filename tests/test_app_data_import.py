import errno
import importlib.util
from pathlib import Path
import os
import tempfile
import unittest
from unittest.mock import patch

ROOT = Path(__file__).resolve().parents[1]
SPEC = importlib.util.spec_from_file_location('app_data_import', ROOT / 'scripts/import-app-data.py')
IMPORT = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(IMPORT)


class AppDataImportTests(unittest.TestCase):
    def setUp(self):
        self.temporary = tempfile.TemporaryDirectory()
        self.addCleanup(self.temporary.cleanup)
        self.home = Path(self.temporary.name).resolve()
        self.source = self.home / 'old-data'
        self.source.mkdir(mode=0o700)
        self.saved = self.source / "a saved note ! é.txt"
        self.saved.write_bytes('keep é\n'.encode())
        self.saved.chmod(0o600)

    def plan(self, **kwargs):
        return IMPORT.plan(self.home, 'io.example.app', kwargs.get('files', []),
                           kwargs.get('trees', [(self.source, '.')]))

    def test_preview_then_private_atomic_import_preserves_source(self):
        destination, entries, total = self.plan()
        self.assertFalse(destination.parent.exists())
        old = self.saved.read_bytes()
        # The user's session umask must not make intermediate directories public.
        previous = os.umask(0o002)
        try:
            IMPORT.apply(destination, entries, total)
        finally:
            os.umask(previous)
        self.assertEqual((destination / self.saved.name).read_bytes(), old)
        self.assertEqual(self.saved.read_bytes(), old)
        for path in [destination, *destination.parents]:
            if path == self.home:
                break
            self.assertEqual(path.stat().st_mode & 0o777, 0o700)
        self.assertEqual((destination / self.saved.name).stat().st_mode & 0o777, 0o600)
        with self.assertRaises(ValueError):
            self.plan()

    def test_unsafe_source_and_overlaps_fail_before_destination_creation(self):
        for hazard in ('link', 'hardlink', 'writable', 'collision'):
            with self.subTest(hazard=hazard):
                extra = self.source / 'extra'
                if hazard == 'link':
                    extra.symlink_to(self.saved)
                elif hazard == 'hardlink':
                    os.link(self.saved, extra)
                elif hazard == 'writable':
                    self.saved.chmod(0o666)
                try:
                    with self.assertRaises(ValueError):
                        self.plan(files=[(self.saved, self.saved.name)] if hazard == 'collision' else [])
                    self.assertFalse((self.home / 'Documents').exists())
                finally:
                    if extra.exists() or extra.is_symlink():
                        extra.unlink()
                    self.saved.chmod(0o600)

    def test_partial_copy_enospc_keeps_source_and_no_visible_appdata(self):
        destination, entries, total = self.plan()
        old = self.saved.read_bytes()

        def fail(source, output, before):
            output.write_bytes(b'partial')
            raise OSError(errno.ENOSPC, 'injected full disk')

        with patch.object(IMPORT, 'copy_file', side_effect=fail), self.assertRaises(OSError):
            IMPORT.apply(destination, entries, total)
        self.assertFalse(destination.exists())
        self.assertEqual(list(destination.parent.iterdir()), [])
        self.assertEqual(self.saved.read_bytes(), old)

    def test_low_space_is_rejected_before_directory_creation(self):
        destination, entries, total = self.plan()
        with patch.object(IMPORT.shutil, 'disk_usage', return_value=type('Usage', (), {'free': 0})()), \
                self.assertRaises(OSError):
            IMPORT.apply(destination, entries, total)
        self.assertFalse(destination.parent.exists())

    def test_exclusive_publish_preserves_destination_created_after_preview(self):
        destination, entries, total = self.plan()
        IMPORT.private_directories(destination)
        sentinel = destination / 'existing.txt'
        sentinel.write_bytes(b'other saved data')
        with self.assertRaises(OSError):
            IMPORT.apply(destination, entries, total)
        self.assertEqual(list(destination.iterdir()), [sentinel])
        self.assertEqual(sentinel.read_bytes(), b'other saved data')
        self.assertFalse(list(destination.parent.glob('.import-*')))

    def test_changed_source_fails_without_publishing_partial_data(self):
        destination, entries, total = self.plan()
        self.saved.write_bytes(b'changed after preview')
        with self.assertRaises(ValueError):
            IMPORT.apply(destination, entries, total)
        self.assertFalse(destination.exists())
        self.assertEqual(self.saved.read_bytes(), b'changed after preview')

    def test_post_commit_sync_failure_reports_present_data(self):
        destination, entries, total = self.plan()
        original = os.fsync

        def fail_after_publish(descriptor):
            if destination.exists():
                raise OSError(errno.EIO, 'injected storage sync failure')
            return original(descriptor)

        with patch.object(IMPORT.os, 'fsync', side_effect=fail_after_publish), \
                self.assertRaisesRegex(OSError, 'Imported data is present'):
            IMPORT.apply(destination, entries, total)
        self.assertEqual((destination / self.saved.name).read_bytes(), self.saved.read_bytes())


if __name__ == '__main__':
    unittest.main()
