#!/usr/bin/python3
"""Privileged PocketCHIP GPU setup; normal desktop processes remain unprivileged.

Only the verified CHIP flash-kernel layout is supported. Never install a whole
reference DTB: merge the device-validated fixed-frequency OPP into the existing
board tree, and check that every pre-existing property survives unchanged.
"""
import argparse
import grp
import fcntl
import hashlib
import json
import os
from pathlib import Path
import pwd
import re
import select
import time
import zlib
import stat
import struct
import subprocess
import sys
import tempfile

GPU = '/soc/gpu@1c40000'
STATE = Path('/var/lib/vitrallis-pocketchip')
HELPER = Path('/usr/libexec/vitrallis-pocketchip-gpu.py')
TRACE = Path('/sys/kernel/tracing')
INSTANCE = TRACE / 'instances/vitrallis-gpu'
READER = Path('/run/vitrallis-gpu/trace_pipe')
SERVICE = Path('/etc/systemd/system/vitrallis-gpu-trace.service')
HOOK = Path('/etc/kernel/postinst.d/zz-fd-vitrallis-gpu')
OVERLAY = b'''/dts-v1/;
/plugin/;
/ {
    fragment@0 {
        target-path = "/";
        __overlay__ {
            gpu_opp_table: opp-table-gpu {
                compatible = "operating-points-v2";
                opp-297000000 { opp-hz = /bits/ 64 <297000000>; };
            };
        };
    };
    fragment@1 {
        target-path = "/soc/gpu@1c40000";
        __overlay__ { operating-points-v2 = <&gpu_opp_table>; };
    };
};
'''
UNIT = '''[Unit]
Description=Vitrallis dedicated GPU utilization trace
After=systemd-modules-load.service
ConditionPathExists=/sys/firmware/devicetree/base

[Service]
Type=oneshot
ExecStart=/usr/bin/python3 -I /usr/libexec/vitrallis-pocketchip-gpu.py --boot
RemainAfterExit=yes
ExecStop=/usr/bin/python3 -I /usr/libexec/vitrallis-pocketchip-gpu.py --stop

[Install]
WantedBy=multi-user.target
'''.encode()
KERNEL_HOOK = b'''#!/bin/sh
# Runs before zz-flash-kernel; its copied DTB retains this kernel's board data.
set -eu
exec /usr/bin/python3 -I /usr/libexec/vitrallis-pocketchip-gpu.py --kernel "$1"
'''


def run(argv):
    return subprocess.run(argv, check=True, stdout=subprocess.PIPE, stderr=subprocess.PIPE,
                          timeout=30).stdout


def read_file(path, limit=4 * 1024 * 1024):
    with path.open('rb') as stream:
        data = stream.read(limit + 1)
    if len(data) > limit: raise ValueError('Oversized platform input: ' + str(path))
    return data


def properties(data):
    """Read an FDT's structure for validation, never as executable DTS text."""
    if len(data) < 40 or len(data) > 4 * 1024 * 1024:
        raise ValueError('Invalid DTB length')
    magic, size, structure, strings, _, version, _, _, string_size, struct_size = struct.unpack_from('>10I', data)
    if (magic != 0xd00dfeed or size != len(data) or version < 17
            or structure < 40 or strings < 40 or structure + struct_size > size
            or strings + string_size > size):
        raise ValueError('Invalid DTB header')
    names, nodes, stack = data[strings:strings + string_size], {}, []
    pos, end = structure, structure + struct_size
    def word():
        nonlocal pos
        if pos + 4 > end: raise ValueError('Truncated DTB')
        result = struct.unpack_from('>I', data, pos)[0]
        pos += 4
        return result
    while pos < end:
        token = word()
        if token == 1:
            stop = data.find(b'\0', pos, end)
            if stop < 0: raise ValueError('Unterminated DT node')
            name = data[pos:stop].decode('ascii')
            if '/' in name or (not stack and name): raise ValueError('Invalid DT node name')
            stack.append(name)
            path = '/'.join(stack) or '/'
            if path in nodes: raise ValueError('Duplicate DT node')
            nodes[path] = {}
            pos = (stop + 4) & ~3
        elif token == 2:
            if not stack: raise ValueError('Unbalanced DTB')
            stack.pop()
        elif token == 3:
            length, offset = word(), word()
            stop = names.find(b'\0', offset)
            if not stack or stop < offset or pos + length > end:
                raise ValueError('Invalid DT property')
            name = names[offset:stop].decode('ascii')
            values = nodes['/'.join(stack) or '/']
            if name in values: raise ValueError('Duplicate DT property')
            values[name] = data[pos:pos + length]
            pos = (pos + length + 3) & ~3
        elif token == 4:
            continue
        elif token == 9 and not stack:
            return nodes
        else:
            raise ValueError('Invalid DT structure')
    raise ValueError('Incomplete DTB')


