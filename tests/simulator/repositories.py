"""Deterministic HTTPS GitHub fixtures; never imported by production code.

The simulator trusts its private test certificate inside the disposable container.
The shell still uses its real curl transport, URL policy, parser and installer.
"""
import hashlib
import io
import json
import ssl
import subprocess
from functools import lru_cache
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer
from pathlib import Path

from PIL import Image

STATE = Path('/sim/state.json')
REQUESTS = Path('/sim/requests.jsonl')
ORIGIN = 'csd113/vitrallis-apps'


def encoded(value):
    return json.dumps(value).encode()


def package(slug, name, version, entry='main.py'):
    identity = 'io.vitrallis.' + slug
    manifest = f'''manifest_version = 1
name = "{name}"
id = "{identity}"
version = "{version}"
runtime = "python"
entry = "{entry}"
[permissions]
network = false
audio = false
storage = false
'''.encode()
    icon = io.BytesIO()
    Image.new('RGB', (32, 32), (32, 164, 142)).save(icon, format='PNG')
    files = {
        'app.toml': manifest,
        'main.py': f"print('{name} OLD ENTRY {version}')\n".encode(),
        'icon.png': icon.getvalue(),
        'README.md': f'# {name}\nSimulator lifecycle fixture.\n'.encode(),
        'requirements.txt': b'# standard library only\n',
        'assets/readme.txt': b'fixture asset\n',
        'release.py': f"VERSION = '{version}'\n".encode(),
    }
    files[entry] = f'''from pathlib import Path
import os, time
from release import VERSION
print('{name} ' + VERSION, flush=True)
Path('/sim/launched-{slug}').write_text(VERSION + '\\n' + __file__)
if os.environ.get('VITRALLIS_SIM_HOLD') == '{slug}': time.sleep(120)
'''.encode()
    if version == '0.1.0':
        files['obsolete.py'] = b"print('obsolete')\n"
    if slug != 'plain':
        files['CHANGELOG.md'] = (
            f'# Changelog\n\n## {version} — 2026-09-12\n\n'
            '- Launch the new release.\n- Keep your library intact.\n\n'
            '## 0.0.1 — 2026-09-01\n\n- First release.\n'
        ).encode()
    if slug == 'malformed':
        files['CHANGELOG.md'] = b'\xff\x00bad changelog'
    commit = hashlib.sha1(encoded({k: hashlib.sha256(v).hexdigest() for k, v in files.items()})).hexdigest()
    p = dict(id=identity, name=name, version=version, runtime='python', entry=entry,
             permissions=dict(network=False, audio=False, storage=False), installable=True,
             description=f'{name} simulator application.', compatibility_notes='Python 3; offline fixture.',
             source=dict(repository=ORIGIN, commit=commit, path=f'apps/{slug}'),
             files=[dict(path=k, size=len(v), sha256=hashlib.sha256(v).hexdigest())
                    for k, v in sorted(files.items())])
    return p, files


PACKAGES = [package('debug', 'Debug', v, 'start.py' if v == '0.2.0' else 'main.py')
            for v in ['0.1.0', '0.1.1', '0.2.0']]
PACKAGES += [package(s, n, '0.1.0') for s, n in [
    ('mediacarousel', 'Carousel'), ('plain', 'Plain'), ('malformed', 'Malformed')]]


@lru_cache(maxsize=1)
def published():
    repo = Path('/workspace/target/app-center-audit/published-repo')
    document = json.loads((repo / 'apps.json').read_text())
    old = json.loads(subprocess.check_output(['git', '-C', str(repo), 'show', 'e1a4561:apps.json']))
    entries = document['apps'] + [p for p in old['apps'] if p['id'] == 'io.vitrallis.debug']
    return [(p, {f['path']: subprocess.check_output([
        'git', '--no-replace-objects', '-C', str(repo), 'show',
        p['source']['commit'] + ':' + p['source']['path'] + '/' + f['path']
    ]) for f in p['files']}) for p in entries]


def routes(state):
    responses = {}
    packages = published() if state.get('published') else PACKAGES
    visible = [p for p, _ in packages if p['id'] != 'io.vitrallis.debug'
               or p['version'] == state.get('debug', '0.1.2' if state.get('published') else '0.1.0')]
    if state.get('bad_app'):
        visible.append({'id': 'bad', 'name': 'Invalid app'})
    catalog = encoded(dict(schema_version=1, apps=visible))
    catalog_commit = hashlib.sha1(catalog).hexdigest()
    responses[f'api.github.com/repos/{ORIGIN}'] = encoded(dict(default_branch='main'))
    responses[f'api.github.com/repos/{ORIGIN}/commits/main'] = encoded(dict(sha=catalog_commit))
    responses[f'raw.githubusercontent.com/{ORIGIN}/{catalog_commit}/apps.json'] = catalog
    trees = {}
    for p, files in packages:
        commit = p['source']['commit']
        slug = p['source']['path'].split('/')[1]
        tree = hashlib.sha1((commit + '/apps').encode()).hexdigest()
        sub = hashlib.sha1((commit + '/' + slug).encode()).hexdigest()
        base = f'api.github.com/repos/{ORIGIN}/git/trees/'
        responses[base + commit] = encoded(dict(truncated=False, tree=[dict(path='apps', type='tree', mode='040000', sha=tree)]))
        trees.setdefault(base + tree, {})[slug] = dict(path=slug, type='tree', mode='040000', sha=sub)
        responses[base + sub + '?recursive=1'] = encoded(dict(truncated=False, tree=[
            dict(path=f['path'], size=f['size'], type='blob', mode='100644') for f in p['files']]))
        for name, data in files.items():
            responses[f'raw.githubusercontent.com/{ORIGIN}/{commit}/apps/{slug}/{name}'] = data
    for url, entries in trees.items():
        responses[url] = encoded(dict(truncated=False, tree=list(entries.values())))
    if state.get('offline'):
        responses = {k: v for k, v in responses.items() if '/apps/' in k}
    for key in list(responses):
        if state.get('fail_payload') and key.endswith('/' + state['fail_payload']):
            responses.pop(key)
        elif state.get('corrupt_payload') and key.endswith('/' + state['corrupt_payload']):
            responses[key] = b'x' * len(responses[key])
    return responses


class Handler(BaseHTTPRequestHandler):
    def do_GET(self):
        state = json.loads(STATE.read_text())
        key = self.headers['Host'] + self.path
        with REQUESTS.open('a') as log:
            log.write(json.dumps(key) + '\n')
        data = routes(state).get(key)
        self.send_response(200 if data is not None else 503)
        self.end_headers()
        self.wfile.write(data if data is not None else b'Simulator repository unavailable')

    def log_message(self, *_):
        pass


if __name__ == '__main__':
    server = ThreadingHTTPServer(('127.0.0.1', 443), Handler)
    context = ssl.SSLContext(ssl.PROTOCOL_TLS_SERVER)
    context.load_cert_chain('/sim/cert.pem', '/sim/key.pem')
    server.socket = context.wrap_socket(server.socket, server_side=True)
    server.serve_forever()
