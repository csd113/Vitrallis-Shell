"""Tor service tests use a local fake Arti; no Tor network is contacted."""
import importlib.util
import json
import os
from pathlib import Path
import selectors
import signal
import socket
import subprocess
import sys
import tempfile
import time
import unittest
from unittest.mock import patch

ROOT = Path(__file__).resolve().parents[1]
SPEC = importlib.util.spec_from_file_location('tor_guardian', ROOT / 'src/tor/supervisor.py')
TOR = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(TOR)

FAKE = '''#!/usr/bin/env python3
import os, signal, socket, sys, time
if sys.argv[1:] == ['--version']:
    print('Arti 2.6.0'); sys.exit()
open(os.path.join(os.path.dirname(sys.argv[-1]), 'fake.pid'), 'w').write(str(os.getpid()))
s = socket.socket(); s.setsockopt(socket.SOL_SOCKET, socket.SO_REUSEADDR, 1); s.bind(('127.0.0.1', 9150)); s.listen(16)
print('Bootstrapping: 45%', flush=True)
print('Sufficiently bootstrapped; proxy now functional.', flush=True)
while True:
    c, _ = s.accept()
    with c:
        if c.recv(3) == b'\\x05\\x01\\x00': c.sendall(b'\\x05\\x00')
'''


class TorGuardian(unittest.TestCase):
    def setUp(self):
        self.temp = tempfile.TemporaryDirectory()
        self.addCleanup(self.temp.cleanup)
        self.root = Path(self.temp.name).resolve()
        self.fake = self.root / 'arti'
        self.fake.write_text(FAKE)
        self.fake.chmod(0o700)
        self.processes = []
        self.addCleanup(self.stop_all)

    def stop_all(self):
        for child in self.processes:
            if child.stdin and not child.stdin.closed:
                child.stdin.close()
            try:
                child.wait(timeout=5)
            except subprocess.TimeoutExpired:
                child.kill(); child.wait()
            if child.stdout: child.stdout.close()

    def start(self, root=None):
        child = subprocess.Popen([sys.executable, '-I', '-u', str(ROOT / 'src/tor/supervisor.py'),
                                  str(root or self.root / 'service'), str(self.fake)],
                                 stdin=subprocess.PIPE, stdout=subprocess.PIPE, stderr=subprocess.PIPE)
        self.processes.append(child)
        self.addCleanup(child.stderr.close)
        return child

    def wait_state(self, child, expected):
        deadline = time.monotonic() + 8
        with selectors.DefaultSelector() as selector:
            selector.register(child.stdout, selectors.EVENT_READ)
            pending = b''
            while time.monotonic() < deadline:
                if not selector.select(0.2): continue
                data = os.read(child.stdout.fileno(), 8192)
                if not data: break
                pending += data
                while b'\n' in pending:
                    line, pending = pending.split(b'\n', 1)
                    value = json.loads(line)
                    if value['state'] == expected: return value
        self.fail('Did not reach ' + expected + '; process=' + str(child.poll()))

    def test_progress_uses_arti_readiness_not_pid_or_socks_alone(self):
        self.assertEqual(TOR.progress(b'INFO Sufficiently bootstrapped; proxy now functional.'), 100)
        self.assertEqual(TOR.progress(b'Bootstrapping: 45%'), 45)
        self.assertIsNone(TOR.progress(b'Listening on 127.0.0.1:9150'))
        self.assertIsNone(TOR.progress(b'private destination or credential'))

    def test_shared_lifecycle_duplicate_prevention_crash_and_restart(self):
        child = self.start()
        self.assertEqual(self.wait_state(child, 'connected')['progress'], 100)
        root = self.root / 'service'
        pid = int((root / 'fake.pid').read_text())
        duplicate = self.start()
        self.assertIn('Another Shell', self.wait_state(duplicate, 'error')['diagnostic'])
        duplicate.wait(timeout=5)
        os.kill(pid, signal.SIGKILL)
        self.assertIn('exited', self.wait_state(child, 'error')['diagnostic'])
        child.wait(timeout=5)
        self.assertFalse((root / 'proxy.sock').exists())
        for _ in range(3):
            child = self.start()
            self.wait_state(child, 'connected')
            pid = int((root / 'fake.pid').read_text())
            child.stdin.close()
            self.assertEqual(child.wait(timeout=5), 0)
            with self.assertRaises(ProcessLookupError): os.kill(pid, 0)
        self.assertEqual(root.stat().st_mode & 0o777, 0o700)
        self.assertEqual((root / 'arti.toml').stat().st_mode & 0o777, 0o600)
        self.assertNotIn('0.0.0.0', (root / 'arti.toml').read_text())

    def test_occupied_endpoint_is_not_adopted_or_killed(self):
        with socket.socket() as listener:
            listener.setsockopt(socket.SOL_SOCKET, socket.SO_REUSEADDR, 1); listener.bind(('127.0.0.1', 9150)); listener.listen()
            child = self.start()
            self.wait_state(child, 'error')
            self.assertNotEqual(child.wait(timeout=5), 0)
            self.assertFalse((self.root / 'service/fake.pid').exists())
            self.assertEqual(listener.getsockname(), ('127.0.0.1', 9150))

    def test_missing_or_wrong_arti_binary_fails_closed(self):
        with patch.object(TOR.sys, 'argv', ['guardian', 'root', '/missing/arti']), patch.object(TOR.os.path, 'isfile', return_value=False):
            with self.assertRaisesRegex(ValueError, 'missing'): TOR.binary()
        with patch.object(TOR.sys, 'argv', ['guardian', 'root', str(self.fake)]), patch.object(TOR.os.path, 'isfile', return_value=True), patch.object(TOR.subprocess, 'run', return_value=subprocess.CompletedProcess([], 0, b'Arti 1.0.0')):
            with self.assertRaisesRegex(ValueError, 'missing'): TOR.binary()

    def test_symlink_runtime_and_config_are_rejected(self):
        target = self.root / 'target'; target.mkdir()
        link = self.root / 'link'; link.symlink_to(target)
        with self.assertRaises(ValueError): TOR.private(link)
        config = target / 'arti.toml'; config.symlink_to(self.fake)
        original = self.fake.read_bytes()
        with self.assertRaises(ValueError): TOR.atomic(config, b'bad')
        self.assertEqual(self.fake.read_bytes(), original)

    @unittest.skipUnless(sys.platform == 'linux', 'Linux parent-death contract')
    def test_guardian_crash_reaps_arti_and_releases_lock(self):
        child = self.start(); self.wait_state(child, 'connected')
        pid = int((self.root / 'service/fake.pid').read_text())
        child.kill(); child.wait(timeout=5)
        deadline = time.monotonic() + 5
        while time.monotonic() < deadline:
            path = Path('/proc') / str(pid) / 'stat'
            try:
                if path.read_text().split()[2] == 'Z': break
            except (FileNotFoundError, ProcessLookupError):
                break
            time.sleep(0.05)
        else: self.fail('Orphan Arti remained running')
        restarted = self.start(); self.wait_state(restarted, 'connected')