def has_opp(nodes):
    gpu = nodes.get(GPU, {})
    reference = gpu.get('operating-points-v2')
    if reference is None: return False
    if len(reference) != 4: raise ValueError('Invalid GPU OPP reference; refusing replacement')
    matches = [path for path, values in nodes.items() if values.get('phandle') == reference]
    if len(matches) != 1: raise ValueError('Unresolved GPU OPP reference; refusing replacement')
    table = matches[0]
    if nodes[table].get('compatible') != b'operating-points-v2\0':
        raise ValueError('Unknown GPU OPP binding; refusing replacement')
    for path, values in nodes.items():
        hz = values.get('opp-hz', b'')
        if path.rpartition('/')[0] == table and len(hz) == 8 and int.from_bytes(hz, 'big') > 0:
            if values.get('status', b'okay\0') in (b'okay\0', b'ok\0'):
                return True
    raise ValueError('GPU OPP table has no usable frequency; refusing replacement')


def validate_board(nodes):
    compatible = nodes.get('/', {}).get('compatible', b'').split(b'\0')
    if b'nextthing,chip' not in compatible or b'allwinner,sun5i-r8' not in compatible:
        raise ValueError('Not the supported CHIP board DTB')
    if b'arm,mali-400' not in nodes.get(GPU, {}).get('compatible', b'').split(b'\0'):
        raise ValueError('Expected Mali-400 GPU node is missing')


def reservations(data):
    position = int.from_bytes(data[16:20], 'big')
    if position < 40 or position % 8:
        raise ValueError('Invalid DTB reservation offset')
    values = []
    while position + 16 <= len(data):
        entry = data[position:position + 16]
        if entry == bytes(16): return values
        values.append(entry)
        position += 16
    raise ValueError('Unterminated DTB reservation map')


def patched(data):
    before = properties(data)
    validate_board(before)
    if has_opp(before): return data
    if '/opp-table-gpu' in before:
        raise ValueError('Unreferenced GPU OPP table exists; refusing to overwrite it')
    with tempfile.TemporaryDirectory(prefix='vitrallis-dtb-') as temporary:
        root = Path(temporary)
        (root / 'base.dtb').write_bytes(data)
        (root / 'gpu.dts').write_bytes(OVERLAY)
        run(['/usr/bin/dtc', '-@', '-I', 'dts', '-O', 'dtb', '-o', str(root / 'gpu.dtbo'), str(root / 'gpu.dts')])
        run(['/usr/bin/fdtoverlay', '-i', str(root / 'base.dtb'), '-o', str(root / 'new.dtb'), str(root / 'gpu.dtbo')])
        result = (root / 'new.dtb').read_bytes()
    after = properties(result)
    if reservations(data) != reservations(result) or data[28:32] != result[28:32]:
        raise ValueError('Overlay changed reserved memory or boot CPU')
    for path, values in before.items():
        for key, value in values.items():
            if after.get(path, {}).get(key) != value:
                raise ValueError('Overlay changed existing DT property: ' + path + '/' + key)
    if not has_opp(after): raise ValueError('Overlay did not create usable GPU OPP data')
    return result


