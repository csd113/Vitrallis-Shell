#!/usr/bin/env python3
"""Check repository-local Markdown links and heading anchors without network access."""
from pathlib import Path
import re
import subprocess
from urllib.parse import unquote, urlsplit

ROOT = Path(__file__).resolve().parents[1]


def anchors(path):
    result = set()
    counts = {}
    text = re.sub(r'```.*?```', '', path.read_text(), flags=re.S)
    for title in re.findall(r'^#{1,6}\s+(.+?)\s*#*$', text, flags=re.M):
        title = re.sub(r'\[([^]]+)\]\([^)]*\)', r'\1', title)
        slug = re.sub(r'[^\w\- ]', '', title.lower()).replace(' ', '-')
        number = counts.get(slug, 0)
        counts[slug] = number + 1
        result.add(slug + ('-' + str(number) if number else ''))
    return result


def main():
    listing = subprocess.check_output(
        ['git', 'ls-files', '-z', '--cached', '--others', '--exclude-standard'], cwd=ROOT)
    files = sorted({name for name in listing.decode('utf-8').split('\0')
                    if name.endswith('.md') and (ROOT / name).is_file()})
    failures = []
    for name in files:
        path = ROOT / name
        content = re.sub(r'```.*?```', '', path.read_text(), flags=re.S)
        for raw in re.findall(r'\]\(([^\s)]+)(?:\s+"[^"]*")?\)', content):
            url = urlsplit(raw.strip('<>'))
            if url.scheme or url.netloc:
                continue
            target = path.parent / unquote(url.path) if url.path else path
            if not target.exists():
                failures.append(name + ': missing ' + raw)
            elif url.fragment and target.suffix == '.md' and unquote(url.fragment) not in anchors(target):
                failures.append(name + ': missing anchor ' + raw)
    if failures:
        raise SystemExit('\n'.join(failures))
    print('Local Markdown links and anchors passed (' + str(len(files)) + ' files).')


if __name__ == '__main__':
    main()
