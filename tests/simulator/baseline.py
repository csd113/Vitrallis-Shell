"""Reproduce catalog, menu-registration and stale-entry failures on the original binary."""
import json
import subprocess
import time
from pathlib import Path

from lifecycle import ARTIFACTS, SIM, Shell, passed, state, wait_for


def main():
    state()
    run_id = str(time.time_ns())
    config = SIM / ('baseline-menu-' + run_id + '.json')
    config.write_text(json.dumps({'pages': [{'name': 'Apps', 'items': []}]}))
    shell = Shell('baseline-menu-' + run_id, config, '/target/vitrallis-baseline')
    try:
        shell.center()
        shell.refresh()
        config.write_text('{broken device menu')
        shell.click(200, 122)  # Original layout: Carousel is the second row.
        shell.key('i')
        wait_for(lambda: shell.version('mediacarousel') == '0.1.0', 'baseline Carousel install')
        time.sleep(.3)
        image = shell.shot('baseline-catalog-wipe')
        assert image.getpixel((20, 120)) == (19, 28, 39)
        shell.key('Home')
        shell.shot('baseline-missing-carousel')
        launch = Path('/sim/launched-mediacarousel')
        launch.unlink(missing_ok=True)
        shell.click(240, 185)  # Expected new fifth menu tile.
        time.sleep(.5)
        assert not launch.exists(), 'Baseline unexpectedly refreshed the live menu'
        assert 'reload_failed' in Path(shell.log.name).read_text()
        passed('Baseline reproduces catalog wipe and missing live Carousel menu after device-menu read failure')
    finally:
        shell.stop()

    state()
    shell = Shell('baseline-update-' + run_id, binary='/target/vitrallis-baseline')
    try:
        shell.center()
        shell.refresh()
        shell.click(200, 85)  # Original layout: Debug is the first row.
        shell.key('i')
        wait_for(lambda: shell.version('debug') == '0.1.0', 'baseline Debug install')
        time.sleep(.5)  # Receipt write precedes the original worker's Done message.
        state(debug='0.2.0')
        shell.refresh()
        shell.click(200, 85)
        shell.key('i')
        wait_for(lambda: shell.version('debug') == '0.2.0', 'baseline Debug update')
        launcher = shell.home / '.local/share/vitrallis/app-center/launchers/io.vitrallis.debug'
        result = subprocess.run([str(launcher)], env=shell.env, capture_output=True, text=True, check=True)
        assert result.stdout.strip() == 'Debug OLD ENTRY 0.2.0'
        assert (shell.root('debug') / 'obsolete.py').exists()
        assert 'main.py' in launcher.read_text()
        assert 'start.py' in (shell.root('debug') / 'app.toml').read_text()
        (ARTIFACTS / 'baseline-stale-entry.txt').write_text(
            launcher.read_text() + '\nObserved output: ' + result.stdout)
        shell.shot('baseline-stale-update')
        passed('Baseline Debug reports updated metadata but retains the old launcher entry and obsolete files')
    finally:
        shell.stop()


if __name__ == '__main__':
    main()
