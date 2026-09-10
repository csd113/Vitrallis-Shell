#!/usr/bin/python3
"""Reversible Awesome launch target, supervised by the existing user systemd."""
import os
from pathlib import Path
import selectors
import signal
import subprocess
import sys

UNIT = 'vitrallis-session'
LIMIT = 128 * 1024
HOME_HOOK = '''
if not vitrallis_saved_keys then
    vitrallis_saved_keys = root.keys()
    local keys = {}
    for _, key in ipairs(vitrallis_saved_keys) do
        if key.key ~= "XF86PowerOff" or #key.modifiers ~= 0 then
            table.insert(keys, key)
        end
    end
    local awful = require("awful")
    local gears = require("gears")
    root.keys(gears.table.join(keys, awful.key({}, "XF86PowerOff", function()
        for _, c in ipairs(client.get()) do
            if c.name == "Vitrallis" then
                client.focus = c; c:raise(); return
            end
        end
        if focus_home_screen then focus_home_screen() end
    end)))
end
return "vitrallis home routing active"
'''
RESTORE_HOOK = '''
if vitrallis_saved_keys then
    root.keys(vitrallis_saved_keys)
    vitrallis_saved_keys = nil
end
if focus_home_screen then focus_home_screen() end
return "marshmallow home routing restored"
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
    if path.exists() and not path.is_file():
        raise ValueError('Expected regular file: ' + str(path))


def log_chunk(path, data):
    regular(path)
    previous = path.with_suffix('.log.1')
    regular(previous)
    if path.exists() and path.stat().st_size + len(data) > LIMIT:
        os.replace(path, previous)
    with path.open('ab') as stream:
        stream.write(data)


def supervise(base):
    binary = base / 'vitrallis'
    regular(binary)
    if (base / '.installation-pending').exists():
        raise RuntimeError('Vitrallis installation incomplete; rerun installer')
    if not binary.is_file() or not os.access(binary, os.X_OK):
        raise RuntimeError('Missing executable: ' + str(binary))
    log = base / 'session.log'
    regular(log)
    regular(log.with_suffix('.log.1'))
    awesome(HOME_HOOK)
    child = None
    def stopping(signum, frame):
        if child is not None:
            child.terminate()
    signal.signal(signal.SIGTERM, stopping)
    signal.signal(signal.SIGINT, stopping)
    try:
        child = subprocess.Popen([str(binary), '--pocketchip'], stdin=subprocess.DEVNULL,
                                 stdout=subprocess.PIPE, stderr=subprocess.STDOUT)
        # Drain continuously; app output cannot fill an undrained pipe or grow
        # the log without bound. systemd removes every child on supervisor exit.
        with selectors.DefaultSelector() as selector:
            selector.register(child.stdout, selectors.EVENT_READ)
            while True:
                if selector.select(timeout=0.25):
                    data = os.read(child.stdout.fileno(), 8192)
                    if not data:
                        break
                    log_chunk(log, data)
                if child.poll() is not None:
                    break
        return child.wait()
    finally:
        if child is not None and child.poll() is None:
            child.terminate()
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


def main():
    script = Path(__file__).resolve()
    action = sys.argv[1] if len(sys.argv) == 2 else 'launch' if len(sys.argv) == 1 else ''
    if action == 'launch':
        return subprocess.run(launch_command(script, os.environ), check=False).returncode
    if action == 'run':
        return supervise(script.parent)
    if action == 'restore':
        awesome(RESTORE_HOOK)
        return 0
    if action == 'stop':
        return subprocess.run(['/usr/bin/systemctl', '--user', 'stop', UNIT], check=False).returncode
    raise ValueError('Usage: vitrallis-session.py [launch|stop|run|restore]')


if __name__ == '__main__':
    try:
        sys.exit(main())
    except (OSError, ValueError, RuntimeError, subprocess.SubprocessError) as error:
        print('Vitrallis session: ' + str(error), file=sys.stderr)
        sys.exit(1)
