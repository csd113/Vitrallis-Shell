"""Device-tree overlay and platform setup regressions; no privileged host writes."""
import importlib.util
import os
from pathlib import Path
import shutil
import subprocess
import tempfile
import unittest
from unittest.mock import patch

ROOT = Path(__file__).resolve().parents[1]
spec = importlib.util.spec_from_file_location('gpu_setup', ROOT / 'integrations/pocketchip/platform-setup.py')
m = importlib.util.module_from_spec(spec)
spec.loader.exec_module(m)
DTS = b'''/dts-v1/;
/memreserve/ 0x10000000 0x1000;
/ {
 compatible = "nextthing,chip", "allwinner,sun5i-r8";
 display { brightness = <7>; calibration = "keep-touch-data"; };
 soc { gpu@1c40000 { compatible = "allwinner,sun4i-a10-mali", "arm,mali-400";
 clocks = <2 49 2 98>; assigned-clock-rates = <320000000>; }; };
};
'''


class Trees(unittest.TestCase):
    def test_malformed_tree_fails_closed(self):
        for data in (b'', b'\0' * 100, b'\xd0\x0d\xfe\xed' + b'\xff' * 96):
            with self.assertRaises(ValueError): m.properties(data)

    @unittest.skipUnless(shutil.which('dtc') and shutil.which('fdtoverlay'), 'requires device-tree-compiler')
    def test_overlay_preserves_board_and_is_byte_idempotent(self):
        def run(args):
            return subprocess.run([shutil.which(Path(args[0]).name)] + args[1:], check=True,
                                  stdout=subprocess.PIPE, stderr=subprocess.PIPE).stdout
        base = subprocess.run([shutil.which('dtc'), '-I', 'dts', '-O', 'dtb'], input=DTS,
                              check=True, stdout=subprocess.PIPE, stderr=subprocess.PIPE).stdout
        with patch.object(m, 'run', run):
            result = m.patched(base)
            self.assertEqual(m.patched(result), result)
        before, after = m.properties(base), m.properties(result)
        self.assertTrue(m.has_opp(after))
        for path, values in before.items():
            for key, value in values.items(): self.assertEqual(after[path][key], value)
        self.assertEqual(after['/opp-table-gpu/opp-297000000']['opp-hz'], (297000000).to_bytes(8, 'big'))
        self.assertNotEqual(base, result)
        self.assertEqual(m.reservations(base), m.reservations(result))

    def test_existing_invalid_opp_is_not_silently_replaced(self):
        for reference in (b'', b'abc', b'\0\0\0\x01'):
            with self.assertRaises(ValueError): m.has_opp({m.GPU: {'operating-points-v2': reference}})

    def test_kernel_path_cannot_escape_expected_tree(self):
        for version in ('../other', '/etc/passwd', '', 'bad version', 'x\ny', 'x' * 200):
            with self.assertRaises(ValueError): m.dtb_path(version)
        self.assertEqual(str(m.dtb_path('6.12.107+deb13-chip', boot=True)),
                         '/boot/dtbs/6.12.107+deb13-chip/sun5i-r8-chip.dtb')

    def test_validation_precedes_all_mutation(self):
        with tempfile.TemporaryDirectory() as temp:
            first, second = Path(temp) / 'first', Path(temp) / 'second'
            first.write_bytes(b'good'); second.write_bytes(b'bad')
            def transform(value):
                if value == b'bad': raise ValueError('bad tree')
                return b'changed'
            with patch.object(m, 'safe'), patch.object(m, 'patched', transform), patch.object(m, 'atomic') as write:
                with self.assertRaises(ValueError): m.update_dtbs([first, second])
                write.assert_not_called()

    def test_write_failure_restores_prior_dtb(self):
        with tempfile.TemporaryDirectory() as temp:
            root = Path(temp)
            first, second = root / 'first', root / 'second'
            first.write_bytes(b'one'); second.write_bytes(b'two')
            def write(path, data, mode=0o644):
                if path == second and data == b'new-two': raise OSError('disk full')
                path.parent.mkdir(parents=True, exist_ok=True)
                path.write_bytes(data)
            with patch.object(m, 'safe'), patch.object(m, 'patched', lambda data: b'new-' + data), \
                    patch.object(m, 'atomic', write), patch.object(m, 'STATE', root / 'state'):
                with self.assertRaises(OSError): m.update_dtbs([first, second])
            self.assertEqual(first.read_bytes(), b'one')
            self.assertEqual(second.read_bytes(), b'two')

    def test_configure_trace_never_exposes_global_trace_or_writable_controls(self):
        with tempfile.TemporaryDirectory() as temp:
            root = Path(temp)
            trace = root / 'tracing'
            instance = trace / 'instances/vitrallis-gpu'
            event = instance / 'events/devfreq/devfreq_monitor'
            event.mkdir(parents=True)
            calls = []
            with patch.object(m, 'TRACE', trace), patch.object(m, 'INSTANCE', instance), \
                    patch.object(m, 'READER', root / 'run/trace_pipe'), patch.object(m, 'safe'), \
                    patch.object(m, 'mount_info', lambda p: ('/', 'tracefs', ['rw']) if p == trace else None), \
                    patch.object(m, 'atomic', lambda p,d,mode: p.write_bytes(d)), \
                    patch.object(m, 'grant_read', lambda *a, **kw: calls.append((a, kw))), patch.object(m, 'run'):
                m.configure_trace(1000)
                m.configure_trace(1000)
            self.assertEqual(calls, [((instance / 'trace_pipe', 1000), {}),
                                     ((instance / 'trace_pipe', 1000), {})])
            self.assertEqual((event / 'filter').read_text(), 'dev_name == "1c40000.gpu"')
            self.assertFalse((trace / 'tracing_on').exists())
            self.assertEqual((instance / 'trace_clock').read_text(), 'mono')