def safe(path, regular=False):
    """Root-owned non-writable ancestors and single-link regular destinations."""
    for part in reversed((path,) + tuple(path.parents)):
        try: info = part.lstat()
        except FileNotFoundError: continue
        if stat.S_ISLNK(info.st_mode) or info.st_uid != 0 or info.st_mode & 0o022:
            raise ValueError('Unsafe platform path: ' + str(part))
        if part == path and regular and (not stat.S_ISREG(info.st_mode) or info.st_nlink != 1):
            raise ValueError('Expected a single-link regular file: ' + str(path))


def atomic(path, data, mode=0o644):
    safe(path, regular=True)
    path.parent.mkdir(parents=True, exist_ok=True, mode=0o755)
    fd, name = tempfile.mkstemp(prefix='.vitrallis-', dir=path.parent)
    try:
        with os.fdopen(fd, 'wb') as stream:
            stream.write(data)
            os.fchmod(stream.fileno(), mode)
            stream.flush()
            os.fsync(stream.fileno())
        os.replace(name, path)
        directory = os.open(path.parent, os.O_RDONLY | os.O_DIRECTORY)
        try: os.fsync(directory)
        finally: os.close(directory)
    finally:
        if os.path.exists(name): os.unlink(name)


def dtb_path(version, boot=False):
    if not re.fullmatch(r'[A-Za-z0-9][A-Za-z0-9.+_-]{0,127}', version):
        raise ValueError('Invalid kernel version')
    root = Path('/boot/dtbs') if boot else Path('/usr/lib')
    return root / (version if boot else 'linux-image-' + version) / 'sun5i-r8-chip.dtb'


def recover_dtbs():
    journal = STATE / 'dtb-transaction.json'
    safe(journal, regular=True)
    if not journal.exists(): return
    records = json.loads(read_file(journal, 4096))
    if not isinstance(records, list) or not 1 <= len(records) <= 2:
        raise ValueError('Invalid DTB recovery journal')
    restore = []
    for record in records:
        if not isinstance(record, dict) or set(record) != {'path', 'before', 'after'}:
            raise ValueError('Invalid DTB recovery entry')
        name = record['path']
        if not isinstance(name, str) or not re.fullmatch(
                r'/(?:boot/dtbs/|usr/lib/linux-image-)[A-Za-z0-9][A-Za-z0-9.+_-]{0,127}/sun5i-r8-chip\.dtb', name):
            raise ValueError('Unsafe DTB recovery path')
        if any(not isinstance(record[key], str) or not re.fullmatch(r'[0-9a-f]{64}', record[key])
               for key in ('before', 'after')):
            raise ValueError('Invalid DTB recovery digest')
        path = Path(name)
        safe(path, regular=True)
        current = hashlib.sha256(path.read_bytes()).hexdigest()
        if current == record['before']: continue
        if current != record['after']: raise ValueError('DTB edited during interrupted setup: ' + name)
        backup = STATE / 'dtb-backups' / (record['before'] + '.dtb')
        safe(backup, regular=True)
        data = backup.read_bytes()
        if hashlib.sha256(data).hexdigest() != record['before']:
            raise ValueError('Corrupt DTB recovery backup')
        restore.append((path, data))
    # If every publish finished, only journal cleanup was interrupted. Keep the
    # complete new generation (especially if it has already booted). Otherwise
    # roll back the partial set after validating all current bytes and backups.
    if len(restore) != len(records):
        for path, data in restore: atomic(path, data)
    journal.unlink()


