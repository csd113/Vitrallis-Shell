"""OS/transport boundaries for the literal PocketCHIP entry block; never runs sudo."""
import ast
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
        (self.home / '.config/awesome').mkdir(parents=True)
        (self.home / '.config/awesome/rc.lua').write_text('-- existing desktop startup\n')
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
        self.write('dpkg', '''
if sys.argv[1] == '--print-architecture':
    print('armhf')
elif sys.argv[1] == '--compare-versions':
    assert sys.argv[3] == 'ge'
    sys.exit(0 if tuple(map(int, sys.argv[2].split('.'))) >= tuple(map(int, sys.argv[4].split('.'))) else 1)
''')
        self.write('getconf', 'print("glibc 2.36")')
        self.write('getent', 'print("chip:x:1000:1000:chip:" + os.environ["HOME"] + ":/bin/bash")')
        for tool in ('picom', 'dtc', 'fdtoverlay', 'bwrap'):
            self.write(tool, 'pass')
        self.write('awesome-client', 'print(\'string "vitrallis-desktop-available"\')')
        self.write('awesome', 'print("awesome v4.3")')
        self.write('stat', 'print({"%u:%a": "1000:700", "%u": "1000", "%a": "700", "%h": "1"}[sys.argv[2]])')
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
        # These fixed mocks are invoked dozens of times by one bootstrap block.
        # Avoid importing filesystem/JSON modules for simple responses under QEMU;
        # the existing watchdog and every command/assertion remain unchanged.
        names = {node.id for node in ast.walk(ast.parse(code)) if isinstance(node, ast.Name)}
        # The mocks use only the standard library, so skip site initialization.
        header = '#!' + sys.executable + ' -S\nimport os, sys\n'
        if 'json' in names:
            header += 'import json\n'
        if names & {'root', 'Path'}:
            header += 'from pathlib import Path\nroot = Path(' + repr(str(self.root)) + ')\n'
        path.write_text(header + code + '\n')
        path.chmod(0o755)

    def calls(self):
        return [json.loads(line) for line in self.log.read_text().splitlines()] if self.log.exists() else []