class RequiredIsolation(TorGuardian):
    @unittest.skipUnless(sys.platform == 'linux' and Path('/usr/bin/bwrap').exists(), 'Linux bubblewrap required')
    def test_required_app_can_use_shared_socks_but_cannot_use_clearnet_or_dns(self):
        child = self.start(); self.wait_state(child, 'connected')
        sandbox = (ROOT / 'src/tor/sandbox.py').read_text()
        guardian = (ROOT / 'src/tor/supervisor.py').read_text()
        program = '''import socket
assert socket.if_nameindex() == [(1, 'lo')]
with socket.create_connection(('127.0.0.1', 9150), timeout=2) as proxy:
    proxy.sendall(b'\\x05\\x01\\x00')
    assert proxy.recv(2) == b'\\x05\\x00'
for family, address in [(socket.AF_INET, ('192.0.2.1', 443)), (socket.AF_INET6, ('2001:db8::1', 443))]:
    with socket.socket(family, socket.SOCK_STREAM) as direct:
        direct.settimeout(.2)
        assert direct.connect_ex(address) != 0
with socket.socket(socket.AF_INET, socket.SOCK_DGRAM) as dns:
    try: dns.sendto(b'query', ('192.0.2.1', 53))
    except OSError: pass
    else: raise AssertionError('Direct DNS escaped')
print('isolated Tor proxy verified')
'''
        args = ['/usr/bin/bwrap', '--die-with-parent', '--unshare-user', '--unshare-pid', '--unshare-net',
                '--cap-drop', 'ALL', '--bind', '/', '/', '--proc', '/proc', '--dev', '/dev', '--',
                '/usr/bin/python3', '-I', '-c', sandbox, str(self.root / 'service/proxy.sock'), guardian,
                '/usr/bin/python3', '-I', '-c', program]
        result = subprocess.run(args, capture_output=True, text=True, timeout=10)
        if 'Operation not permitted' in result.stderr:
            self.skipTest('Container kernel denies user/network namespaces; test on supported device')
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertIn('isolated Tor', result.stdout)
        child.stdin.close(); child.wait(timeout=5)
        result = subprocess.run(args, capture_output=True, text=True, timeout=10)
        self.assertNotEqual(result.returncode, 0)
        self.assertNotIn('isolated Tor', result.stdout)


