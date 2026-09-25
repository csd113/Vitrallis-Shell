"""Real 480x272 desktop shortcut CRUD and launch checks, inside Docker only."""
import json
import os
import shutil
import subprocess
import time
from pathlib import Path

from lifecycle import ARTIFACTS, SIM, Shell, remote_checks, wait_for


class ShortcutShell(Shell):
    def start(self):
        self.env['LD_PRELOAD'] = '/sim/touch.so'
        self.env['VITRALLIS_TEST_TOUCH_FIFO'] = '/sim/shortcut-touch'
        super().start()

    def touch(self, x, y, duration=.06):
        with open('/sim/shortcut-touch', 'wb', buffering=0) as stream:
            stream.write(f'D {x / 480} {y / 272}'.ljust(64).encode())
            time.sleep(duration)
            stream.write(f'U {x / 480} {y / 272}'.ljust(64).encode())
        time.sleep(.2)

    def text(self, value):
        self.xdo('type', '--clearmodifiers', '--delay', 1, value)
        time.sleep(.1)

    def touch_text(self, value):
        keys = 'abcdefghijklmnopqrstuvwxyz0123456789 /._-'
        for character in value:
            index = keys.index(character)
            self.touch(8 + index % 10 * 46 + 22, 72 + index // 10 * 28 + 13)

    def add(self, name, command):
        self.key('F2', 'Return')
        self.text(name)
        self.key('Return', 'Down', 'Return')
        self.text(command)
        self.key('Return')

    def custom(self):
        return [app for app in self.menu() if app['source'] == 'Custom']


def main():
    assert Path('/.dockerenv').exists(), 'Docker simulator only'
    subprocess.run(['cc', '-shared', '-fPIC', 'tests/simulator/touch.c', '-o', '/sim/touch.so', '-lSDL2', '-ldl'], check=True)
    fifo = SIM / 'shortcut-touch'
    fifo.unlink(missing_ok=True)
    os.mkfifo(fifo, 0o666)
    os.chmod(fifo, 0o666)
    wrapper = SIM / 'shortcut-vitrallis'
    wrapper.write_text('#!/bin/sh\nexec setpriv --reuid=65534 --regid=65534 --clear-groups /target/debug/vitrallis "$@"\n')
    wrapper.chmod(0o755)
    home = SIM / ('shortcuts-' + str(time.time_ns()))
    home.mkdir()
    os.chown(home, 65534, 65534)
    target = home / 'my script'
    output = home / 'output'
    work = home / 'working directory'
    work.mkdir()
    os.chown(work, 65534, 65534)
    icon = home / 'chosen.png'
    shutil.copyfile('assets/native/terminal.png', icon)
    target.write_text('#!/bin/sh\nprintf "%s\\n" "$PWD" "$@" > "$HOME/output"\n')
    target.chmod(0o755)
    shell = ShortcutShell(home.name, binary=str(wrapper))
    checks = []
    try:
        preview_marker = home / 'must-not-execute'
        preview_env = dict(shell.env, SDL_VIDEODRIVER='dummy')
        preview_env.pop('LD_PRELOAD', None)
        subprocess.run(['/target/debug/vitrallis-terminal', '--smoke-test', '--command', '/bin/sh', '-c', f'touch {preview_marker}'], env=preview_env, user=65534, group=65534, check=True)
        assert not preview_marker.exists()
        shell.add('Quoted script', '/bin/false')
        shell.click(200, 144)  # Browse executable.
        files = sorted(home.iterdir(), key=lambda p: (not p.is_dir(), str(p)))
        script_index = files.index(target)
        for _ in range(script_index // 5):
            shell.key('Page_Down')
        shell.touch(200, 80 + 32 * (script_index % 5))
        shell.click(200, 112)
        shell.text(" 'two words' '' '$HOME'")
        shell.key('Return')
        shell.shot('shortcut-step-command')
        shell.click(200, 176)
        shell.text(str(work))
        shell.key('Return')
        shell.shot('shortcut-step-cwd')
        shell.click(200, 208)
        shell.shot('shortcut-step-picker')
        files = sorted(home.iterdir(), key=lambda p: (not p.is_dir(), str(p)))
        icon_index = files.index(icon)
        for _ in range(icon_index // 5):
            shell.key('Page_Down')
        shell.touch(200, 80 + 32 * (icon_index % 5))
        shell.shot('shortcut-editor')
        assert not output.exists(), 'Saving/preview must never execute'
        shell.key('shift+Tab', 'Return')  # Save is the last focus target.
        wait_for(lambda: len(shell.custom()) == 1, 'shortcut save')
        first = shell.custom()[0]
        assert first['args'] == ['two words', '', '$HOME']
        assert first['entry'] == str(target)
        assert not output.exists()
        icon.unlink()
        shell.stop()
        shell.start()
        assert shell.custom()[0]['id'] == first['id']
        records = list((home / '.local/share/vitrallis/shortcuts').glob('vitrallis-shortcut-*.json'))
        assert bytes(json.loads(records[0].read_text())['icon']) == Path('assets/native/terminal.png').read_bytes()
        checks.append('Keyboard add, executable browsing, quoting, save does not execute')
        checks.append('Explicit working directory and touch icon picker; copied icon survives original deletion and restart')
        # Four built-ins precede the saved shortcut.
        index = next(i for i, app in enumerate(shell.menu()) if app['id'] == first['id'])
        x, y = 80 + index % 3 * 155, 82 + index // 3 * 104
        shell.touch(x, y, .75)
        shell.shot('shortcut-touch-menu')
        assert not output.exists(), 'Long press must not launch'
        shell.key('Escape')
        shell.xdo('mousemove', '--window', shell.window, x, y, 'click', 3)
        time.sleep(.2)
        shell.shot('shortcut-mouse-menu')
        assert not output.exists(), 'Context click must not launch'
        shell.key('Escape')
        shell.touch(x, y)
        wait_for(output.exists, 'touch launches shebang script')
        assert output.read_text().splitlines() == [str(work), 'two words', '', '$HOME']
        checks.append('Touch tap launches; touch long press and right click open actions without launch')
        shell.stop()
        shell.start()
        assert shell.custom()[0]['id'] == first['id']
        checks.append('Restart retains stable identity and command')
        shell.click(x, y)  # Immediate exit must leave the desktop usable.
        time.sleep(.4)
        shell.key('F10', 'Prior', 'Next', 'Down', 'Return')  # Edit selected custom shortcut.
        shell.key('Return', 'ctrl+a')
        shell.text('Edited script')
        shell.key('Return', 'shift+Tab', 'Return')
        shell.shot('shortcut-edit-result')
        wait_for(lambda: shell.custom()[0]['name'] == 'Edited script', 'edit saved')
        shell.key('F10', 'Prior', 'Next', 'Down', 'Down', 'Return', 'Return')  # Default Cancel.
        assert len(shell.custom()) == 1
        shell.key('F10', 'Prior', 'Next', 'Down', 'Down', 'Return')  # Remove shortcut.
        shell.shot('shortcut-remove-confirmation')
        shell.touch(360, 253)
        wait_for(lambda: not shell.custom(), 'remove shortcut')
        assert target.exists() and output.exists()
        checks.append('Edit, safe cancellation, confirmed touch removal keeps target and data')
        shell.add('Terminal command', '/bin/sh -c \'printf terminal > "$HOME/terminal-output"; sleep 0.2\'')
        shell.key('Page_Down')
        shell.click(200, 112)  # Run in terminal.
        shell.touch(360, 253)  # Save.
        wait_for(lambda: len(shell.custom()) == 1, 'terminal shortcut saved')
        terminal_app = shell.custom()[0]
        assert terminal_app['entry'] == '/target/debug/vitrallis-terminal'
        assert terminal_app['args'][:2] == ['--command', '/bin/sh']
        assert not (home / 'terminal-output').exists()
        shell.click(x, y)
        wait_for((home / 'terminal-output').exists, 'native terminal command executed as user')
        assert (home / 'terminal-output').read_text() == 'terminal'
        result = subprocess.run(['xdotool', 'search', '--onlyvisible', '--name', '^Terminal$'], capture_output=True, text=True, check=True)
        terminal_window = result.stdout.splitlines()[-1]
        subprocess.run(['import', '-window', terminal_window, str(ARTIFACTS / 'shortcut-terminal.png')], check=True)
        # The successful command can close between capture and key delivery.
        # Send keydown only: its dialog may close the window before keyup.
        subprocess.run(['xdotool', 'keydown', '--window', terminal_window, 'Return'], stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)
        shell.xdo('keyup', 'Return')
        time.sleep(.5)
        shell.xdo('windowactivate', '--sync', shell.window)
        checks.append('Native terminal receives structured argv and runs the command as an unprivileged user')
        shell.center()
        shell.refresh()
        shell.install('Carousel', 'mediacarousel', '0.1.0')
        managed = next(app for app in shell.menu() if app['source'] == 'AppCenter')
        app_root = home / '.local/share/vitrallis/apps' / managed['id']
        receipt = app_root / '.vitrallis-receipt.json'
        original_receipt = receipt.read_bytes()
        before_checks = remote_checks()

        def uninstall_menu():
            shell.key('Home')
            index = next(i for i, app in enumerate(shell.menu()) if app['id'] == managed['id'])
            assert index < 6
            shell.xdo('mousemove', '--window', shell.window, 80 + index % 3 * 155, 82 + index // 3 * 104, 'click', 3)
            time.sleep(.2)
            # A managed tile's actions menu ends with "Uninstall app" on the page
            # after the add/folder/move rows.
            shell.key('Prior', 'Next', 'Down', 'Return')
            time.sleep(.5)

        uninstall_menu()
        shell.shot('shortcut-managed-confirmation')
        shell.key('Return')  # App Center's existing Cancel default.
        assert receipt.read_bytes() == original_receipt
        assert remote_checks() == before_checks
        checks.append('Managed desktop action uses App Center confirmation; Cancel preserves receipt without catalog refresh')
        uninstall_menu()
        corrupt = json.loads(original_receipt)
        corrupt['id'] = 'io.simulator.different'
        receipt.write_text(json.dumps(corrupt))
        before_files = {str(p): p.read_bytes() for p in app_root.rglob('*') if p.is_file()}
        shell.key('Right', 'Return')
        time.sleep(.6)
        shell.shot('shortcut-managed-refused')
        assert {str(p): p.read_bytes() for p in app_root.rglob('*') if p.is_file()} == before_files
        assert any(app['id'] == managed['id'] for app in shell.menu())
        receipt.write_bytes(original_receipt)
        checks.append('Receipt identity change after confirmation refuses uninstall and preserves files and tile')
        uninstall_menu()
        shell.key('Right', 'Return')
        wait_for(lambda: not receipt.exists(), 'managed uninstall committed')
        assert not any(app['id'] == managed['id'] for app in shell.menu())
        assert len(shell.custom()) == 1
        assert target.exists() and output.exists()
        assert remote_checks() == before_checks
        checks.append('Managed uninstall succeeds through the existing transaction; custom shortcuts and targets survive')
        shell.key('Home')
        shell.touch(390, 253)  # Actions opens the shared menu.
        shell.touch(120, 80)  # Add shortcut.
        shell.touch(120, 80)
        shell.touch_text('touch')
        shell.touch(422, 253)  # Done.
        shell.touch(120, 112)
        shell.touch_text('/bin/true')
        shell.touch(422, 253)
        shell.touch(360, 253)
        wait_for(lambda: len(shell.custom()) == 2, 'touch-only shortcut creation')
        assert any(app['name'] == 'touch' and app['entry'] == '/bin/true' for app in shell.custom())
        checks.append('Touch-only creation uses the on-screen keyboard, Done and Save')
        print(json.dumps(checks, indent=2), flush=True)
    finally:
        if shell.process and shell.process.poll() is None:
            shell.shot('shortcut-last-frame')
        (ARTIFACTS / 'shortcuts.json').write_text(json.dumps(checks, indent=2))
        shell.stop()


if __name__ == '__main__':
    main()
