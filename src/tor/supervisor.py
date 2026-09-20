"""Embedded Arti guardian. No shell, persistent logs, PID files or app data.

stdin EOF is the Shell lifetime lease. Linux parent-death handling additionally
kills Arti if this guardian itself is killed. An inherited flock prevents restart
races even while the former Arti is still exiting.
"""
import ctypes
import fcntl
import json
import os
from pathlib import Path
import re
import selectors
import signal
import socket
import stat
import subprocess
import sys
import time

HOST, PORT = '127.0.0.1', 9150


def private(path):
    if not path.is_absolute() or '..' in path.parts or any(ord(c) < 32 for c in str(path)) or '$' in str(path):
        raise ValueError('Unsafe Tor storage path')
    for parent in reversed((path, *path.parents)):
        if parent.is_symlink():
            raise ValueError('Symlinks are forbidden in Tor storage')
        if not parent.exists():
            parent.mkdir(mode=0o700)
        mode = parent.stat()
        if not stat.S_ISDIR(mode.st_mode) or (mode.st_mode & 0o022 and not mode.st_mode & stat.S_ISVTX):
            raise ValueError('Unsafe Tor storage permissions')
    if path.stat().st_uid != os.getuid():
        raise ValueError('Tor directory belongs to another user')
    path.chmod(0o700)


def atomic(path, data):
    temporary = path.with_name(path.name + '.new')
    try:
        fd = os.open(temporary, os.O_WRONLY | os.O_CREAT | os.O_EXCL | os.O_NOFOLLOW, 0o600)
        with os.fdopen(fd, 'wb') as output:
            output.write(data)
            output.flush()
            os.fsync(output.fileno())
        if path.is_symlink() or (path.exists() and (not path.is_file() or path.stat().st_nlink != 1)):
            raise ValueError('Unsafe Tor configuration file')
        os.replace(temporary, path)
    finally:
        temporary.unlink(missing_ok=True)


def binary():
    for candidate in (sys.argv[2], '/usr/bin/arti', '/usr/local/bin/arti', '/opt/homebrew/bin/arti'):
        if not os.path.isfile(candidate):
            continue
        result = subprocess.run([candidate, '--version'], capture_output=True, timeout=5, check=True)
        if result.stdout.splitlines()[:1] == [b'Arti 2.6.0']:
            return candidate
    raise ValueError('Arti missing; complete beta4 in Settings > Device > Updates')


def death_signal(parent):
    if sys.platform == 'linux':
        libc = ctypes.CDLL(None, use_errno=True)
        if libc.prctl(1, signal.SIGTERM, 0, 0, 0) != 0 or os.getppid() != parent:
            os._exit(125)


def status(state, progress=None, diagnostic=''):
    print(json.dumps(dict(state=state, progress=progress, diagnostic=diagnostic)), flush=True)


def progress(line):
    # Arti's documented functional-proxy notification, not C Tor's log format.
    if b'Sufficiently bootstrapped; proxy now functional.' in line:
        return 100
    match = re.search(rb'(?:Bootstrapping|bootstrap)[^\r\n%]{0,100}?([0-9]{1,3})%', line, re.I)
    return min(int(match[1]), 99) if match else None


def socks_ready():
    try:
        with socket.create_connection((HOST, PORT), timeout=0.2) as stream:
            stream.sendall(b'\x05\x01\x00')
            return stream.recv(2) == b'\x05\x00'
    except OSError:
        return False


class Relay:
    """Bounded nonblocking Unix-to-SOCKS relay; no per-connection threads."""
    def __init__(self, selector, listener, destination):
        self.selector, self.listener, self.destination = selector, listener, destination
        self.peers, self.buffers, self.eof = {}, {}, set()
        selector.register(listener, selectors.EVENT_READ, self)

    def close(self, stream):
        peer = self.peers.pop(stream, None)
        for endpoint in (stream, peer):
            if endpoint is None:
                continue
            self.peers.pop(endpoint, None)
            self.buffers.pop(endpoint, None)
            self.eof.discard(endpoint)
            try:
                self.selector.unregister(endpoint)
            except (KeyError, ValueError):
                pass
            endpoint.close()

    def watch(self, stream):
        peer = self.peers[stream]
        mask = (selectors.EVENT_READ if stream not in self.eof and len(self.buffers[peer]) < 65536 else 0)
        if self.buffers[stream]: mask |= selectors.EVENT_WRITE
        try: self.selector.unregister(stream)
        except KeyError: pass
        if mask: self.selector.register(stream, mask, self)

    def event(self, stream, mask):
        if stream is self.listener:
            client, _ = stream.accept()
            if len(self.peers) >= 64:
                client.close()
                return
            upstream = socket.socket(self.destination[0], socket.SOCK_STREAM)
            try:
                upstream.settimeout(0.2)
                upstream.connect(self.destination[1])
            except OSError:
                client.close()
                upstream.close()
                return
            for a, b in ((client, upstream), (upstream, client)):
                a.setblocking(False)
                self.peers[a], self.buffers[a] = b, bytearray()
                self.selector.register(a, selectors.EVENT_READ, self)
            return
        try:
            peer = self.peers.get(stream)
            if peer is None:
                return
            if mask & selectors.EVENT_READ:
                data = stream.recv(min(16384, 65536 - len(self.buffers[peer])))
                if not data:
                    self.eof.add(stream)
                    if not self.buffers[peer]: peer.shutdown(socket.SHUT_WR)
                else:
                    self.buffers[peer].extend(data)
            if mask & selectors.EVENT_WRITE:
                pending = self.buffers[stream]
                sent = stream.send(pending)
                del pending[:sent]
                if not pending and peer in self.eof: stream.shutdown(socket.SHUT_WR)
            if stream in self.eof and peer in self.eof and not self.buffers[stream] and not self.buffers[peer]:
                self.close(stream)
            else:
                self.watch(stream)
                self.watch(peer)
        except BlockingIOError:
            pass
        except (OSError, KeyError):
            self.close(stream)