@unittest.skipUnless(os.environ.get('VITRALLIS_TEST_ARTI'), 'Opt-in real Tor network test')
class RealArti(unittest.TestCase):
    def test_real_bootstrap_and_clean_stop(self):
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary).resolve()
            child = subprocess.Popen([sys.executable, '-I', '-u', str(ROOT / 'src/tor/supervisor.py'),
                                      str(root / 'tor'), str(Path(os.environ['VITRALLIS_TEST_ARTI']).resolve())],
                                     stdin=subprocess.PIPE, stdout=subprocess.PIPE, stderr=subprocess.PIPE)
            try:
                deadline, connected = time.monotonic() + 190, False
                with selectors.DefaultSelector() as selector:
                    selector.register(child.stdout, selectors.EVENT_READ)
                    while time.monotonic() < deadline and child.poll() is None:
                        if not selector.select(1): continue
                        record = json.loads(child.stdout.readline())
                        if record['state'] == 'connected': connected = True; break
                        if record['state'] == 'error': self.fail(record['diagnostic'])
                self.assertTrue(connected, 'Tor did not bootstrap')
            finally:
                child.stdin.close()
                child.wait(timeout=6)
                child.stdout.close(); child.stderr.close()

class RelayIntegrity(unittest.TestCase):
    def test_half_close_drains_backpressure_without_truncation(self):
        import threading
        with tempfile.TemporaryDirectory() as temporary:
            path = str(Path(temporary) / 'upstream')
            upstream = socket.socket(socket.AF_UNIX)
            upstream.bind(path); upstream.listen()
            listener = socket.socket(); listener.bind(('127.0.0.1', 0)); listener.listen()
            errors = []
            data = bytes(range(256)) * 2048
            def echo():
                try:
                    connection, _ = upstream.accept()
                    with connection:
                        received = bytearray()
                        while True:
                            chunk = connection.recv(16384)
                            if not chunk: break
                            received.extend(chunk)
                        self.assertEqual(received, data)
                        connection.sendall(received)
                except BaseException as error: errors.append(error)
            server = threading.Thread(target=echo);server.start()
            with selectors.DefaultSelector() as selector:
                relay = TOR.Relay(selector, listener, (socket.AF_UNIX, path))
                done = threading.Event()
                def transfer():
                    try:
                        with socket.create_connection(listener.getsockname(), timeout=5) as client:
                            client.sendall(data);client.shutdown(socket.SHUT_WR)
                            received = bytearray()
                            while True:
                                chunk = client.recv(16384)
                                if not chunk:break
                                received.extend(chunk)
                            self.assertEqual(received, data)
                    except BaseException as error: errors.append(error)
                    finally: done.set()
                client = threading.Thread(target=transfer);client.start()
                deadline=time.monotonic()+8
                while not done.is_set() and time.monotonic()<deadline:
                    for key, mask in selector.select(.1):key.data.event(key.fileobj,mask)
                client.join(timeout=1);server.join(timeout=1)
                listener.close();upstream.close()
                self.assertFalse(errors, errors)
                self.assertTrue(done.is_set())

if __name__ == '__main__':
    unittest.main()
