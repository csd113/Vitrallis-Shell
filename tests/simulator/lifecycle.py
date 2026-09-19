"""End-to-end checks against a real SDL shell and HTTPS repository service.

No installer or UI state is mocked. Assertions combine on-screen pixels, live
menu activation, verified receipts, actual launch output and HTTP request counts.
"""
import json
import os
import shutil
import subprocess
import time
from pathlib import Path

from PIL import Image

SIM = Path('/sim')
ARTIFACTS = SIM / 'artifacts'
REPORT = []


def wait_for(predicate, description, timeout=15):
    until = time.monotonic() + timeout
    while time.monotonic() < until:
        if predicate():
            return
        time.sleep(.05)
    raise AssertionError('Timeout: ' + description)


def state(**values):
    path = SIM / 'state.json'
    path.with_suffix('.tmp').write_text(json.dumps(values))
    path.with_suffix('.tmp').replace(path)


def requests():
    path = SIM / 'requests.jsonl'
    return [json.loads(line) for line in path.read_text().splitlines()] if path.exists() else []


def remote_checks():
    return sum(url.endswith('/apps.json') for url in requests())


def passed(name):
    REPORT.append(name)
    print('PASS:', name, flush=True)
    (ARTIFACTS / 'scenarios.json').write_text(json.dumps(REPORT, indent=2))