def run(root):
    os.umask(0o077)
    private(root)
    fd = os.open(root / 'service.lock', os.O_RDWR | os.O_CREAT | os.O_NOFOLLOW, 0o600)
    with os.fdopen(fd, 'rb') as lock:
        if os.fstat(fd).st_nlink != 1 or not stat.S_ISREG(os.fstat(fd).st_mode):
            raise ValueError('Unsafe Tor lock file')
        try:
            fcntl.flock(lock, fcntl.LOCK_EX | fcntl.LOCK_NB)
        except BlockingIOError as error:
            raise ValueError('Another Shell owns Tor; close it before retrying') from error
        # Never adopt or kill an unrelated process merely because a port/PID exists.
        with socket.socket() as probe:
            probe.setsockopt(socket.SOL_SOCKET, socket.SO_REUSEADDR, 1)
            probe.bind((HOST, PORT))
        for name in ('cache', 'state'):
            private(root / name)
        config = ('[proxy]\nsocks_listen = "127.0.0.1:9150"\n'
                  '[storage]\ncache_dir = ' + json.dumps(str(root / 'cache')) + '\nstate_dir = '
                  + json.dumps(str(root / 'state')) + '\n[logging]\nconsole = "info"\n'
                  '[application]\nwatch_configuration = false\n')
        # Recover only our own bounded temporary file under the exclusive lock.
        (root / 'arti.toml.new').unlink(missing_ok=True)
        atomic(root / 'arti.toml', config.encode())
        def listen(name):
            path = root / name
            if path.exists() or path.is_symlink():
                if not stat.S_ISSOCK(path.lstat().st_mode) or path.lstat().st_uid != os.getuid():
                    raise ValueError('Unsafe Tor service socket')
                path.unlink()
            listener = socket.socket(socket.AF_UNIX, socket.SOCK_STREAM)
            listener.bind(str(path))
            listener.listen(16)
            listener.setblocking(False)
            return listener
        listener = listen('proxy.sock')
        status_listener = listen('status.sock')
        child = None
        try:
            parent = os.getpid()
            child = subprocess.Popen([binary(), 'proxy', '-c', str(root / 'arti.toml')],
                                     stdin=subprocess.DEVNULL, stdout=subprocess.PIPE,
                                     stderr=subprocess.STDOUT, pass_fds=(fd,),
                                     env={'HOME': os.environ.get('HOME', ''), 'PATH': '/usr/bin:/bin', 'LANG': 'C', 'RUST_LOG_STYLE': 'never'},
                                     preexec_fn=lambda: death_signal(parent))
            with selectors.DefaultSelector() as selector:
                selector.register(sys.stdin, selectors.EVENT_READ, 'owner')
                selector.register(status_listener, selectors.EVENT_READ, 'status')
                selector.register(child.stdout, selectors.EVENT_READ, 'log')
                relay = Relay(selector, listener, (socket.AF_INET, (HOST, PORT)))
                status('bootstrapping')
                deadline, connected, pending, last = time.monotonic() + 180, False, b'', None
                while child.poll() is None:
                    for key, mask in selector.select(1):
                        if key.data == 'owner':
                            if not os.read(sys.stdin.fileno(), 1024):
                                return
                        elif key.data == 'status':
                            connection, _ = status_listener.accept()
                            with connection:
                                connection.settimeout(0.1)
                                try:
                                    connection.sendall(json.dumps(dict(api=1, available=last == 100,
                                        socks_host=HOST, socks_port=PORT,
                                        state='connected' if last == 100 else 'bootstrapping',
                                        progress=last)).encode() + b'\n')
                                except OSError:
                                    pass
                        elif key.data == 'log':
                            data = os.read(child.stdout.fileno(), 4096)
                            if not data:
                                selector.unregister(child.stdout)
                            pending = (pending + data)[-8192:]
                            lines = pending.split(b'\n')
                            pending = lines.pop()
                            for line in lines:
                                percent = progress(line)
                                if percent == 100:
                                    connected = True
                                elif percent is not None and percent != last:
                                    last = percent
                                    status('bootstrapping', percent)
                        else:
                            key.data.event(key.fileobj, mask)
                    if connected and last != 100 and socks_ready():
                        last = 100
                        status('connected', 100)
                    if last != 100 and time.monotonic() >= deadline:
                        raise ValueError('Tor bootstrap timed out; check connectivity and clock')
                raise ValueError('Arti exited with status ' + str(child.returncode))
        finally:
            listener.close()
            status_listener.close()
            (root / 'proxy.sock').unlink(missing_ok=True)
            (root / 'status.sock').unlink(missing_ok=True)
            if child is not None:
                child.terminate() if child.poll() is None else None
                try:
                    child.wait(timeout=2)
                except subprocess.TimeoutExpired:
                    child.kill()
                    child.wait()
                child.stdout.close()


def main():
    signal.signal(signal.SIGTERM, lambda *_: sys.exit(0))
    signal.signal(signal.SIGINT, lambda *_: sys.exit(0))
    try:
        run(Path(sys.argv[1]))
    except (OSError, ValueError, subprocess.SubprocessError) as error:
        # Only guardian-generated diagnostics. Never forward circuit addresses,
        # SOCKS credentials, app arguments, or arbitrary Arti log records.
        diagnostic = str(error) if isinstance(error, ValueError) else 'Tor service operation failed (' + type(error).__name__ + ')'
        status('error', diagnostic=diagnostic)
        return 1
    return 0


if __name__ == '__main__':
    sys.exit(main())