def update_dtbs(paths):
    changes = {}
    for path in paths:
        safe(path, regular=True)
        data = read_file(path)
        result = patched(data)
        if result != data: changes[path] = (data, result)
    # Validate every tree before any write; durable original backups precede it.
    for path, (data, _) in changes.items():
        backup = STATE / 'dtb-backups' / (hashlib.sha256(data).hexdigest() + '.dtb')
        safe(backup, regular=True)
        if backup.exists():
            if backup.read_bytes() != data: raise ValueError('Conflicting DTB backup')
        else: atomic(backup, data)
    if not changes: return False
    journal = STATE / 'dtb-transaction.json'
    atomic(journal, json.dumps([{'path': str(path),
                                'before': hashlib.sha256(before).hexdigest(),
                                'after': hashlib.sha256(after).hexdigest()}
                               for path, (before, after) in changes.items()]).encode(), 0o600)
    changed = []
    try:
        for path, (before, after) in changes.items():
            if path.read_bytes() != before: raise ValueError('DTB changed during setup')
            changed.append(path)
            atomic(path, after)
    except BaseException:
        for path in reversed(changed):
            before, after = changes[path]
            if path.read_bytes() == after: atomic(path, before)
        # Leave the journal for interruption recovery (the normal rollback is
        # already byte-identical; it will be cleared after revalidation).
        raise
    journal.unlink()
    return True


def live_nodes():
    root = Path('/sys/firmware/devicetree/base')
    result = {}
    for folder, _, files in os.walk(root):
        path = Path(folder)
        key = '/' + str(path.relative_to(root)) if path != root else '/'
        result[key] = {name: (path / name).read_bytes() for name in files}
    return result


def active_boot_version():
    # This is the observed flash-kernel/U-Boot path, not an assumed /boot/dtb link.
    safe(Path('/boot/boot.scr'), regular=True)
    data = read_file(Path('/boot/boot.scr'))
    if len(data) < 72 or data[:4] != b'\x27\x05\x19\x56':
        raise ValueError('Unsupported boot script; expected CHIP U-Boot image')
    header = bytearray(data[:64])
    expected_header = int.from_bytes(header[4:8], 'big')
    header[4:8] = b'\0' * 4
    if (zlib.crc32(header) != expected_header
            or int.from_bytes(data[12:16], 'big') != len(data) - 64
            or zlib.crc32(data[64:]) != int.from_bytes(data[24:28], 'big')):
        raise ValueError('Boot script checksum/length mismatch')
    text = data[72:].decode('ascii')
    matches = re.findall(r'^ubifsload 0x43000000 /boot/dtbs/([A-Za-z0-9.+_-]+)/sun5i-r8-chip.dtb\s*$', text, re.M)
    if len(matches) != 1 or 'bootz 0x42000000 - 0x43000000' not in text:
        raise ValueError('Unsupported boot DTB selection; refusing to guess')
    return matches[0]


def private_group(user):
    group = grp.getgrgid(user.pw_gid)
    if any(member != user.pw_name for member in group.gr_mem) or any(
            account.pw_gid == user.pw_gid and account.pw_uid != user.pw_uid
            for account in pwd.getpwall()):
        raise ValueError('GPU trace access requires a private primary desktop group')
    return user.pw_gid


def grant_read(path, gid):
    """Grant the private desktop group read access to the dedicated pipe only."""
    info = path.lstat()
    if stat.S_ISLNK(info.st_mode) or info.st_uid != 0 or info.st_gid not in (0, gid):
        raise ValueError('Unmanaged tracefs permissions: ' + str(path))
    os.chown(path, 0, gid)
    os.chmod(path, 0o440)


def mount_info(path):
    for line in read_file(Path('/proc/self/mountinfo')).decode('ascii').splitlines():
        fields = line.split()
        if len(fields) > 9 and fields[4] == str(path):
            separator = fields.index('-')
            return fields[3], fields[separator + 1], fields[5].split(',')
    return None


def reader_mounted():
    mounted = mount_info(READER)
    if mounted is None: return False
    if mounted[:2] != ('/instances/vitrallis-gpu/trace_pipe', 'tracefs'):
        raise ValueError('Unmanaged mount at GPU reader path')
    return True


