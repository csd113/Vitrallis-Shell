#!/usr/bin/env python3
"""Local Linux measurements; no hardware connection or timing-based CI threshold."""
import argparse
import json
import os
from pathlib import Path
import signal
import socket
import statistics
import subprocess
import tempfile
import time

APPS = ('vitrallis-terminal', 'vitrallis-notepad', 'vitrallis-files')


def snapshot(process):
    if process.poll() is not None:
        raise RuntimeError('Native application exited during idle measurement')
    root = Path('/proc') / str(process.pid)
    fields = (root / 'stat').read_text().rsplit(')', 1)[1].split()
    cpu = (int(fields[11]) + int(fields[12])) / os.sysconf('SC_CLK_TCK')
    rss = int((root / 'statm').read_text().split()[1]) * os.sysconf('SC_PAGE_SIZE')
    return cpu, rss


def measure(directory, seconds, samples, driver):
    if not Path('/proc/self/stat').exists() or os.geteuid() == 0:
        raise ValueError('Run this local benchmark on Linux as a normal user')
    result = {'method': '480x272 SDL ' + driver + '; warm first-frame-and-exit samples; real idle UI with session inbox and /bin/sh PTY; RSS excludes child shell', 'samples': samples, 'idle_seconds': seconds, 'apps': {}}
    with tempfile.TemporaryDirectory(prefix='vitrallis-measure-') as temp:
        home = Path(temp)
        environment = dict(os.environ, HOME=temp, SHELL='/bin/sh', SDL_VIDEODRIVER=driver)
        environment.pop('VITRALLIS_NATIVE_BROKER', None)
        environment.pop('VITRALLIS_SESSION', None)
        broker = socket.socket(socket.AF_UNIX, socket.SOCK_DGRAM)
        broker.bind(str(home / 'shell'))
        processes = []
        try:
            for name in APPS:
                binary = directory / name
                times = []
                for _ in range(samples):
                    start = time.monotonic()
                    subprocess.run([str(binary), '--size', '480x272', '--smoke-test'],
                                   cwd=home, env=environment, check=True, timeout=10,
                                   stdin=subprocess.DEVNULL, stdout=subprocess.PIPE, stderr=subprocess.PIPE)
                    times.append((time.monotonic() - start) * 1000)
                process = subprocess.Popen([str(binary), '--size', '480x272'], cwd=home,
                                           env=dict(environment, VITRALLIS_NATIVE_BROKER=str(home / 'shell')),
                                           stdin=subprocess.DEVNULL, stdout=subprocess.DEVNULL,
                                           start_new_session=True)
                processes.append((name, process))
                result['apps'][name] = {'first_frame_exit_median_ms': round(statistics.median(times), 2),
                                        'release_bytes': binary.stat().st_size}
            time.sleep(1)
            before = {name: snapshot(process) for name, process in processes}
            start = time.monotonic()
            time.sleep(seconds)
            elapsed = time.monotonic() - start
            for name, process in processes:
                cpu, rss = snapshot(process)
                result['apps'][name].update(idle_cpu_percent=round((cpu - before[name][0]) / elapsed * 100, 3),
                                            rss_mib=round(rss / 1024 / 1024, 2))
        finally:
            for _, process in processes:
                if process.poll() is None:
                    os.killpg(process.pid, signal.SIGTERM)
                try:
                    process.wait(timeout=1)
                except subprocess.TimeoutExpired:
                    # SDL maps SIGTERM to a close event; Terminal correctly asks
                    # for confirmation. End only this owned benchmark group.
                    os.killpg(process.pid, signal.SIGKILL)
                    process.wait(timeout=5)
            broker.close()
    return result


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--bin-dir', type=Path, required=True)
    parser.add_argument('--driver', choices=('dummy', 'x11'), default='dummy')
    parser.add_argument('--seconds', type=int, default=20, choices=range(5, 51), metavar='5..50')
    parser.add_argument('--samples', type=int, default=5, choices=range(1, 11), metavar='1..10')
    args = parser.parse_args()
    print(json.dumps(measure(args.bin_dir.resolve(), args.seconds, args.samples, args.driver), indent=2))


if __name__ == '__main__':
    main()
