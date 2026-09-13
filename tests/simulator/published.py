"""Optional integration pass using the verified Vitrallis-Apps Git checkout.

Clone the pinned publisher checkout as documented before running this script.
App Center provisions the declared dependencies during installation.
"""
import json
import os
import subprocess
import time
from pathlib import Path

from lifecycle import ARTIFACTS, SIM, Shell, passed, state, wait_for


def app_window(name):
    result = subprocess.run(['xdotool', 'search', '--onlyvisible', '--name', '^' + name + '$'], capture_output=True, text=True)
    return result.stdout.splitlines()[-1] if result.stdout.strip() else None


def open_app(shell, name):
    shell.click(180, 45)  # Open primary action, using the live menu entry.
    wait_for(lambda: app_window(name), name + ' real application window', timeout=20)
    window = app_window(name)
    subprocess.run(['import', '-window', window, str(ARTIFACTS / (name.replace(' ', '-') + '.png'))], check=True)
    result = subprocess.check_output(['xprop', '-id', window, '_NET_WM_PID'], text=True)
    pid = int(result.rsplit(' ', 1)[-1])
    cmdline = Path('/proc', str(pid), 'cmdline').read_bytes().split(b'\0')
    assert any(str(shell.home).encode() in arg and arg.endswith(b'/main.py') for arg in cmdline), cmdline
    shell.xdo('windowactivate', '--sync', window)
    shell.xdo('key', 'Escape')
    wait_for(lambda: not app_window(name), 'application closes')
    time.sleep(.6)
    shell.center()
    return pid


def main():
    state(published=True, debug='0.1.1')
    shell = Shell('published-' + str(time.time_ns()))
    try:
        shell.center()
        shell.refresh()
        shell.install('Vitrallis Media Carousel', 'mediacarousel', '0.1.2')
        environments = list((shell.root('mediacarousel') / 'runtime').glob('*/bin/python3'))
        assert len(environments) == 1, environments
        with (ARTIFACTS / 'published-runtime.txt').open('w') as output:
            subprocess.run([str(environments[0]), '-m', 'pip', 'freeze'], check=True, stdout=output)
        shell.shot('published-carousel-installed')
        open_app(shell, 'Vitrallis Media Carousel')
        passed('Published Carousel 0.1.2: pinned source download, install, immediate Open and real Tk window')
        shell.install('Vitrallis Debug', 'debug', '0.1.1')
        old_pid = open_app(shell, 'Vitrallis Debug')
        state(published=True, debug='0.1.2')
        shell.refresh()
        shell.select('Vitrallis Debug')
        shell.click(240, 230)
        shell.click(240, 45)
        shell.shot('published-debug-whats-new')
        shell.click(240, 45)
        shell.click(400, 45)
        shell.install('Vitrallis Debug', 'debug', '0.1.2')
        new_pid = open_app(shell, 'Vitrallis Debug')
        assert old_pid != new_pid
        manifest = (shell.root('debug') / 'app.toml').read_text()
        assert 'version = "0.1.2"' in manifest
        # This published patch adds release notes; upstream explicitly declares
        # unchanged runtime behavior. The synthetic regression proves code change.
        passed('Published Debug 0.1.1 -> 0.1.2: release notes, verified files/metadata and fresh real process')
        shell.remove('Vitrallis Media Carousel', 'mediacarousel')
        passed('Published Carousel removal updates App Center and Apps menu')
    finally:
        shell.stop()


if __name__ == '__main__':
    main()