class Recovery(unittest.TestCase):
    def test_interrupted_dtb_recovers_only_if_new_digest_still_matches(self):
        import hashlib
        import json
        with tempfile.TemporaryDirectory() as temp:
            state = Path(temp)
            original, updated = b'original-tree', b'updated-tree'
            before, after = (hashlib.sha256(value).hexdigest() for value in (original, updated))
            backup = state / 'dtb-backups' / (before + '.dtb')
            backup.parent.mkdir()
            backup.write_bytes(original)
            journal = state / 'dtb-transaction.json'
            destination = m.dtb_path('6.12.107+deb13-chip', boot=True)
            source = m.dtb_path('6.12.107+deb13-chip')
            record = [{'path': str(destination), 'before': before, 'after': after},
                      {'path': str(source), 'before': before, 'after': after}]
            read_bytes = Path.read_bytes
            for current, succeeds in ((b'user-edit', False), (updated, True)):
                journal.write_text(json.dumps(record))
                def read(path):
                    if path == destination: return current
                    if path == source: return original
                    return read_bytes(path)
                with patch.object(m, 'STATE', state), patch.object(m, 'safe'), \
                        patch.object(Path, 'read_bytes', read), patch.object(m, 'atomic') as write:
                    if succeeds:
                        m.recover_dtbs()
                        write.assert_called_once_with(destination, original)
                        self.assertFalse(journal.exists())
                    else:
                        with self.assertRaisesRegex(ValueError, 'edited'): m.recover_dtbs()
                        write.assert_not_called()
                        self.assertTrue(journal.exists())

    def test_boot_selection_requires_valid_crc_and_exact_supported_path(self):
        import struct
        import zlib
        script = b'ubifsload 0x43000000 /boot/dtbs/6.12.107+deb13-chip/sun5i-r8-chip.dtb\nbootz 0x42000000 - 0x43000000\n'
        body = struct.pack('>II', len(script), 0) + script
        header = bytearray(64)
        struct.pack_into('>I', header, 0, 0x27051956)
        struct.pack_into('>I', header, 12, len(body))
        struct.pack_into('>I', header, 24, zlib.crc32(body))
        struct.pack_into('>I', header, 4, zlib.crc32(header))
        with patch.object(m, 'safe'), patch.object(m, 'read_file', return_value=bytes(header) + body):
            self.assertEqual(m.active_boot_version(), '6.12.107+deb13-chip')
        with patch.object(m, 'safe'), patch.object(m, 'read_file', return_value=bytes(header) + body + b'changed'):
            with self.assertRaisesRegex(ValueError, 'checksum/length'): m.active_boot_version()

    def test_unrelated_mount_at_reader_path_is_not_modified(self):
        with patch.object(m, 'mount_info', return_value=('/', 'tmpfs', ['rw'])):
            with self.assertRaisesRegex(ValueError, 'Unmanaged mount'): m.reader_mounted()