class Shell:
    def __init__(self, home, config=None, binary='/target/debug/vitrallis'):
        self.home = SIM / home
        self.home.mkdir(exist_ok=True)
        self.env = dict(os.environ, HOME=str(self.home),
                        XDG_DATA_HOME=str(self.home / '.local/share'),
                        XDG_CONFIG_HOME=str(self.home / '.config'),
                        HTTP_PROXY='', HTTPS_PROXY='', ALL_PROXY='')
        self.config = config
        self.binary = binary
        self.process = None
        self.start()

    def start(self):
        command = [self.binary, '--size', '480x272']
        if self.config:
            command += ['--app-config', str(self.config)]
        self.log = (ARTIFACTS / (self.home.name + '.log')).open('a')
        self.process = subprocess.Popen(command, env=self.env, stdout=self.log, stderr=self.log)
        found = []

        def window():
            result = subprocess.run(['xdotool', 'search', '--all', '--onlyvisible', '--pid', str(self.process.pid), '--name', 'Vitrallis'],
                                    capture_output=True, text=True)
            found[:] = result.stdout.splitlines()
            return bool(found)

        wait_for(window, 'shell window')
        self.window = found[-1]
        self.xdo('windowactivate', '--sync', self.window)
        time.sleep(.5)

    def stop(self):
        if self.process:
            self.process.terminate()
            try:
                self.process.wait(timeout=3)
            except subprocess.TimeoutExpired:
                self.process.kill()
                self.process.wait(timeout=3)
            self.log.close()
            self.process = None

    @staticmethod
    def xdo(*args):
        subprocess.run(['xdotool', *map(str, args)], check=True, stdout=subprocess.DEVNULL)

    def click(self, x, y):
        self.xdo('mousemove', '--window', self.window, x, y, 'click', 1)
        time.sleep(.12)

    def key(self, *keys):
        self.xdo('key', '--window', self.window, *keys)
        time.sleep(.16)

    def shot(self, name):
        path = ARTIFACTS / (name + '.png')
        subprocess.run(['import', '-window', self.window, str(path)], check=True)
        return Image.open(path).convert('RGB')

    def center(self):
        self.click(80, 183)
        time.sleep(.35)

    def refresh(self):
        before = remote_checks()
        self.key('c')
        wait_for(lambda: remote_checks() > before, 'repository refresh')
        time.sleep(.5)

    def select(self, name):
        title = self.shot("search-list-title").crop((0, 0, 480, 30)).tobytes()
        self.click(170, 78)
        self.click(180, 45)  # Clear search text
        self.xdo('type', '--window', self.window, '--clearmodifiers', name)
        self.key("Left", "Return")  # Clear retains focus; Search is previous.
        wait_for(lambda: self.shot("search-applied").crop((0, 0, 480, 30)).tobytes() == title, "search applied to list")
        image = self.shot('selection')
        if image.getpixel((9, 106)) != (93, 218, 201):
            self.click(220, 120)

    def root(self, slug):
        return self.home / '.local/share/vitrallis/apps' / ('io.vitrallis.' + slug)

    def version(self, slug):
        root = self.root(slug)
        receipt = root / '.vitrallis-receipt.json'
        if not receipt.exists() or (root / '.installation-pending').exists():
            return None
        return json.loads(receipt.read_text())['version']

    def install(self, name, slug, version):
        self.select(name)
        before = remote_checks()
        header = self.shot('before-' + slug).crop((8, 66, 310, 92)).tobytes()
        self.key('i')
        wait_for(lambda: self.version(slug) == version, name + ' installation')
        time.sleep(.25)
        after = self.shot('installed-' + slug)
        assert after.getpixel((20, 120)) != (14, 24, 34), 'Catalog disappeared'
        assert header == after.crop((8, 66, 310, 92)).tobytes(), 'Search changed'
        assert remote_checks() == before, 'Mutation fetched remote catalog'

    def menu(self):
        command = [self.binary, '--list-apps']
        if self.config:
            command += ['--app-config', str(self.config)]
        result = subprocess.run(command, env=self.env, capture_output=True, text=True, check=True)
        return json.loads(result.stdout)['apps']

    def launch_menu(self, slug, version):
        launch = SIM / ('launched-' + slug)
        launch.unlink(missing_ok=True)
        self.key('Home')
        apps = self.menu()
        index = next(i for i, app in enumerate(apps) if app['id'] == 'io.vitrallis.' + slug)
        assert index < 6, 'Fixture expected on first menu page'
        self.shot('menu-' + slug)
        self.click(80 + index % 3 * 155, 82 + index // 3 * 104)
        wait_for(launch.exists, 'live menu launches ' + slug)
        output = launch.read_text().splitlines()
        assert output[0] == version, output
        assert Path(output[1]).parent == self.root(slug), output
        time.sleep(.6)
        self.center()

    def remove(self, name, slug):
        self.select(name)
        self.click(240, 230)  # Details
        self.click(240, 230)  # Remove
        self.shot('remove-confirmation')
        self.key('Return')  # Safe default is Cancel
        assert self.version(slug) is not None
        self.click(240, 230)
        self.click(360, 242)  # Confirm
        wait_for(lambda: self.version(slug) is None, 'removal')
        time.sleep(.3)
        self.click(400, 45)  # Back
        self.shot('removed-' + slug)
        assert all(app['id'] != 'io.vitrallis.' + slug for app in self.menu())


def main():
    ARTIFACTS.mkdir(exist_ok=True)
    state()
    run_id = str(time.time_ns())
    shell = Shell('lifecycle-' + run_id)
    try:
        shell.shot('fresh-home')
        shell.center()
        shell.shot('fresh-center')
        shell.refresh()
        shell.shot('catalog')
        passed('Fresh shell/App Center and repository load')
        shell.install('Debug', 'debug', '0.1.0')
        passed('Install Debug; catalog, selection and search survive with no remote recheck')
        shell.install('Carousel', 'mediacarousel', '0.1.0')
        passed('Install Carousel after Debug without manual recheck')
        shell.launch_menu('mediacarousel', '0.1.0')
        passed('Carousel immediately appears in live Apps menu and launches canonical files')
        shell.launch_menu('debug', '0.1.0')
        passed('Older Debug actually executes version 0.1.0')
        state(debug='0.2.0')
        shell.refresh()
        shell.select('Debug')
        shell.shot('update-available')
        shell.click(400, 78)
        shell.click(400, 78)
        update_filter = shell.shot('updates-filter').crop((350, 68, 408, 90)).tobytes()
        shell.click(240, 230)
        shell.shot('details-update')
        before = len(requests())
        shell.click(240, 45)
        shell.shot('changelog')
        shell.key('Next')
        shell.shot('changelog-history')
        assert len(requests()) == before, 'Opening release notes fetched network data'
        shell.click(240, 45)  # Back to details
        shell.click(400, 45)  # Back to apps
        passed('Available-version changelog and history display from verified cache')
        state(debug='0.2.0', corrupt_payload='start.py')
        shell.key('i')
        time.sleep(1)
        assert shell.version('debug') == '0.1.0'
        shell.shot('failed-update')
        shell.launch_menu('debug', '0.1.0')
        passed('Corrupt update preserves previous working version and catalog')
        state(debug='0.2.0')
        shell.select('Debug')
        launch = shell.home / '.local/share/vitrallis/app-center/launchers/io.vitrallis.debug'
        held = subprocess.Popen([str(launch)], env=dict(shell.env, VITRALLIS_SIM_HOLD='debug'), stdout=subprocess.DEVNULL)
        try:
            time.sleep(.3)
            shell.key('i')
            time.sleep(.4)
            shell.shot('running-confirmation')
            shell.key('Return')
            time.sleep(.2)
            assert held.poll() is None, 'Cancel terminated running app'
            assert shell.version('debug') == '0.1.0'
            shell.key('i')
            time.sleep(.4)
            shell.click(360, 242)
            wait_for(lambda: shell.version('debug') == '0.2.0', 'confirmed running-app update')
            held.wait(timeout=10)
        finally:
            if held.poll() is None:
                held.terminate()
                held.wait(timeout=10)
        passed('Running old entry: Cancel keeps process; Close and update stops it and commits new version')
        wait_for(lambda: shell.shot('updated-filter-preserved').getpixel((20, 120)) == (14, 24, 34), 'updated UI filter')
        filtered = shell.shot('updated-filter-preserved')
        assert filtered.crop((350, 68, 408, 90)).tobytes() == update_filter
        assert filtered.getpixel((20, 120)) == (14, 24, 34)
        assert (239, 241, 245) not in set(filtered.crop((126, 34, 233, 56)).getdata()), 'hidden selection must not expose an active app action'
        passed('Active Updates filter survives mutation and explains zero matches')
        assert not (shell.root('debug') / 'obsolete.py').exists()
        shell.launch_menu('debug', '0.2.0')
        shell.click(400, 78)  # Updates -> All
        passed('Update changes installed metadata, removes obsolete files and launches new entry/version')
        shell.remove('Carousel', 'mediacarousel')
        passed('Removal updates App Center and menu; confirmation defaults to Cancel')
        shell.install('Plain', 'plain', '0.1.0')
        passed('Install A, install B, update A, remove B, install C sequence stays usable')
        script = shell.root('plain') / 'main.py'
        saved = script.read_bytes()
        unrelated = (shell.root('debug') / 'start.py').read_bytes()
        script.unlink()
        script.symlink_to(shell.root('debug') / 'start.py')
        shell.select('Plain')
        shell.click(240, 230)
        shell.click(240, 230)
        shell.click(360, 242)
        time.sleep(.5)
        shell.shot('failed-removal')
        assert shell.version('plain') == '0.1.0'
        assert (shell.root('debug') / 'start.py').read_bytes() == unrelated
        script.unlink()
        script.write_bytes(saved)
        shell.click(400, 45)
        passed('Failed removal rejects unsafe files and preserves unrelated installation')
        shell.select('Plain')
        shell.click(240, 230)
        shell.click(240, 45)
        shell.shot('missing-changelog')
        shell.click(240, 45)
        shell.click(400, 45)
        shell.select('Malformed')
        shell.click(240, 230)
        shell.click(240, 45)
        shell.shot('malformed-changelog')
        shell.click(240, 45)
        shell.click(400, 45)
        passed('Missing and malformed changelogs leave readable Details and catalog')
        state(debug='0.2.0', fail_payload='main.py')
        shell.key('i')
        time.sleep(1)
        assert shell.version('malformed') is None
        shell.shot('failed-install')
        passed('Download failure leaves other apps and valid catalog intact')
        state(debug='0.2.0', offline=True)
        before = len(requests())
        shell.key('c')
        wait_for(lambda: len(requests()) > before, 'offline refresh attempted')
        time.sleep(.5)
        shell.shot('offline-catalog')
        shell.select('Debug')
        assert shell.shot('offline-debug').getpixel((20, 120)) != (14, 24, 34)
        passed('Failed repository refresh retains known-good entries')
        before = len(requests())
        shell.key('Home')
        shell.center()
        assert len(requests()) == before
        shell.stop()
        shell.start()
        shell.center()
        assert len(requests()) == before
        shell.shot('restarted-offline')
        assert shell.version('debug') == '0.2.0'
        passed('Reopening and restarting reconstruct local state/cache without remote requests')
        state(debug='0.2.0', bad_app=True)
        shell.refresh()
        shell.select('Debug')
        shell.shot('one-bad-app')
        assert shell.shot('good-app-retained').getpixel((20, 120)) != (14, 24, 34)
        passed('One malformed app does not disable valid repository apps')
        shell.click(300, 45)  # Repositories
        shell.click(60, 45)  # Add
        shell.xdo('type', '--window', shell.window, '--clearmodifiers', 'broken/catalog')
        shell.click(48, 45)  # Save
        time.sleep(.3)
        shell.click(420, 45)  # Back
        shell.refresh()
        shell.select('Debug')
        assert shell.shot('multiple-repositories').getpixel((20, 120)) != (14, 24, 34)
        shell.click(300, 45)
        shell.shot('repositories-with-error')
        shell.click(420, 45)
        passed('Multiple configured repositories: one unavailable source preserves usable apps')
    finally:
        shell.stop()
    state()
    config = SIM / ('menu-' + run_id + '.json')
    config.write_text(json.dumps({'pages': [{'name': 'Apps', 'items': []}]}))
    shell = Shell('menu-' + run_id, config)
    try:
        shell.center()
        shell.refresh()
        config.write_text('{broken device menu')
        shell.install('Carousel', 'mediacarousel', '0.1.0')
        shell.launch_menu('mediacarousel', '0.1.0')
        passed('Carousel appears and launches immediately even when unrelated device-menu refresh fails')
    finally:
        shell.stop()


if __name__ == '__main__':
    main()