def configure_trace(gid):
    mounted = mount_info(TRACE)
    if mounted is None:
        run(['/usr/bin/mount', '-t', 'tracefs', '-o', 'nosuid,nodev,noexec', 'tracefs', str(TRACE)])
        mounted = mount_info(TRACE)
    if mounted is None or mounted[:2] != ('/', 'tracefs'):
        raise ValueError('Expected the canonical tracefs mount')
    # Root owns every control. We never write the global tracing_on/events files.
    INSTANCE.mkdir(exist_ok=True)
    (INSTANCE / 'tracing_on').write_text('0')
    try:
        (INSTANCE / 'events/enable').write_text('0')
        (INSTANCE / 'current_tracer').write_text('nop')
        (INSTANCE / 'buffer_size_kb').write_text('16')
        (INSTANCE / 'buffer_percent').write_text('0')
        (INSTANCE / 'trace_clock').write_text('mono')
        event = INSTANCE / 'events/devfreq/devfreq_monitor'
        (event / 'filter').write_text('dev_name == "1c40000.gpu"')
        (event / 'enable').write_text('1')
        grant_read(INSTANCE / 'trace_pipe', gid)
        # Expose only the dedicated pipe through a read-only bind mount. Giving
        # traversal to the tracefs root also exposes its world-readable global
        # files, so the normal session never gets access to that directory.
        safe(READER.parent)
        READER.parent.mkdir(mode=0o755, exist_ok=True)
        if not reader_mounted():
            safe(READER, regular=True)
            if not READER.exists(): atomic(READER, b'', 0o600)
            run(['/usr/bin/mount', '--bind', str(INSTANCE / 'trace_pipe'), str(READER)])
        run(['/usr/bin/mount', '-o', 'remount,bind,ro,nosuid,nodev,noexec', str(READER)])
        (INSTANCE / 'tracing_on').write_text('1')
    except BaseException:
        (INSTANCE / 'tracing_on').write_text('0')
        raise


def status():
    nodes = live_nodes()
    configured = has_opp(nodes)
    device = Path('/sys/bus/platform/devices/1c40000.gpu')
    bound = (device / 'driver').resolve().name == 'lima'
    devfreq = device / 'devfreq/1c40000.gpu'
    try:
        frequency = int(read_file(devfreq / 'cur_freq', 64).strip())
        if not 0 < frequency < 2**64: frequency = None
    except (OSError, ValueError): frequency = None
    return {'reboot_required': not configured, 'lima_bound': bound,
            'devfreq': devfreq.is_dir(), 'frequency_hz': frequency}


def save_status(extra=None):
    result = status()
    if extra: result.update(extra)
    atomic(STATE / 'gpu-status.json', (json.dumps(result) + '\n').encode())
    print(json.dumps(result), flush=True)
    if result['reboot_required']:
        print('REBOOT REQUIRED: GPU OPP configuration is installed; restart PocketCHIP to activate it.', flush=True)
    return result


def install(username):
    user = pwd.getpwnam(username)
    if user.pw_uid == 0: raise ValueError('Choose the normal desktop user')
    nodes = live_nodes()
    validate_board(nodes)
    if b'nextthing,pocketchip' not in nodes['/'].get('compatible', b'').split(b'\0'):
        raise ValueError('Platform setup requires PocketCHIP')
    version = active_boot_version()
    if any(Path('/etc/flash-kernel/dtbs').rglob('sun5i-r8-chip.dtb')):
        raise ValueError('Custom flash-kernel DTB override needs review before automatic GPU setup')
    # Preflight tools/permissions before installing boot changes.
    for path in ('/usr/bin/dtc', '/usr/bin/fdtoverlay', '/usr/bin/systemctl'):
        if not os.access(path, os.X_OK): raise ValueError('Missing prerequisite: ' + path)
    private_group(user)
    for path in (HELPER, SERVICE, HOOK, STATE / 'user.json'):
        safe(path, regular=True)
    update_dtbs([dtb_path(version), dtb_path(version, boot=True)])
    atomic(HELPER, Path(__file__).read_bytes())
    atomic(HOOK, KERNEL_HOOK, 0o755)
    atomic(SERVICE, UNIT)
    atomic(STATE / 'user.json', json.dumps({'uid': user.pw_uid, 'name': username}).encode(), 0o600)
    run(['/usr/bin/systemctl', 'daemon-reload'])
    run(['/usr/bin/systemctl', 'enable', 'vitrallis-gpu-trace.service'])
    # Before reboot, devfreq can be absent. The boot service retries with the new
    # tree; missing telemetry never prevents desktop/session installation.
    save_status()


