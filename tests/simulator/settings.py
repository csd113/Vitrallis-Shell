"""Real 480x272 Settings navigation and preference restart checks in the simulator."""
import json
import time

from lifecycle import ARTIFACTS, Shell, wait_for


def main():
    shell = Shell('settings-' + str(time.time_ns()))
    results = []
    preferences = shell.home / '.config/vitrallis/preferences.json'

    def open_settings():
        shell.key('XF86PowerOff')  # The simulator home has no imported Settings tile.
        return shell.shot('settings-home')

    def clock_page():
        open_settings()
        shell.click(350, 63)

    def saved_clock(ampm):
        return preferences.exists() and json.loads(preferences.read_text())['ampm'] is ampm

    try:
        clock_page()
        shell.click(240, 90)
        wait_for(lambda: saved_clock(True), '12-hour preference saved')
        clock12 = shell.shot('settings-clock-12').crop((12, 44, 468, 135)).tobytes()
        shell.stop()
        shell.start()
        clock_page()
        assert saved_clock(True)
        assert shell.shot('settings-clock-restart').crop((12, 44, 468, 135)).tobytes() == clock12
        shell.click(240, 90)
        wait_for(lambda: saved_clock(False), '24-hour preference saved')
        clock24 = shell.shot('settings-clock-24').crop((12, 44, 468, 135)).tobytes()
        assert clock24 != clock12
        shell.stop()
        shell.start()
        clock_page()
        assert saved_clock(False)
        assert shell.shot('settings-clock-24-restart').crop((12, 44, 468, 135)).tobytes() == clock24
        results.append('12/24-hour choices survive actual Shell restarts and render consistently')
        shell.key('Escape')
        # Compare the page title without the live clock/status row below it.
        home_title = shell.shot('settings-home-return').crop((0, 0, 480, 20)).tobytes()
        for iteration in range(3):
            for x, y in [(100, 63), (350, 63), (100, 108), (350, 108),
                         (100, 153), (350, 153), (100, 198), (350, 198)]:
                shell.click(x, y)
                title = shell.shot(f'settings-category-{iteration}-{x}-{y}').crop((0, 0, 480, 20)).tobytes()
                assert title != home_title, 'Category did not open'
                shell.key('Escape')
                assert shell.shot('settings-category-return').crop((0, 0, 480, 20)).tobytes() == home_title
        results.append('Repeated navigation through all eight categories returns one level on Escape')
        shell.click(350, 198)
        shell.shot('settings-about-version')
        shell.key('Escape')
        shell.click(100, 108)
        wireless_title = shell.shot('settings-wireless').crop((0, 0, 480, 20)).tobytes()
        shell.click(240, 210)
        tor = shell.shot('settings-tor')
        assert tor.crop((0, 0, 480, 20)).tobytes() != wireless_title
        shell.key('Down', 'Down')
        assert shell.shot('settings-tor-back-focus').getpixel((12, 255)) == (112, 215, 255)
        shell.key('Return')
        assert shell.shot('settings-wireless-return').crop((0, 0, 480, 20)).tobytes() == wireless_title
        shell.key('Escape', 'Escape')
        results.append('Visible Tor row opens with a click; arrows reach Back and return to Wireless')
        assert shell.process.poll() is None
    finally:
        shell.stop()
    (ARTIFACTS / 'settings.json').write_text(json.dumps(results, indent=2) + '\n')
    for result in results:
        print('PASS:', result, flush=True)


if __name__ == '__main__':
    main()
