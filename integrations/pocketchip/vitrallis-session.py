#!/usr/bin/python3
"""Reversible Awesome launch target, supervised by the existing user systemd."""
import os
import ctypes
import fcntl
from pathlib import Path
import selectors
import resource
import signal
import stat
import subprocess
import sys
import tempfile

UNIT = 'vitrallis-session'
LIMIT = 128 * 1024
HOME_HOOK = '''
if not vitrallis_home_route then
    local route = {removed = {}, added = {}, previous = client.focus}
    local keys = {}
    for _, key in ipairs(root.keys()) do
        if key.key == "XF86PowerOff" and #key.modifiers == 0 then
            table.insert(route.removed, key)
        else
            table.insert(keys, key)
        end
    end
    local awful = require("awful")
    route.added = awful.key({}, "XF86PowerOff", function()
        for _, c in ipairs(client.get()) do
            if c.name == "Vitrallis" then
                client.focus = c; c:raise(); return
            end
        end
        if route.previous and route.previous.valid then
            client.focus = route.previous; route.previous:raise()
        end
    end)
    for _, key in ipairs(route.added) do table.insert(keys, key) end
    vitrallis_home_route = route
    root.keys(keys)
end
return "vitrallis home routing active"
'''
RESTORE_HOOK = '''
if vitrallis_home_route then
    local route = vitrallis_home_route
    local keys = {}
    for _, key in ipairs(root.keys()) do
        local owned = false
        for _, added in ipairs(route.added) do
            if key == added then owned = true end
        end
        if not owned then table.insert(keys, key) end
    end
    for _, key in ipairs(route.removed) do
        local present = false
        for _, current in ipairs(keys) do
            if key == current then present = true end
        end
        if not present then table.insert(keys, key) end
    end
    root.keys(keys)
    if route.previous and route.previous.valid then
        client.focus = route.previous; route.previous:raise()
    end
    vitrallis_home_route = nil
end
return "original session home routing restored"
'''


def awesome(code):
    result = subprocess.run(['/usr/bin/awesome-client', code],
                            stdin=subprocess.DEVNULL, stdout=subprocess.PIPE,
                            stderr=subprocess.PIPE, timeout=5, check=True)
    if b'string "' not in result.stdout or b'Error' in result.stdout:
        raise RuntimeError('Awesome did not confirm the session operation')


def regular(path):
    if any(p.is_symlink() for p in (path,) + tuple(path.parents)):
        raise ValueError('Refusing symlink: ' + str(path))
    if path.exists() and path.stat().st_nlink != 1:
        raise ValueError('Refusing hardlink: ' + str(path))
    if path.exists() and not path.is_file():
        raise ValueError('Expected regular file: ' + str(path))


def log_chunk(path, data):
    if len(data) > LIMIT:
        raise ValueError('Log chunk exceeds limit')
    regular(path)
    previous = path.with_suffix('.log.1')
    regular(previous)
    if path.exists() and path.stat().st_size + len(data) > LIMIT:
        os.replace(path, previous)
    fd = os.open(path, os.O_WRONLY | os.O_APPEND | os.O_CREAT | os.O_NOFOLLOW | os.O_NONBLOCK, 0o600)
    with os.fdopen(fd, 'ab') as stream:
        info = os.fstat(stream.fileno())
        if not stat.S_ISREG(info.st_mode) or info.st_nlink != 1:
            raise ValueError('Expected a regular log file with one link')
        stream.write(data)


