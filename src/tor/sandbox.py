"""Run only inside bubblewrap's fresh user/PID/network namespaces.

The namespace has only loopback. A bounded bridge exposes the shared Arti Unix
socket at the normal TCP endpoint. Direct TCP/UDP/DNS has no route to the host.
This guards accidental clearnet access, not malicious same-user desktop apps
(which can already control their shared X11 session).
"""
import os
import json
import selectors
import socket
import subprocess
import sys

# Supplied by Shell, never from an application manifest.
source, guardian, program, *arguments = sys.argv[1:]
try:
    with socket.socket(socket.AF_UNIX, socket.SOCK_STREAM) as state:
        state.settimeout(1)
        state.connect(os.path.join(os.path.dirname(source), 'status.sock'))
        record = bytearray()
        while b'\n' not in record and len(record) < 1024:
            data = state.recv(1024 - len(record))
            if not data: break
            record.extend(data)
        if json.loads(record).get('available') is not True:
            raise OSError('Tor is not ready')
    with socket.socket(socket.AF_UNIX, socket.SOCK_STREAM) as probe:
        probe.settimeout(1)
        probe.connect(source)
        probe.sendall(b'\x05\x01\x00')
        if probe.recv(2) != b'\x05\x00': raise OSError('proxy handshake')
except (OSError, ValueError):
    sys.exit('Tor required: shared service unavailable; launch from Vitrallis Shell')
namespace = {'__name__': 'tor_relay'}
exec(guardian, namespace)
listener = socket.socket()
listener.bind(('127.0.0.1', 9150))
listener.listen(16)
listener.setblocking(False)
with selectors.DefaultSelector() as selector:
    relay = namespace['Relay'](selector, listener, (socket.AF_UNIX, source))
    environment = dict(os.environ)
    environment.update(ALL_PROXY='socks5h://127.0.0.1:9150', all_proxy='socks5h://127.0.0.1:9150',
                       NO_PROXY='', no_proxy='', VITRALLIS_TOR_AVAILABLE='1')
    child = subprocess.Popen([program, *arguments], env=environment)
    try:
        while child.poll() is None:
            for key, mask in selector.select(0.5):
                key.data.event(key.fileobj, mask)
    finally:
        if child.poll() is None:
            child.terminate()
        child.wait()
        listener.close()
sys.exit(child.returncode)
