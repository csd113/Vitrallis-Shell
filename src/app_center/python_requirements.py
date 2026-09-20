"""Metadata-only requirement validation, shared by detection and provisioning."""
import importlib.metadata as metadata
import re
import sys


def check(mode, lines):
    try:
        from packaging.requirements import Requirement
    except ImportError:
        try:
            from pip._vendor.packaging.requirements import Requirement
        except ImportError:
            Requirement = None
    for line in lines:
        # Apply policy even to inactive markers. Never give pip options, URLs,
        # local paths or extras a separate, less restrictive install path.
        if not re.fullmatch(r'[A-Za-z0-9][A-Za-z0-9_.-]*(?:\s*[<>=!~].*)?(?:\s*;.*)?', line) or any(c in line for c in '@/\\'):
            raise RuntimeError('Unsupported dependency declaration: ' + line)
        if Requirement is None:
            # Preserve dependency-free and simple system-Python detection when
            # neither packaging nor pip is installed. Complex syntax fails closed.
            if not re.fullmatch(r'[A-Za-z0-9_.-]+(?:==[A-Za-z0-9_.+-]+)?', line):
                raise RuntimeError('Python packaging is required to validate: ' + line)
            name, _, pin = line.partition('==')
            if mode != 'validate' and pin and metadata.version(name) != pin:
                raise RuntimeError('Dependency version mismatch: ' + line)
            if mode != 'validate':
                metadata.version(name)
            continue
        requirement = Requirement(line)
        if requirement.url or requirement.extras:
            raise RuntimeError('Unsupported dependency declaration: ' + line)
        if mode != 'validate' and (requirement.marker is None or requirement.marker.evaluate()):
            found = metadata.version(requirement.name)
            if found not in requirement.specifier:
                raise RuntimeError('Dependency version mismatch: ' + line + ' (found ' + found + ')')
    if mode == 'tk':
        import tkinter


if __name__ == '__main__':
    check(sys.argv[1], sys.argv[2:])
