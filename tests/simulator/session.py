"""Real Awesome/X11 Home routing and native launch/return in a disposable container.

Run under dbus-run-session. The supervisor runs directly because the container
has no systemd user manager; unit ownership/stop commands have separate fixtures.
"""
import importlib.util
import json
import os
from pathlib import Path
import shutil
import subprocess
import time

from lifecycle import wait_for

ROOT = Path('/workspace')
OUT = Path('/sim/artifacts')
HOME = Path('/sim/stock-session-home')
BASE = HOME / '.local/share/vitrallis'


def run(*args, **kwargs):
    result = subprocess.run(args, capture_output=True, text=True, **kwargs)
    if result.returncode:
        raise RuntimeError(f'{args}: {result.stderr}')
    return result.stdout.strip()


def windows(name):
    result = subprocess.run(['xdotool', 'search', '--onlyvisible', '--name', '^' + name + '$'],
                            capture_output=True, text=True)
    return result.stdout.splitlines()


def focus(window):
    run('xdotool', 'windowactivate', '--sync', window)


def active():
    return run('xdotool', 'getactivewindow')


def key(*keys):
    run('xdotool', 'key', '--clearmodifiers', *keys)
    time.sleep(.15)


def main():
    assert Path('/.dockerenv').is_file()
    HOME.mkdir()
    os.environ.update(HOME=str(HOME), XDG_DATA_HOME=str(HOME / '.local/share'),
                      XDG_CONFIG_HOME=str(HOME / '.config'), DISPLAY=':100',
                      VITRALLIS_SESSION='1', SDL_VIDEODRIVER='x11')
    config = Path('/usr/share/pocket-home/config.json')
    config.parent.mkdir(parents=True, exist_ok=True)
    shutil.copyfile(ROOT / 'tests/fixtures/pockethome/stock.json', config)
    before = config.read_bytes()
    generation = BASE / 'generations' / ('a' * 64)
    generation.mkdir(parents=True)
    for name in ('vitrallis', 'vitrallis-terminal', 'vitrallis-notepad', 'vitrallis-files'):
        shutil.copyfile(Path('/target/debug') / name, generation / name)
        (generation / name).chmod(0o755)
    (BASE / 'current').symlink_to('generations/' + 'a' * 64)
    helper = BASE / 'vitrallis-session.py'
    shutil.copyfile(ROOT / 'integrations/pocketchip/vitrallis-session.py', helper)
    spec = importlib.util.spec_from_file_location('session', helper)
    session = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(session)
    children = []
    logs = []
    def spawn(name, *args):
        log = (OUT / (name + '.log')).open('w')
        logs.append(log)
        child = subprocess.Popen(args, stdout=log, stderr=subprocess.STDOUT)
        children.append(child)
        return child
    try:
        spawn('session-xvfb', 'Xvfb', ':100', '-screen', '0', '480x272x24', '-nolisten', 'tcp')
        wait_for(lambda: Path('/tmp/.X11-unix/X100').exists(), 'session X server')
        spawn('session-awesome', 'awesome')
        time.sleep(1)
        spawn('original-session', 'xterm', '-title', 'Original session')
        wait_for(lambda: windows('Original session'), 'original session window')
        original = windows('Original session')[0]
        focus(original)
        # No modified-launcher globals or files. Record a real existing Home binding.
        session.awesome('''
assert(focus_home_screen == nil and launch_home_screen == nil)
stock_home_count = 0
local keys = root.keys()
for _, k in ipairs(require('awful').key({}, 'XF86PowerOff', function()
    stock_home_count = stock_home_count + 1
end)) do table.insert(keys, k) end
root.keys(keys)
stock_original_keys = root.keys()
return "stock key installed"
''')
        supervisor = spawn('supervised-native', '/usr/bin/python3', str(helper), 'run')
        wait_for(lambda: windows('Vitrallis'), 'supervised shell')
        wait_for(lambda: (BASE / 'session.log').exists() and 'event=ready' in (BASE / 'session.log').read_text(), 'supervised boot handoff')
        shell = windows('Vitrallis')[-1]
        (OUT / 'session-windows.txt').write_text(run('xwininfo', '-root', '-tree'))
        run('import', '-window', 'root', str(OUT / 'session-before.png'))
        session.awesome('''
assert(vitrallis_home_route ~= nil)
local keys = root.keys()
stock_extra_keys = require('awful').key({}, 'F12', function() end)
for _, k in ipairs(stock_extra_keys) do table.insert(keys, k) end
root.keys(keys)
return "concurrent key added"
''')
        # Actual menu is derived from the unchanged stock source; native IDs occur once.
        menu = json.loads(run(str(generation / 'vitrallis'), '--linux-handheld', '--list-apps'))
        for app in ('terminal', 'notepad', 'files'):
            assert sum(a['id'] == 'io.vitrallis.' + app for a in menu['apps']) == 1
        assert not any(a['source'] == 'PocketHome' and a['name'] in ('Terminal', 'Write', 'Browse Files') for a in menu['apps'])
        assert any(a['name'] == 'Play PICO-8' for a in menu['apps'])
        for index, name in enumerate(('Terminal', 'Notepad', 'Files')):
            focus(shell)
            time.sleep(.3)
            run('xdotool', 'mousemove', '--window', shell, str(80 + index * 155), '82', 'click', '1')
            time.sleep(.5)
            run('import', '-window', 'root', str(OUT / ('session-' + name + '-launch.png')))
            wait_for(lambda: windows(name), name + ' native launch')
            native = windows(name)[-1]
            focus(native)
            key('XF86PowerOff')
            wait_for(lambda: active() == shell, name + ' Home return')
            assert windows(name), 'Home must leave the native app running'
            # Re-select the same tile: resume the existing window, do not duplicate it.
            run('xdotool', 'mousemove', '--window', shell, str(80 + index * 155), '82', 'click', '1')
            wait_for(lambda: active() == native, name + ' resume')
            assert windows(name) == [native]
            if name == 'Notepad':
                run('xdotool', 'type', '--clearmodifiers', 'unsaved close test')
            session.awesome(f'for _, c in ipairs(client.get()) do if c.window == {int(native)} then c:kill() end end; return \"close requested\"')
            if name == 'Notepad':
                # A real WM close must leave the unsaved confirmation open.
                # SDL's default synthetic Quit used to cancel it immediately.
                time.sleep(.6)
                key('Return')  # Default Cancel keeps the document/window.
                assert windows(name) == [native]
                session.awesome(f'for _, c in ipairs(client.get()) do if c.window == {int(native)} then c:kill() end end; return \"close requested\"')
                time.sleep(.6)
                key('Right', 'Right', 'Return')  # Explicit Discard closes it.
            wait_for(lambda: not windows(name), name + ' clean close')
            # A crashed accelerated client must be reaped and launchable again.
            focus(shell)
            run('xdotool', 'mousemove', '--window', shell, str(80 + index * 155), '82', 'click', '1')
            wait_for(lambda: windows(name), name + ' relaunch before crash')
            crashed = windows(name)[-1]
            time.sleep(.6)  # Crash a running client, after startup activation completes.
            os.kill(int(run('xdotool', 'getwindowpid', crashed)), 9)
            wait_for(lambda: not windows(name), name + ' crash cleanup')
            time.sleep(.6)  # Allow the shell's existing bounded reap interval.
            focus(shell)
            run('xdotool', 'mousemove', '--window', shell, str(80 + index * 155), '82', 'click', '1')
            wait_for(lambda: windows(name), name + ' recovery launch')
            recovered = windows(name)[-1]
            session.awesome(f'for _, c in ipairs(client.get()) do if c.window == {int(recovered)} then c:kill() end end; return "close requested"')
            wait_for(lambda: not windows(name), name + ' recovery close')
        focus(shell)
        run('import', '-window', shell, str(OUT / 'stock-session-menu.png'))
        session.awesome(f'for _, c in ipairs(client.get()) do if c.window == {int(shell)} then c:kill() end end; return \"close requested\"')
        assert supervisor.wait(timeout=10) == 0
        if os.environ.get('VITRALLIS_TEST_ACCELERATED') == '1':
            diagnostics = (BASE / 'session.log').read_text().splitlines()
            shutil.copyfile(BASE / 'session.log', OUT / 'native-renderers.log')
            for name in ('Terminal', 'Notepad', 'Files'):
                selected = [line for line in diagnostics if 'event=renderer_initialized' in line and f'app="{name}"' in line]
                assert len(selected) >= 3, (name, diagnostics)
                assert all('requested=auto mode=hardware' in line and 'fallback=false' in line for line in selected)

        wait_for(lambda: active() == original, 'original window restored')
        session.awesome('''
assert(vitrallis_home_route == nil)
local keys = root.keys()
assert(#keys == #stock_original_keys + #stock_extra_keys)
for _, old in ipairs(stock_original_keys) do
    local found = false
    for _, k in ipairs(keys) do if k == old then found = true end end
    assert(found)
end
return "keys restored"
''')
        session.awesome(session.RESTORE_HOOK)  # ExecStopPost may repeat restoration.
        key('XF86PowerOff')
        session.awesome('assert(stock_home_count == 1); return "original Home works"')
        assert config.read_bytes() == before
        assert not (HOME / '.pocket-home').exists()
        # Missing native utility remains a repair tile, with no imported replacement.
        (generation / 'vitrallis-files').unlink()
        missing = json.loads(run(str(generation / 'vitrallis'), '--linux-handheld', '--list-apps'))
        files = [a for a in missing['apps'] if a['id'] == 'io.vitrallis.files']
        assert len(files) == 1 and 'Repair' in files[0]['unavailable']
        assert not any(a['name'] == 'Browse Files' for a in missing['apps'])
        (OUT / 'stock-session.json').write_text(json.dumps({
            'native_crash_recovery': ['Terminal', 'Notepad', 'Files'],
            'native_launch_home_resume_close': ['Terminal', 'Notepad', 'Files'],
            'original_focus_and_keys_restored': True,
            'concurrent_binding_preserved': True,
            'stock_config_unchanged': True,
            'missing_native_repair': True,
            'systemd_unit': 'separate command/ownership fixtures; no user manager in container'
        }, indent=2))
        print('PASS: stock discovery, real Awesome Home, native launch/resume/close, session restoration', flush=True)
    finally:
        for child in reversed(children):
            if child.poll() is None:
                child.terminate()
                try:
                    child.wait(timeout=5)
                except subprocess.TimeoutExpired:
                    child.kill()
                    child.wait(timeout=5)
        for log in logs:
            log.close()


if __name__ == '__main__':
    main()