def verify():
    result = status()
    print(json.dumps(result), flush=True)
    if result['reboot_required']:
        raise ValueError('Reboot is required before runtime verification')
    if not result['lima_bound'] or not result['devfreq'] or not result['frequency_hz']:
        raise ValueError('Lima/devfreq/frequency check failed')
    fd = os.open(READER, os.O_RDONLY | os.O_NONBLOCK | os.O_NOFOLLOW)
    try:
        fcntl.flock(fd, fcntl.LOCK_EX | fcntl.LOCK_NB)
        deadline = time.monotonic() + 3
        pending = b''
        while time.monotonic() < deadline:
            if not select.select([fd], [], [], max(0, deadline - time.monotonic()))[0]: break
            try: data = os.read(fd, 16384)
            except BlockingIOError: continue
            if not data: break
            lines = (pending + data).split(b'\n')
            pending = lines.pop()[-4096:]
            for line in lines:
                match = re.search(rb' ([0-9]+\.[0-9]+): devfreq_monitor:\s+dev_name=1c40000\.gpu\s+freq=([0-9]+)\s+polling_ms=([0-9]+)\s+load=([0-9]+)\s*$', line)
                if (match and 0 <= time.monotonic() - float(match[1]) <= 2
                        and 0 < int(match[2]) < 2**64 and 0 < int(match[3]) <= 60000
                        and 0 <= int(match[4]) <= 100):
                    print('Verified real GPU sample: ' + line.decode('ascii', 'replace').strip())
                    return
        raise ValueError('No fresh GPU sample. Run a GPU workload and retry; idle Lima can runtime-suspend.')
    finally: os.close(fd)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    group = parser.add_mutually_exclusive_group(required=True)
    group.add_argument('--install-user')
    group.add_argument('--kernel')
    group.add_argument('--boot', action='store_true')
    group.add_argument('--stop', action='store_true')
    group.add_argument('--verify', action='store_true')
    args = parser.parse_args()
    if os.geteuid() != 0: raise ValueError('Platform setup needs root; run through the installer')
    safe(STATE)
    STATE.mkdir(mode=0o755, exist_ok=True)
    lock = STATE / 'lock'
    safe(lock, regular=True)
    with os.fdopen(os.open(lock, os.O_CREAT | os.O_RDWR | os.O_NOFOLLOW, 0o600), 'a+') as stream:
        fcntl.flock(stream, fcntl.LOCK_EX)
        recover_dtbs()
        if args.install_user: install(args.install_user)
        elif args.kernel: update_dtbs([dtb_path(args.kernel)])
        elif args.stop:
            if INSTANCE.exists(): (INSTANCE / 'tracing_on').write_text('0')
            if reader_mounted(): run(['/usr/bin/umount', '--lazy', str(READER)])
        elif args.verify: verify()
        else:
            try:
                user = json.loads(read_file(STATE / 'user.json', 4096))
                if pwd.getpwnam(user['name']).pw_uid != user['uid']: raise ValueError('Desktop account changed')
                configure_trace(private_group(pwd.getpwnam(user['name'])))
                save_status({'trace_configured': True})
            except (OSError, ValueError, subprocess.SubprocessError) as error:
                save_status({'trace_configured': False, 'trace_error': str(error)})
                print('GPU telemetry unavailable: ' + str(error), file=sys.stderr)

    if args.install_user:
        # The oneshot acquires the same lock. Start it after releasing ours.
        run(['/usr/bin/systemctl', 'restart', 'vitrallis-gpu-trace.service'])


if __name__ == '__main__':
    try: main()
    except (OSError, ValueError, KeyError, subprocess.SubprocessError) as error:
        print('PocketCHIP GPU setup failed: ' + str(error), file=sys.stderr)
        sys.exit(1)