def start_compositor(log):
    """Own one effects-free X Present compositor for this session, if needed.

    Child/windowed swaps cannot pageflip an uncomposited root window. XRender
    composition uses the existing glamor acceleration and a full-screen Present
    backbuffer. Never replace another compositor or daemonize outside our unit.
    """
    display = None
    try:
        x = ctypes.CDLL('libX11.so.6')
        x.XOpenDisplay.argtypes = [ctypes.c_char_p]
        x.XOpenDisplay.restype = ctypes.c_void_p
        x.XDefaultScreen.argtypes = [ctypes.c_void_p]
        x.XDefaultScreen.restype = ctypes.c_int
        x.XInternAtom.argtypes = [ctypes.c_void_p, ctypes.c_char_p, ctypes.c_int]
        x.XInternAtom.restype = ctypes.c_ulong
        x.XGetSelectionOwner.argtypes = [ctypes.c_void_p, ctypes.c_ulong]
        x.XGetSelectionOwner.restype = ctypes.c_ulong
        x.XQueryExtension.argtypes = [ctypes.c_void_p, ctypes.c_char_p,
                                     ctypes.POINTER(ctypes.c_int), ctypes.POINTER(ctypes.c_int),
                                     ctypes.POINTER(ctypes.c_int)]
        x.XQueryExtension.restype = ctypes.c_int
        x.XCloseDisplay.argtypes = [ctypes.c_void_p]
        x.XCloseDisplay.restype = ctypes.c_int
        display = x.XOpenDisplay(None)
        if not display:
            raise OSError('Cannot inspect X11 compositor ownership')
        name = ('_NET_WM_CM_S' + str(x.XDefaultScreen(display))).encode('ascii')
        atom = x.XInternAtom(display, name, 0)
        if x.XGetSelectionOwner(display, atom):
            log_chunk(log, b'presentation compositor=existing synchronization=externally-managed\n')
            return None
        opcode, event, error = ctypes.c_int(), ctypes.c_int(), ctypes.c_int()
        if not x.XQueryExtension(display, b'Present', ctypes.byref(opcode),
                                 ctypes.byref(event), ctypes.byref(error)):
            raise OSError('X Present extension unavailable')
        log_chunk(log, b'presentation compositor=picom backend=xrender buffering=present-pixmaps vsync=requested\n')
        process = subprocess.Popen(['/usr/bin/picom', '--config', '/dev/null',
                                    '--backend', 'xrender', '--vsync'],
                                   stdin=subprocess.DEVNULL, stdout=subprocess.PIPE,
                                   stderr=subprocess.STDOUT)
        return process
    except OSError as error:
        log_chunk(log, ('presentation compositor=unavailable tearing_possible=true: '
                        + str(error) + '\n').encode())
        return None
    finally:
        if display:
            x.XCloseDisplay(display)


def supervise(base):
    marker = base / '.installation-pending'
    regular(marker)
    if marker.exists():
        raise RuntimeError('Vitrallis installation incomplete; rerun installer')
    removal = base / '.vitrallis-update/removal'
    if removal.exists() or removal.is_symlink():
        raise RuntimeError('Vitrallis removal is pending; rerun the local uninstaller')
    current = base / 'current'
    if not current.is_symlink():
        raise RuntimeError('Missing native build pointer: ' + str(current))
    relative = Path(os.readlink(current))
    if (len(relative.parts) != 2 or relative.parts[0] != 'generations'
            or len(relative.parts[1]) != 64
            or any(c not in '0123456789abcdef' for c in relative.parts[1])):
        raise RuntimeError('Invalid native build pointer')
    # The shell exposes a repair diagnostic for any missing native companion.
    # Installation still validates the complete bundle before publication.
    binary = base / relative / 'vitrallis'
    regular(binary)
    if not binary.is_file() or not os.access(binary, os.X_OK):
        raise RuntimeError('Missing executable: ' + str(binary))
    log = base / 'session.log'
    regular(log)
    regular(log.with_suffix('.log.1'))
    child = None
    compositor = None
    def stopping(signum, frame):
        if child is not None:
            child.terminate()
    signal.signal(signal.SIGTERM, stopping)
    signal.signal(signal.SIGINT, stopping)
    try:
        awesome(HOME_HOOK)
        compositor = start_compositor(log)
        child = subprocess.Popen([str(binary), '--linux-handheld'], stdin=subprocess.DEVNULL,
                                 stdout=subprocess.PIPE, stderr=subprocess.STDOUT)
        # Drain continuously; app output cannot fill an undrained pipe or grow
        # the log without bound. systemd removes every child on supervisor exit.
        with selectors.DefaultSelector() as selector:
            selector.register(child.stdout, selectors.EVENT_READ)
            if compositor is not None:
                selector.register(compositor.stdout, selectors.EVENT_READ)
            while True:
                for key, _ in selector.select(timeout=0.25):
                    data = os.read(key.fd, 8192)
                    if data:
                        log_chunk(log, data)
                    else:
                        selector.unregister(key.fileobj)
                        if compositor is not None and key.fileobj is compositor.stdout:
                            log_chunk(log, b'presentation compositor=exited tearing_possible=true\n')
                if child.poll() is not None:
                    break
        return child.wait()
    finally:
        for process in (child, compositor):
            if process is None:
                continue
            if process.poll() is None:
                process.terminate()
            try:
                process.wait(timeout=2)
            except subprocess.TimeoutExpired:
                process.kill()
                process.wait()
            process.stdout.close()
        awesome(RESTORE_HOOK)


