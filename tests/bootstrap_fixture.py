"""OS/transport boundaries for the literal PocketCHIP entry block; never runs sudo."""
import json
import os
from pathlib import Path
import shlex
import socket
import sys


class ShellFixture:
    def __init__(self, root, source):
        self.root = root
        self.commands = root / 'commands'
        self.commands.mkdir()
        self.home = root / 'home'
        self.home.mkdir()
        self.uid = 1000
        self.runtime = root / 'run/user/1000'
        self.runtime.mkdir(parents=True, mode=0o700)
        self.bus = socket.socket(socket.AF_UNIX)
        self.bus.bind(str(self.runtime / 'bus'))
        self.os_release = root / 'os-release'
        self.os_release.write_text('ID=debian\nVERSION_ID="13"\n')
        (root / 'compatible').write_bytes(b'nextthing,pocketchip\0')
        (root / 'boot.scr').write_bytes(b'fixture boot image')
        self.log = root / 'sudo.jsonl'
        self.marker = root / 'packages-installed'
        self.plan = root / 'plan'
        self.plan.write_text('Inst picom (12.5-1 Debian:13/stable [armhf])\nConf picom (12.5-1 Debian:13/stable [armhf])\n')
        self.env = dict(os.environ, HOME=str(self.home), FIXTURE_ROOT=str(root))
        self.script = source.replace('PATH=/usr/sbin:/usr/bin:/sbin:/bin',
                                     'PATH=' + shlex.quote(str(self.commands)) + ':/usr/bin:/bin')
        for original, replacement in [('/etc/os-release', self.os_release),
                                      ('/sys/firmware/devicetree/base/compatible', root / 'compatible'),
                                      ('/boot/boot.scr', root / 'boot.scr'),
                                      ('/run/user/', root / 'run/user/')]:
            self.script = self.script.replace(original, str(replacement) + ('/' if original.endswith('/') else ''))
        self.write('id', 'print("1000")')
        self.write('uname', 'print("Linux" if sys.argv[1] == "-s" else "armv7l")')
        self.write('dpkg', 'print("armhf") if sys.argv[1] == "--print-architecture" else None')
        self.write('getconf', 'print("glibc 2.36")')
        for tool in ('picom', 'dtc', 'fdtoverlay', 'bwrap'):
            self.write(tool, 'pass')
        self.write('awesome-client', 'print(\'string "vitrallis-desktop-available"\')')
        self.write('awesome', 'print("awesome v4.3")')
        self.write('stat', 'print("1000:700" if sys.argv[2] == "%u:%a" else "1000")')
        self.write('systemctl', 'print("inactive")')
        self.write('df', 'print("Filesystem 1024-blocks Used Available Capacity Mounted on\\nfixture 999999 0 999999 0% /")')
        self.write('dpkg-query', '''
package = sys.argv[-1]
if package == 'picom' and not (root / 'packages-installed').exists():
    sys.exit(1)
print('install ok installed', end='')
''')
        self.write('sudo', '''
with (root / 'sudo.jsonl').open('a') as stream:
    stream.write(json.dumps(sys.argv[1:]) + '\\n')
if '-v' in sys.argv:
    sys.exit(0)
if '--simulate' in sys.argv:
    print((root / 'plan').read_text(), end='')
elif 'update' in sys.argv:
    if os.environ.get('FIXTURE_UPDATE_FAIL'):
        sys.exit(100)
else:
    if os.environ.get('FIXTURE_INSTALL_FAIL'):
        sys.exit(100)
    (root / 'packages-installed').touch()
''')
        self.write('curl', '''
output = Path(sys.argv[sys.argv.index('-o') + 1])
(root / 'download-path').write_text(str(output))
output.write_text('raise SystemExit("fixture only")')
if os.environ.get('FIXTURE_DOWNLOAD_FAIL'):
    sys.exit(22)
''')
        self.write('python3', '''
if '-c' not in sys.argv:
    (root / 'bootstrap-executed').touch()
''')

    def write(self, name, code):
        path = self.commands / name
        path.write_text('#!' + sys.executable + '\nimport json, os, sys\nfrom pathlib import Path\n'
                        + 'root = Path(' + repr(str(self.root)) + ')\n' + code + '\n')
        path.chmod(0o755)

    def calls(self):
        return [json.loads(line) for line in self.log.read_text().splitlines()] if self.log.exists() else []