def launch_command(script, env):
    for key in ('DISPLAY', 'XAUTHORITY', 'DBUS_SESSION_BUS_ADDRESS'):
        if not env.get(key) or any(c in env[key] for c in '\n\r\0'):
            raise ValueError('Launch from the existing graphical session: missing ' + key)
    # ExecStopPost also runs when the Python supervisor itself is killed.
    escaped = str(script).replace('\\', '\\\\').replace('"', '\\"').replace('%', '%%')
    args = ['/usr/bin/systemd-run', '--user', '--collect', '--unit=' + UNIT,
            '--property=KillMode=control-group', '--property=TimeoutStopSec=5',
            '--property=Restart=no', '--property=UMask=0077',
            '--property=ExecStopPost=/usr/bin/python3 "' + escaped + '" restore']
    for key in ('DISPLAY', 'XAUTHORITY', 'DBUS_SESSION_BUS_ADDRESS'):
        args.append('--setenv=' + key + '=' + env[key])
    args += ['--setenv=VITRALLIS_SESSION=1', '--setenv=SDL_VIDEODRIVER=x11', '/usr/bin/python3', str(script), 'run']
    return args


def unit_state():
    def limits():
        resource.setrlimit(resource.RLIMIT_FSIZE, (4096, 4096))
        resource.setrlimit(resource.RLIMIT_CORE, (0, 0))
    with tempfile.TemporaryFile() as output:
        subprocess.run(['/usr/bin/systemctl', '--user', 'show', UNIT + '.service',
                        '--property=LoadState,ActiveState,Transient,MainPID'],
                       stdin=subprocess.DEVNULL, stdout=output, stderr=output,
                       timeout=8, check=True, preexec_fn=limits)
        output.seek(0)
        text = output.read(4097).decode('utf-8')
    return dict(line.split('=', 1) for line in text.splitlines() if '=' in line)


def owned_session(script):
    state = unit_state()
    if state.get('LoadState') == 'not-found':
        return None
    if state.get('Transient') != 'yes':
        raise ValueError('Refusing to stop an unrelated vitrallis-session unit')
    if state.get('ActiveState') in ('inactive', 'failed') and state.get('MainPID') == '0':
        return None
    pid = state.get('MainPID', '')
    if not pid.isdigit() or int(pid) <= 1:
        raise ValueError('Cannot establish Vitrallis session ownership')
    proc = Path('/proc') / pid
    expected = ['/usr/bin/python3', str(script), 'run']
    with (proc / 'cmdline').open('rb') as stream:
        command = stream.read(4097)
    if (proc.stat().st_uid != os.getuid() or len(command) > 4096
            or command != b'\0'.join(os.fsencode(a) for a in expected) + b'\0'):
        raise ValueError('Refusing to stop a session not owned by this installation')
    # Process start identity protects against a PID being reused between checks.
    identity = (proc / 'stat').read_text().rsplit(')', 1)[1].split()[19]
    return pid, identity


def stop_owned(script, dry_run=False):
    identity = owned_session(script)
    if identity is None:
        return False
    if dry_run:
        return True
    if owned_session(script) != identity:
        raise ValueError('Session changed before stop; retry')
    subprocess.run(['/usr/bin/systemctl', '--user', 'stop', UNIT + '.service'],
                   stdin=subprocess.DEVNULL, timeout=12, check=True)
    if owned_session(script) is not None:
        raise RuntimeError('Vitrallis session is still running')
    # Also restore explicitly while the existing graphical session is available.
    # The unit's ExecStopPost performs this restoration on crashes as well.
    awesome(RESTORE_HOOK)
    return True


def launch(script):
    lock_path = script.parent / '.vitrallis-update/lock'
    regular(lock_path)
    fd = os.open(lock_path, os.O_RDONLY | os.O_NOFOLLOW | os.O_NONBLOCK)
    with os.fdopen(fd, 'r') as lock:
        info = os.fstat(lock.fileno())
        if not stat.S_ISREG(info.st_mode) or info.st_nlink != 1 or info.st_uid != os.getuid():
            raise ValueError('Unsafe session/update lock')
        fcntl.flock(lock, fcntl.LOCK_SH | fcntl.LOCK_NB)
        return subprocess.run(launch_command(script, os.environ), check=False).returncode


def main():
    script = Path(__file__).resolve()
    action = sys.argv[1] if len(sys.argv) == 2 else 'launch' if len(sys.argv) == 1 else ''
    if action == 'launch':
        return launch(script)
    if action == 'run':
        return supervise(script.parent)
    if action == 'restore':
        awesome(RESTORE_HOOK)
        return 0
    if action == 'stop':
        stop_owned(script)
        return 0
    raise ValueError('Usage: vitrallis-session.py [launch|stop|run|restore]')


if __name__ == '__main__':
    try:
        sys.exit(main())
    except (OSError, ValueError, RuntimeError, subprocess.SubprocessError) as error:
        print('Vitrallis session: ' + str(error), file=sys.stderr)
        sys.exit(1)
